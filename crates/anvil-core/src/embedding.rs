use serde::Serialize;

use crate::{
    capability::{CapabilityPolicy, CapabilityPolicyError, CapabilityProfile},
    host::{HostCallContext, HostCallResult, HostFunctionRegistry, HostFunctionSpec},
    module_session::ModuleSession,
    resource::{
        HandleEntry, HandleTable, ResourceDenialReason, ResourceEntry, ResourceOpenRequest,
        ResourceRegistry,
    },
    response::{EvalResponse, ResponseOptions, ResponseStatus},
    source::SourceText,
    vm::{Value, VmBudget},
};

pub const EMBEDDING_PROTOCOL: &str = "anvil.embedding.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EmbeddedRuntimeConfig {
    pub runtime_id: String,
    pub default_budget: VmBudget,
}

impl EmbeddedRuntimeConfig {
    pub fn new(runtime_id: impl Into<String>) -> Self {
        Self {
            runtime_id: runtime_id.into(),
            default_budget: VmBudget::default(),
        }
    }

    pub fn with_default_budget(mut self, budget: VmBudget) -> Self {
        self.default_budget = budget;
        self
    }
}

#[derive(Debug, Clone)]
pub struct EmbeddedRuntime {
    config: EmbeddedRuntimeConfig,
    session: ModuleSession,
    resources: ResourceRegistry,
    handles: HandleTable,
    policy: CapabilityPolicy,
    audit_events: Vec<EmbeddedRuntimeAuditEvent>,
    active_profile_id: Option<String>,
}

impl EmbeddedRuntime {
    pub fn new(runtime_id: impl Into<String>) -> Self {
        Self::with_config(EmbeddedRuntimeConfig::new(runtime_id))
    }

    pub fn with_config(config: EmbeddedRuntimeConfig) -> Self {
        let mut session = ModuleSession::new();
        session.set_budget(config.default_budget);

        Self {
            config,
            session,
            resources: ResourceRegistry::new(),
            handles: HandleTable::new(),
            policy: CapabilityPolicy::new(),
            audit_events: Vec::new(),
            active_profile_id: None,
        }
    }

    pub fn runtime_id(&self) -> &str {
        &self.config.runtime_id
    }

    pub fn eval(&mut self, source: &str) -> EvalResponse {
        let response = self.session.eval_response(source);
        self.record_eval_authority_denial(&response);
        response
    }

    pub fn eval_with_options(&mut self, source: &str, options: ResponseOptions) -> EvalResponse {
        let response = self.session.eval_response_with_options(source, options);
        self.record_eval_authority_denial(&response);
        response
    }

    pub fn eval_source_text(&mut self, source: &SourceText) -> EvalResponse {
        let response = self.session.eval_source_text_response(source);
        self.record_eval_authority_denial(&response);
        response
    }

    pub fn eval_source_text_with_options(
        &mut self,
        source: &SourceText,
        options: ResponseOptions,
    ) -> EvalResponse {
        let response = self
            .session
            .eval_source_text_response_with_options(source, options);
        self.record_eval_authority_denial(&response);
        response
    }

    pub fn register_host_function<F>(&mut self, spec: HostFunctionSpec, function: F)
    where
        F: Fn(&HostCallContext, &[Value]) -> HostCallResult + Send + Sync + 'static,
    {
        self.session.register_host_function(spec, function);
    }

    pub fn register_resource(&mut self, resource: ResourceEntry) {
        self.resources.register(resource);
    }

    pub fn register_profile(&mut self, profile: CapabilityProfile) {
        self.policy.register_profile(profile);
    }

