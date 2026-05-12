use std::collections::BTreeMap;

use serde::Serialize;

use crate::{
    capability::CapabilityProfile,
    host::{HostCallContext, HostCallResult, HostFunctionRegistry, HostFunctionSpec},
    module_session::ModuleSession,
    resource::{HandleEntry, HandleTable, ResourceEntry, ResourceOpenRequest, ResourceRegistry},
    response::{EvalResponse, ResponseOptions},
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
    profiles: BTreeMap<String, CapabilityProfile>,
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
            profiles: BTreeMap::new(),
            active_profile_id: None,
        }
    }

    pub fn runtime_id(&self) -> &str {
        &self.config.runtime_id
    }

    pub fn eval(&mut self, source: &str) -> EvalResponse {
        self.session.eval_response(source)
    }

    pub fn eval_with_options(&mut self, source: &str, options: ResponseOptions) -> EvalResponse {
        self.session.eval_response_with_options(source, options)
    }

    pub fn eval_source_text(&mut self, source: &SourceText) -> EvalResponse {
        self.session.eval_source_text_response(source)
    }

    pub fn eval_source_text_with_options(
        &mut self,
        source: &SourceText,
        options: ResponseOptions,
    ) -> EvalResponse {
        self.session
            .eval_source_text_response_with_options(source, options)
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
        self.profiles.insert(profile.profile_id.clone(), profile);
    }

    pub fn activate_profile(&mut self, profile_id: &str) -> Result<(), EmbeddedRuntimeError> {
        let profile =
            self.profiles
                .get(profile_id)
                .cloned()
                .ok_or_else(|| EmbeddedRuntimeError {
                    kind: EmbeddedRuntimeErrorKind::ProfileNotFound,
                    message: format!("capability profile {profile_id} is not registered"),
                    expected: self.profiles.keys().cloned().collect(),
                    actual: Some(profile_id.to_string()),
                    suggestion: Some(
                        "Register the profile before activating it for this runtime.".to_string(),
                    ),
                })?;

        self.session.set_capability_profile(profile);
        self.active_profile_id = Some(profile_id.to_string());
        Ok(())
    }

    pub fn clear_active_profile(&mut self) {
        self.session.clear_capability_profile();
        self.active_profile_id = None;
    }

    pub fn active_profile(&self) -> Option<&CapabilityProfile> {
        self.active_profile_id
            .as_deref()
            .and_then(|profile_id| self.profiles.get(profile_id))
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
        let request = ResourceOpenRequest::new(holder, resource_id, grants);

        if let Some(profile) = profile.as_ref() {
            self.resources
                .open_handle_with_profile(&mut self.handles, profile, request)
        } else {
            self.resources.open_handle(&mut self.handles, request)
        }
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
        self.profiles.values()
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
            profiles: self.profiles.values().cloned().collect(),
            resources: self.resources.resources().cloned().collect(),
            handles: self.handles.handles().cloned().collect(),
        }
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