    pub fn register_composed_profile<I, S>(
        &mut self,
        profile_id: impl Into<String>,
        component_ids: I,
    ) -> Result<CapabilityProfile, EmbeddedRuntimeError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let profile = self
            .policy
            .compose_profile(profile_id, component_ids)
            .map_err(EmbeddedRuntimeError::from)?;
        self.record_audit_event(
            EmbeddedRuntimeAuditEvent::allowed(
                EmbeddedRuntimeAuditKind::ProfileComposed,
                &self.config.runtime_id,
            )
            .with_profile(&profile),
        );
        self.policy.register_profile(profile.clone());
        Ok(profile)
    }

    pub fn activate_profile(&mut self, profile_id: &str) -> Result<(), EmbeddedRuntimeError> {
        let profile = match self.policy.profile(profile_id).cloned() {
            Some(profile) => profile,
            None => {
                let error = EmbeddedRuntimeError {
                    kind: EmbeddedRuntimeErrorKind::ProfileNotFound,
                    message: format!("capability profile {profile_id} is not registered"),
                    expected: self.policy.profile_ids().cloned().collect(),
                    actual: Some(profile_id.to_string()),
                    suggestion: Some(
                        "Register the profile before activating it for this runtime.".to_string(),
                    ),
                };
                self.record_audit_event(
                    EmbeddedRuntimeAuditEvent::denied(
                        EmbeddedRuntimeAuditKind::ProfileActivationDenied,
                        &self.config.runtime_id,
                    )
                    .with_profile_id(profile_id)
                    .with_message(&error.message),
                );
                return Err(error);
            }
        };

        self.session.set_capability_profile(profile.clone());
        self.active_profile_id = Some(profile_id.to_string());
        self.record_audit_event(
            EmbeddedRuntimeAuditEvent::allowed(
                EmbeddedRuntimeAuditKind::ProfileActivated,
                &self.config.runtime_id,
            )
            .with_profile(&profile),
        );
        Ok(())
    }

    pub fn clear_active_profile(&mut self) {
        self.session.clear_capability_profile();
        self.active_profile_id = None;
    }

    pub fn active_profile(&self) -> Option<&CapabilityProfile> {
        self.active_profile_id
            .as_deref()
            .and_then(|profile_id| self.policy.profile(profile_id))
    }

    pub fn open_resource(
        &mut self,
        resource_id: impl Into<String>,
        grants: Vec<String>,
    ) -> crate::resource::ResourceResult<HandleEntry> {
        let resource_id = resource_id.into();
        let profile = self.active_profile().cloned();
        let holder = profile
            .as_ref()
            .map(|profile| profile.principal.clone())
            .unwrap_or_else(|| self.config.runtime_id.clone());
        let request = ResourceOpenRequest::new(holder, resource_id.clone(), grants);

        let result = if let Some(profile) = profile.as_ref() {
            self.resources
                .open_handle_with_profile(&mut self.handles, profile, request)
        } else {
            self.resources.open_handle(&mut self.handles, request)
        };

        match &result {
            Ok(handle) => self.record_audit_event(
                EmbeddedRuntimeAuditEvent::allowed(
                    EmbeddedRuntimeAuditKind::ResourceOpened,
                    &self.config.runtime_id,
                )
                .with_profile_opt(profile.as_ref())
                .with_resource(&handle.resource_id, "open")
                .with_trust_zone(&handle.trust_zone),
            ),
            Err(error) => self.record_audit_event(
                EmbeddedRuntimeAuditEvent::denied(
                    EmbeddedRuntimeAuditKind::ResourceOpenDenied,
                    &self.config.runtime_id,
                )
                .with_profile_opt(profile.as_ref())
                .with_resource(&resource_id, "open")
                .with_resource_denial(error.denial.reason)
                .with_capability_opt(error.denial.missing_capability.as_deref())
                .with_message(error.diagnostic.message.as_str()),
            ),
        }

        result
    }

    pub fn host_functions(&self) -> &HostFunctionRegistry {
        self.session.host_functions()
    }

    pub fn resources(&self) -> &ResourceRegistry {
        &self.resources
    }

    pub fn handles(&self) -> &HandleTable {
        &self.handles
    }

    pub fn profiles(&self) -> impl Iterator<Item = &CapabilityProfile> {
        self.policy.profiles()
    }

    pub fn policy(&self) -> &CapabilityPolicy {
        &self.policy
    }

    pub fn audit_events(&self) -> &[EmbeddedRuntimeAuditEvent] {
        &self.audit_events
    }

    pub fn session(&self) -> &ModuleSession {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut ModuleSession {
        &mut self.session
    }

    pub fn snapshot(&self) -> EmbeddedRuntimeSnapshot {
        EmbeddedRuntimeSnapshot {
            protocol: EMBEDDING_PROTOCOL,
            runtime_id: self.config.runtime_id.clone(),
            default_budget: self.config.default_budget,
            active_profile_id: self.active_profile_id.clone(),
            host_functions: self.host_functions().specs().into_iter().cloned().collect(),
            profiles: self.policy.profiles().cloned().collect(),
            resources: self.resources.resources().cloned().collect(),
            handles: self.handles.handles().cloned().collect(),
            audit_events: self.audit_events.clone(),
        }
    }

    fn record_eval_authority_denial(&mut self, response: &EvalResponse) {
        if response.status != ResponseStatus::Error {
            return;
        }

        let Some(diagnostic) = response.primary_diagnostic() else {
            return;
        };

        if !matches!(
            diagnostic.code,
            "ANVIL_RUNTIME_HOST_PROFILE_REQUIRED"
                | "ANVIL_RUNTIME_HOST_TRUST_ZONE_DENIED"
                | "ANVIL_RUNTIME_HOST_CAPABILITY_DENIED"
        ) {
            return;
        }

        let profile = self.active_profile().cloned();
        self.record_audit_event(
            EmbeddedRuntimeAuditEvent::denied(
                EmbeddedRuntimeAuditKind::EvalDenied,
                &self.config.runtime_id,
            )
            .with_profile_opt(profile.as_ref())
            .with_diagnostic(diagnostic.code)
            .with_message(&diagnostic.message),
        );
    }

    fn record_audit_event(&mut self, mut event: EmbeddedRuntimeAuditEvent) {
        event.event_id = format!(
            "audit:{}:{}",
            sanitize_audit_part(&self.config.runtime_id),
            self.audit_events.len() + 1
        );
        self.audit_events.push(event);
    }
}

impl Default for EmbeddedRuntime {
    fn default() -> Self {
        Self::new("runtime")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EmbeddedRuntimeSnapshot {
    pub protocol: &'static str,
    pub runtime_id: String,
    pub default_budget: VmBudget,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_profile_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub host_functions: Vec<HostFunctionSpec>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<CapabilityProfile>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<ResourceEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub handles: Vec<HandleEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub audit_events: Vec<EmbeddedRuntimeAuditEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EmbeddedRuntimeAuditEvent {
    pub event_id: String,
    pub kind: EmbeddedRuntimeAuditKind,
    pub decision: EmbeddedRuntimeAuditDecision,
    pub runtime_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub principal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust_zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_denial: Option<ResourceDenialReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl EmbeddedRuntimeAuditEvent {
    fn allowed(kind: EmbeddedRuntimeAuditKind, runtime_id: &str) -> Self {
        Self::new(kind, EmbeddedRuntimeAuditDecision::Allowed, runtime_id)
    }

    fn denied(kind: EmbeddedRuntimeAuditKind, runtime_id: &str) -> Self {
        Self::new(kind, EmbeddedRuntimeAuditDecision::Denied, runtime_id)
    }

    fn new(
        kind: EmbeddedRuntimeAuditKind,
        decision: EmbeddedRuntimeAuditDecision,
        runtime_id: &str,
    ) -> Self {
        Self {
            event_id: String::new(),
            kind,
            decision,
            runtime_id: runtime_id.to_string(),
            profile_id: None,
            principal: None,
            trust_zone: None,
            capability: None,
            resource_id: None,
            operation: None,
            resource_denial: None,
            diagnostic_code: None,
            message: None,
        }
    }

    fn with_profile(mut self, profile: &CapabilityProfile) -> Self {
        self.profile_id = Some(profile.profile_id.clone());
        self.principal = Some(profile.principal.clone());
        self
    }

    fn with_profile_opt(self, profile: Option<&CapabilityProfile>) -> Self {
        if let Some(profile) = profile {
            self.with_profile(profile)
        } else {
            self
        }
    }

    fn with_profile_id(mut self, profile_id: &str) -> Self {
        self.profile_id = Some(profile_id.to_string());
        self
    }

    fn with_resource(mut self, resource_id: &str, operation: &str) -> Self {
        self.resource_id = Some(resource_id.to_string());
        self.operation = Some(operation.to_string());
        self
    }

    fn with_trust_zone(mut self, trust_zone: &str) -> Self {
        self.trust_zone = Some(trust_zone.to_string());
        self
    }

    fn with_capability_opt(mut self, capability: Option<&str>) -> Self {
        self.capability = capability.map(ToString::to_string);
        self
    }

    fn with_resource_denial(mut self, reason: ResourceDenialReason) -> Self {
        self.resource_denial = Some(reason);
        self
    }

    fn with_diagnostic(mut self, code: &str) -> Self {
        self.diagnostic_code = Some(code.to_string());
        self
    }

    fn with_message(mut self, message: &str) -> Self {
        self.message = Some(message.to_string());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddedRuntimeAuditKind {
    ProfileActivated,
    ProfileActivationDenied,
    ProfileComposed,
    ResourceOpened,
    ResourceOpenDenied,
    EvalDenied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddedRuntimeAuditDecision {
    Allowed,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EmbeddedRuntimeError {
    pub kind: EmbeddedRuntimeErrorKind,
    pub message: String,
    pub expected: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddedRuntimeErrorKind {
    ProfileNotFound,
    PolicyError,
}

impl From<CapabilityPolicyError> for EmbeddedRuntimeError {
    fn from(error: CapabilityPolicyError) -> Self {
        Self {
            kind: EmbeddedRuntimeErrorKind::PolicyError,
            message: error.message,
            expected: error.expected,
            actual: error.actual,
            suggestion: error.suggestion,
        }
    }
}

fn sanitize_audit_part(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::{HostFunctionSignature, HostParameterSpec, HostResultSpec, HostValueType};

    #[test]
    fn embedded_runtime_evaluates_with_response_envelopes() {
        let mut runtime = EmbeddedRuntime::new("agent-runtime");

        let define = runtime.eval("(define answer 42)");
        let answer = runtime.eval("answer");

        assert_eq!(define.status, crate::response::ResponseStatus::Ok);
        assert_eq!(answer.summary, "42");
        assert_eq!(answer.value().expect("value").display, "42");
    }

    #[test]
    fn runtime_config_budget_and_eval_options_are_visible() {
        let mut runtime = EmbeddedRuntime::with_config(
            EmbeddedRuntimeConfig::new("agent-runtime")
                .with_default_budget(VmBudget::with_instruction_fuel(20)),
        );

        assert_eq!(runtime.runtime_id(), "agent-runtime");
        assert_eq!(
            runtime.session().budget(),
            VmBudget::with_instruction_fuel(20)
        );

        runtime
            .session_mut()
            .set_budget(VmBudget::with_instruction_fuel(30));
        assert_eq!(
            runtime.session().budget(),
            VmBudget::with_instruction_fuel(30)
        );

        let source = SourceText::repl("(+ 1 2)");
        let summary = runtime.eval_source_text(&source);
        let debug = runtime.eval_source_text_with_options(&source, ResponseOptions::debug());
        let debug_from_str = runtime.eval_with_options("(+ 2 3)", ResponseOptions::debug());

        assert_eq!(summary.summary, "3");
        assert!(summary.facets.is_empty());
        assert_eq!(debug.summary, "3");
        assert_eq!(debug.facets[0].name, "vm.metrics");
        assert_eq!(debug_from_str.summary, "5");
    }

    #[test]
    fn embedded_runtime_snapshot_reports_registered_contract_surface() {
        let mut runtime = EmbeddedRuntime::new("agent-runtime");
        runtime.register_host_function(
            HostFunctionSpec::new("host/add")
                .with_exact_arity(2)
                .with_signature(HostFunctionSignature::new(
                    vec![
                        HostParameterSpec::required("left", HostValueType::Integer),
                        HostParameterSpec::required("right", HostValueType::Integer),
                    ],
                    HostResultSpec::new(HostValueType::Integer),
                )),
            |_context, _args| Ok(Value::Nil),
        );
        runtime.register_profile(CapabilityProfile::read_only(
            "readonly",
            "agent.alpha",
            "project.markodb",
        ));
        runtime.register_resource(ResourceEntry::new(
            "markodb:papers",
            "markodb.collection",
            "runtime",
            "project.markodb",
        ));

        let snapshot = runtime.snapshot();

        assert_eq!(snapshot.protocol, EMBEDDING_PROTOCOL);
        assert_eq!(snapshot.runtime_id, "agent-runtime");
        assert_eq!(snapshot.host_functions[0].name, "host/add");
        assert_eq!(snapshot.profiles[0].profile_id, "readonly");
        assert_eq!(snapshot.resources[0].resource_id, "markodb:papers");
    }

    #[test]
    fn composed_profiles_can_be_registered_activated_and_audited() {
        let mut runtime = EmbeddedRuntime::new("agent-runtime");
        runtime.register_profile(
            CapabilityProfile::new("reader", "agent.alpha", "project.markodb")
                .with_capabilities(["resource/open", "resource/read"]),
        );
        runtime.register_profile(
            CapabilityProfile::new("qbbn", "agent.alpha", "project.qbbn")
                .with_capability("qbbn/ask"),
        );

        let profile = runtime
            .register_composed_profile("agent.alpha.composed", ["reader", "qbbn"])
            .expect("composed profile");
        runtime
            .activate_profile("agent.alpha.composed")
            .expect("profile activated");

        assert_eq!(profile.principal, "agent.alpha");
        assert!(runtime.policy().profile("agent.alpha.composed").is_some());
        assert_eq!(
            runtime.active_profile().expect("active profile").profile_id,
            "agent.alpha.composed"
        );
        assert_eq!(runtime.audit_events().len(), 2);
        assert_eq!(
            runtime.audit_events()[0].kind,
            EmbeddedRuntimeAuditKind::ProfileComposed
        );
        assert_eq!(
            runtime.audit_events()[1].decision,
            EmbeddedRuntimeAuditDecision::Allowed
        );
    }

    #[test]
    fn runtime_audit_log_records_authority_denials_and_resource_opens() {
        let mut runtime = EmbeddedRuntime::new("agent-runtime");
        runtime.register_host_function(
            HostFunctionSpec::new("host/secret")
                .with_exact_arity(0)
                .with_required_capability("host/secret")
                .with_trust_zone("project.markodb"),
            |_context, _args| Ok(Value::Keyword("authorized".to_string())),
        );
        runtime.register_profile(
            CapabilityProfile::new("readonly", "agent.alpha", "project.markodb")
                .with_capability("host/read"),
        );
        runtime
            .activate_profile("readonly")
            .expect("profile activated");

        let response = runtime.eval("(host/secret)");

        assert_eq!(response.status, crate::response::ResponseStatus::Error);
        assert!(
            runtime.audit_events().iter().any(|event| {
                event.kind == EmbeddedRuntimeAuditKind::EvalDenied
                    && event.diagnostic_code.as_deref()
                        == Some("ANVIL_RUNTIME_HOST_CAPABILITY_DENIED")
            }),
            "{:?}",
            runtime.audit_events()
        );

        runtime.register_resource(
            ResourceEntry::new(
                "markodb:papers",
                "markodb.collection",
                "runtime",
                "project.markodb",
            )
            .with_operation("read", "read"),
        );
        runtime
            .open_resource("markodb:papers", vec!["read".to_string()])
            .expect_err("profile missing resource/read capability");

        assert!(
            runtime.audit_events().iter().any(|event| {
                event.kind == EmbeddedRuntimeAuditKind::ResourceOpenDenied
                    && event.resource_denial == Some(ResourceDenialReason::CapabilityDenied)
            }),
            "{:?}",
            runtime.audit_events()
        );
    }

    #[test]
    fn resource_open_without_active_profile_uses_runtime_holder() {
        let mut runtime = EmbeddedRuntime::new("agent-runtime");
        runtime.register_resource(
            ResourceEntry::new(
                "markodb:papers",
                "markodb.collection",
                "runtime",
                "project.markodb",
            )
            .with_operation("read", "read"),
        );

        let handle = runtime
            .open_resource("markodb:papers", vec!["read".to_string()])
            .expect("resource handle");

        assert_eq!(handle.holder, "agent-runtime");
        assert_eq!(
            runtime
                .resources()
                .get("markodb:papers")
                .expect("resource")
                .type_id,
            "markodb.collection"
        );
        assert_eq!(
            runtime
                .handles()
                .get(&handle.handle_id)
                .expect("stored handle")
                .resource_id,
            "markodb:papers"
        );
    }

    #[test]
    fn profile_activation_reports_missing_profiles_and_can_clear() {
        let mut runtime = EmbeddedRuntime::new("agent-runtime");
        runtime.register_profile(CapabilityProfile::read_only(
            "readonly",
            "agent.alpha",
            "project.markodb",
        ));

        let missing = runtime
            .activate_profile("writer")
            .expect_err("missing profile");
        assert_eq!(missing.kind, EmbeddedRuntimeErrorKind::ProfileNotFound);
        assert_eq!(missing.expected, vec!["readonly"]);
        assert_eq!(missing.actual.as_deref(), Some("writer"));
        assert_eq!(
            runtime.audit_events()[0].kind,
            EmbeddedRuntimeAuditKind::ProfileActivationDenied
        );
        assert_eq!(
            runtime.audit_events()[0].decision,
            EmbeddedRuntimeAuditDecision::Denied
        );

        runtime
            .activate_profile("readonly")
            .expect("profile activated");
        assert_eq!(
            runtime.active_profile().expect("active profile").principal,
            "agent.alpha"
        );
        assert_eq!(runtime.profiles().count(), 1);
        assert_eq!(
            runtime.snapshot().active_profile_id.as_deref(),
            Some("readonly")
        );

        runtime.clear_active_profile();

        assert!(runtime.active_profile().is_none());
        assert!(runtime.session().capability_profile().is_none());
    }

    #[test]
    fn composed_profile_errors_are_structured_runtime_errors() {
        let mut runtime = EmbeddedRuntime::new("agent-runtime");
        runtime.register_profile(CapabilityProfile::new(
            "alpha",
            "agent.alpha",
            "project.markodb",
        ));
        runtime.register_profile(CapabilityProfile::new(
            "beta",
            "agent.beta",
            "project.markodb",
        ));

        let empty = runtime
            .register_composed_profile("empty", Vec::<String>::new())
            .expect_err("empty composition");
        assert_eq!(empty.kind, EmbeddedRuntimeErrorKind::PolicyError);
        assert_eq!(empty.expected, vec!["one or more profile ids"]);

        let missing = runtime
            .register_composed_profile("missing", ["alpha", "absent"])
            .expect_err("missing composition");
        assert_eq!(missing.kind, EmbeddedRuntimeErrorKind::PolicyError);
        assert_eq!(missing.actual.as_deref(), Some("absent"));

        let mismatch = runtime
            .register_composed_profile("mixed", ["alpha", "beta"])
            .expect_err("cross-principal composition");
        assert_eq!(mismatch.kind, EmbeddedRuntimeErrorKind::PolicyError);
        assert_eq!(mismatch.actual.as_deref(), Some("principal:agent.beta"));
    }

    #[test]
    fn active_profiles_gate_host_calls_before_callbacks() {
        let mut runtime = EmbeddedRuntime::new("agent-runtime");
        runtime.register_host_function(
            HostFunctionSpec::new("host/secret")
                .with_exact_arity(0)
                .with_required_capability("host/secret")
                .with_trust_zone("project.markodb"),
            |_context, _args| Ok(Value::Keyword("authorized".to_string())),
        );
        runtime.register_profile(CapabilityProfile::read_only(
            "readonly",
            "agent.alpha",
            "project.markodb",
        ));
        runtime
            .activate_profile("readonly")
            .expect("profile activated");

        let response = runtime.eval("(host/secret)");

        assert_eq!(response.status, crate::response::ResponseStatus::Error);
        assert_eq!(
            response.primary_diagnostic().expect("diagnostic").code,
            "ANVIL_RUNTIME_HOST_CAPABILITY_DENIED"
        );
    }
}
