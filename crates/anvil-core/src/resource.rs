use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::{
    diagnostic::{Diagnostic, DiagnosticPhase, DiagnosticSpec},
    source::{SourceLocation, SourceSpan, SourceText},
};

pub type ResourceDiagnostic = Diagnostic;
pub type ResourceResult<T> = Result<T, Box<ResourceError>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourceEntry {
    pub resource_id: String,
    pub type_id: String,
    pub owner: String,
    pub trust_zone: String,
    pub operations: Vec<ResourceOperationSchema>,
    pub policy: ResourcePolicy,
    pub budget_policy: ResourceBudgetPolicy,
    pub debug_policy: ResourceDebugPolicy,
}

impl ResourceEntry {
    pub fn new(
        resource_id: impl Into<String>,
        type_id: impl Into<String>,
        owner: impl Into<String>,
        trust_zone: impl Into<String>,
    ) -> Self {
        Self {
            resource_id: resource_id.into(),
            type_id: type_id.into(),
            owner: owner.into(),
            trust_zone: trust_zone.into(),
            operations: Vec::new(),
            policy: ResourcePolicy::default(),
            budget_policy: ResourceBudgetPolicy::default(),
            debug_policy: ResourceDebugPolicy::default(),
        }
    }

    pub fn with_operation(
        mut self,
        operation: impl Into<String>,
        capability: impl Into<String>,
    ) -> Self {
        self.add_operation(operation, capability);
        self
    }

    pub fn with_delegation_policy(mut self, delegation_policy: HandleDelegationPolicy) -> Self {
        self.policy.delegation_policy = delegation_policy;
        self
    }

    pub fn add_operation(&mut self, operation: impl Into<String>, capability: impl Into<String>) {
        self.operations
            .push(ResourceOperationSchema::new(operation, capability));
    }

    pub fn operation_schema(&self, operation: &str) -> Option<&ResourceOperationSchema> {
        self.operations
            .iter()
            .find(|schema| schema.operation == operation)
    }

    pub fn supported_capabilities(&self) -> BTreeSet<String> {
        self.operations
            .iter()
            .map(|schema| schema.required_capability.clone())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourceOperationSchema {
    pub operation: String,
    pub required_capability: String,
    pub effect: ResourceEffect,
}

impl ResourceOperationSchema {
    pub fn new(operation: impl Into<String>, required_capability: impl Into<String>) -> Self {
        let operation = operation.into();
        let effect = ResourceEffect::from_operation(&operation);

        Self {
            operation,
            required_capability: required_capability.into(),
            effect,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceEffect {
    Import,
    Read,
    Write,
    Call,
    Stream,
    Inspect,
    Delegate,
    Close,
    Revoke,
}

impl ResourceEffect {
    fn from_operation(operation: &str) -> Self {
        match operation {
            "import" | "open" => Self::Import,
            "read" => Self::Read,
            "write" => Self::Write,
            "stream" => Self::Stream,
            "inspect" => Self::Inspect,
            "delegate" => Self::Delegate,
            "close" => Self::Close,
            "revoke" => Self::Revoke,
            _ => Self::Call,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourcePolicy {
    pub delegation_policy: HandleDelegationPolicy,
    pub audit_policy: ResourceAuditPolicy,
    pub redaction_policy: ResourceRedactionPolicy,
    pub lifetime_policy: ResourceLifetime,
}

impl Default for ResourcePolicy {
    fn default() -> Self {
        Self {
            delegation_policy: HandleDelegationPolicy::NotAllowed,
            audit_policy: ResourceAuditPolicy::AllEffects,
            redaction_policy: ResourceRedactionPolicy::Redacted,
            lifetime_policy: ResourceLifetime::Runtime,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourceBudgetPolicy {
    pub host_calls: Option<usize>,
    pub memory_bytes: Option<usize>,
}

impl Default for ResourceBudgetPolicy {
    fn default() -> Self {
        Self {
            host_calls: Some(1),
            memory_bytes: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourceDebugPolicy {
    pub inspectable: bool,
    pub redact_values: bool,
}

impl Default for ResourceDebugPolicy {
    fn default() -> Self {
        Self {
            inspectable: true,
            redact_values: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceAuditPolicy {
    AllEffects,
    DenialsOnly,
    Redacted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceRedactionPolicy {
    Redacted,
    MetadataOnly,
    FullWhenAuthorized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceLifetime {
    Lexical,
    Call,
    Process,
    Actor,
    Session,
    Runtime,
    Lease,
    Stream,
    Draft,
    TestRun,
    ArtifactBound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HandleDelegationPolicy {
    NotAllowed,
    NarrowOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HandleRevocationState {
    Active,
    Closing,
    Closed,
    Expired,
    Revoked,
    Poisoned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HandleDisplayPolicy {
    Redacted,
    MetadataOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HandleEntry {
    pub handle_id: String,
    pub resource_id: String,
    pub type_id: String,
    pub holder: String,
    pub grants: Vec<String>,
    pub trust_zone: String,
    pub lifetime: ResourceLifetime,
    pub revocation_state: HandleRevocationState,
    pub delegation_policy: HandleDelegationPolicy,
    pub audit_policy: ResourceAuditPolicy,
    pub display_policy: HandleDisplayPolicy,
}

impl HandleEntry {
    fn new(
        handle_id: String,
        resource: &ResourceEntry,
        holder: String,
        grants: Vec<String>,
    ) -> Self {
        Self {
            handle_id,
            resource_id: resource.resource_id.clone(),
            type_id: resource.type_id.clone(),
            holder,
            grants,
            trust_zone: resource.trust_zone.clone(),
            lifetime: resource.policy.lifetime_policy,
            revocation_state: HandleRevocationState::Active,
            delegation_policy: resource.policy.delegation_policy,
            audit_policy: resource.policy.audit_policy,
            display_policy: HandleDisplayPolicy::Redacted,
        }
    }

    pub fn has_grant(&self, grant: &str) -> bool {
        self.grants.iter().any(|candidate| candidate == grant)
    }

    pub fn display_summary(&self) -> String {
        format!(
            "#<resource {} {} caps=[{}] zone={}>",
            self.type_id,
            self.resource_id,
            self.grants.join(" "),
            self.trust_zone
        )
    }

    fn delegated(&self, handle_id: String, holder: String, grants: Vec<String>) -> HandleEntry {
        let mut delegated = self.clone();
        delegated.handle_id = handle_id;
        delegated.holder = holder;
        delegated.grants = grants;
        delegated
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct HandleTable {
    entries: BTreeMap<String, HandleEntry>,
    next_handle_index: usize,
}

impl HandleTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, handle_id: &str) -> Option<&HandleEntry> {
        self.entries.get(handle_id)
    }

    pub fn handles(&self) -> impl Iterator<Item = &HandleEntry> {
        self.entries.values()
    }

    pub fn close(&mut self, handle_id: &str) -> ResourceResult<HandleEntry> {
        self.set_revocation_state(handle_id, HandleRevocationState::Closed, "close")
    }

    pub fn revoke(&mut self, handle_id: &str) -> ResourceResult<HandleEntry> {
        self.set_revocation_state(handle_id, HandleRevocationState::Revoked, "revoke")
    }

    fn allocate_handle_id(&mut self) -> String {
        self.next_handle_index += 1;
        format!("handle-{}", self.next_handle_index)
    }

    fn insert(&mut self, entry: HandleEntry) {
        self.entries.insert(entry.handle_id.clone(), entry);
    }

    fn set_revocation_state(
        &mut self,
        handle_id: &str,
        state: HandleRevocationState,
        operation: &str,
    ) -> ResourceResult<HandleEntry> {
        let entry = self.entries.get_mut(handle_id).ok_or_else(|| {
            ResourceError::new(ResourceErrorSpec {
                reason: ResourceDenialReason::HandleMissing,
                operation,
                handle_id: Some(handle_id),
                resource_id: None,
                type_id: None,
                holder: None,
                trust_zone: None,
                expected: vec!["active handle".to_string()],
                actual: None,
                missing_capability: None,
            })
        })?;

        entry.revocation_state = state;
        Ok(entry.clone())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ResourceRegistry {
    resources: BTreeMap<String, ResourceEntry>,
}

impl ResourceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, resource: ResourceEntry) {
        self.resources
            .insert(resource.resource_id.clone(), resource);
    }

    pub fn get(&self, resource_id: &str) -> Option<&ResourceEntry> {
        self.resources.get(resource_id)
    }

    pub fn open_handle(
        &self,
        table: &mut HandleTable,
        request: ResourceOpenRequest,
    ) -> ResourceResult<HandleEntry> {
        let resource = self.resource_or_error(&request.resource_id, "open")?;
        self.ensure_requested_grants(resource, &request.grants, "open")?;

        let handle_id = request
            .handle_id
            .unwrap_or_else(|| table.allocate_handle_id());
        let handle = HandleEntry::new(handle_id, resource, request.holder, request.grants);
        table.insert(handle.clone());
        Ok(handle)
    }

    pub fn check_operation(
        &self,
        table: &HandleTable,
        handle_id: &str,
        operation: &str,
    ) -> ResourceResult<ResourceOperationAuthorization> {
        let handle = self.handle_or_error(table, handle_id, operation)?;
        self.ensure_active(handle, operation)?;
        let resource = self.resource_or_error(&handle.resource_id, operation)?;
        self.ensure_handle_matches_resource(handle, resource, operation)?;
        let schema = self.operation_schema_or_error(resource, handle, operation)?;
        self.ensure_handle_grant(handle, schema, operation)?;

        Ok(ResourceOperationAuthorization {
            operation: operation.to_string(),
            handle_id: handle.handle_id.clone(),
            resource_id: handle.resource_id.clone(),
            type_id: handle.type_id.clone(),
            capability: schema.required_capability.clone(),
            audit_event: ResourceAuditEvent::allowed(
                operation,
                &handle.handle_id,
                &handle.resource_id,
                &handle.holder,
                &handle.trust_zone,
            ),
        })
    }

    pub fn delegate_handle(
        &self,
        table: &mut HandleTable,
        request: ResourceDelegationRequest,
    ) -> ResourceResult<HandleEntry> {
        let source = self
            .handle_or_error(table, &request.source_handle_id, "delegate")?
            .clone();
        self.ensure_active(&source, "delegate")?;
        self.ensure_delegation_allowed(&source, &request.grants)?;
        self.resource_or_error(&source.resource_id, "delegate")?;

        let handle_id = request
            .handle_id
            .unwrap_or_else(|| table.allocate_handle_id());
        let delegated = source.delegated(handle_id, request.delegate_to, request.grants);
        table.insert(delegated.clone());
        Ok(delegated)
    }

    fn resource_or_error(
        &self,
        resource_id: &str,
        operation: &str,
    ) -> ResourceResult<&ResourceEntry> {
        self.resources.get(resource_id).ok_or_else(|| {
            ResourceError::new(ResourceErrorSpec {
                reason: ResourceDenialReason::ResourceUnavailable,
                operation,
                handle_id: None,
                resource_id: Some(resource_id),
                type_id: None,
                holder: None,
                trust_zone: None,
                expected: vec!["registered resource".to_string()],
                actual: None,
                missing_capability: None,
            })
        })
    }

    fn handle_or_error<'table>(
        &self,
        table: &'table HandleTable,
        handle_id: &str,
        operation: &str,
    ) -> ResourceResult<&'table HandleEntry> {
        table.get(handle_id).ok_or_else(|| {
            ResourceError::new(ResourceErrorSpec {
                reason: ResourceDenialReason::HandleMissing,
                operation,
                handle_id: Some(handle_id),
                resource_id: None,
                type_id: None,
                holder: None,
                trust_zone: None,
                expected: vec!["active handle".to_string()],
                actual: None,
                missing_capability: None,
            })
        })
    }

    fn ensure_requested_grants(
        &self,
        resource: &ResourceEntry,
        grants: &[String],
        operation: &str,
    ) -> ResourceResult<()> {
        let supported = resource.supported_capabilities();
        if let Some(missing) = grants.iter().find(|grant| !supported.contains(*grant)) {
            return Err(ResourceError::new(ResourceErrorSpec {
                reason: ResourceDenialReason::MissingCapability,
                operation,
                handle_id: None,
                resource_id: Some(&resource.resource_id),
                type_id: Some(&resource.type_id),
                holder: None,
                trust_zone: Some(&resource.trust_zone),
                expected: supported.into_iter().collect(),
                actual: Some(missing),
                missing_capability: Some(missing),
            }));
        }

        Ok(())
    }

    fn ensure_active(&self, handle: &HandleEntry, operation: &str) -> ResourceResult<()> {
        match handle.revocation_state {
            HandleRevocationState::Active => Ok(()),
            HandleRevocationState::Closed => Err(handle_denial(
                ResourceDenialReason::HandleClosed,
                operation,
                handle,
            )),
            HandleRevocationState::Expired => Err(handle_denial(
                ResourceDenialReason::HandleExpired,
                operation,
                handle,
            )),
            HandleRevocationState::Revoked => Err(handle_denial(
                ResourceDenialReason::HandleRevoked,
                operation,
                handle,
            )),
            HandleRevocationState::Closing | HandleRevocationState::Poisoned => Err(handle_denial(
                ResourceDenialReason::ResourceUnavailable,
                operation,
                handle,
            )),
        }
    }

    fn ensure_handle_matches_resource(
        &self,
        handle: &HandleEntry,
        resource: &ResourceEntry,
        operation: &str,
    ) -> ResourceResult<()> {
        if handle.type_id != resource.type_id {
            return Err(ResourceError::new(ResourceErrorSpec {
                reason: ResourceDenialReason::WrongResourceType,
                operation,
                handle_id: Some(&handle.handle_id),
                resource_id: Some(&resource.resource_id),
                type_id: Some(&handle.type_id),
                holder: Some(&handle.holder),
                trust_zone: Some(&handle.trust_zone),
                expected: vec![resource.type_id.clone()],
                actual: Some(&handle.type_id),
                missing_capability: None,
            }));
        }

        if handle.trust_zone != resource.trust_zone {
            return Err(ResourceError::new(ResourceErrorSpec {
                reason: ResourceDenialReason::WrongTrustZone,
                operation,
                handle_id: Some(&handle.handle_id),
                resource_id: Some(&resource.resource_id),
                type_id: Some(&handle.type_id),
                holder: Some(&handle.holder),
                trust_zone: Some(&handle.trust_zone),
                expected: vec![resource.trust_zone.clone()],
                actual: Some(&handle.trust_zone),
                missing_capability: None,
            }));
        }

        Ok(())
    }

    fn operation_schema_or_error<'resource>(
        &self,
        resource: &'resource ResourceEntry,
        handle: &HandleEntry,
        operation: &str,
    ) -> ResourceResult<&'resource ResourceOperationSchema> {
        resource.operation_schema(operation).ok_or_else(|| {
            ResourceError::new(ResourceErrorSpec {
                reason: ResourceDenialReason::OperationUnsupported,
                operation,
                handle_id: Some(&handle.handle_id),
                resource_id: Some(&resource.resource_id),
                type_id: Some(&resource.type_id),
                holder: Some(&handle.holder),
                trust_zone: Some(&handle.trust_zone),
                expected: resource
                    .operations
                    .iter()
                    .map(|schema| schema.operation.clone())
                    .collect(),
                actual: Some(operation),
                missing_capability: None,
            })
        })
    }

    fn ensure_handle_grant(
        &self,
        handle: &HandleEntry,
        schema: &ResourceOperationSchema,
        operation: &str,
    ) -> ResourceResult<()> {
        if handle.has_grant(&schema.required_capability) {
            return Ok(());
        }

        Err(ResourceError::new(ResourceErrorSpec {
            reason: ResourceDenialReason::MissingCapability,
            operation,
            handle_id: Some(&handle.handle_id),
            resource_id: Some(&handle.resource_id),
            type_id: Some(&handle.type_id),
            holder: Some(&handle.holder),
            trust_zone: Some(&handle.trust_zone),
            expected: vec![schema.required_capability.clone()],
            actual: Some(&handle.grants.join(",")),
            missing_capability: Some(&schema.required_capability),
        }))
    }

    fn ensure_delegation_allowed(
        &self,
        source: &HandleEntry,
        requested_grants: &[String],
    ) -> ResourceResult<()> {
        if source.delegation_policy != HandleDelegationPolicy::NarrowOnly {
            return Err(handle_denial(
                ResourceDenialReason::DelegationDenied,
                "delegate",
                source,
            ));
        }

        if requested_grants.iter().all(|grant| source.has_grant(grant)) {
            return Ok(());
        }

        Err(ResourceError::new(ResourceErrorSpec {
            reason: ResourceDenialReason::DelegationDenied,
            operation: "delegate",
            handle_id: Some(&source.handle_id),
            resource_id: Some(&source.resource_id),
            type_id: Some(&source.type_id),
            holder: Some(&source.holder),
            trust_zone: Some(&source.trust_zone),
            expected: source.grants.clone(),
            actual: Some(&requested_grants.join(",")),
            missing_capability: None,
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourceOpenRequest {
    pub holder: String,
    pub resource_id: String,
    pub grants: Vec<String>,
    pub handle_id: Option<String>,
}

impl ResourceOpenRequest {
    pub fn new(
        holder: impl Into<String>,
        resource_id: impl Into<String>,
        grants: Vec<String>,
    ) -> Self {
        Self {
            holder: holder.into(),
            resource_id: resource_id.into(),
            grants,
            handle_id: None,
        }
    }

    pub fn with_handle_id(mut self, handle_id: impl Into<String>) -> Self {
        self.handle_id = Some(handle_id.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourceDelegationRequest {
    pub source_handle_id: String,
    pub delegate_to: String,
    pub grants: Vec<String>,
    pub handle_id: Option<String>,
}

impl ResourceDelegationRequest {
    pub fn new(
        source_handle_id: impl Into<String>,
        delegate_to: impl Into<String>,
        grants: Vec<String>,
    ) -> Self {
        Self {
            source_handle_id: source_handle_id.into(),
            delegate_to: delegate_to.into(),
            grants,
            handle_id: None,
        }
    }

    pub fn with_handle_id(mut self, handle_id: impl Into<String>) -> Self {
        self.handle_id = Some(handle_id.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourceOperationAuthorization {
    pub operation: String,
    pub handle_id: String,
    pub resource_id: String,
    pub type_id: String,
    pub capability: String,
    pub audit_event: ResourceAuditEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourceError {
    pub denial: ResourceDenial,
    pub audit_event: ResourceAuditEvent,
    pub diagnostic: Box<ResourceDiagnostic>,
}

impl ResourceError {
    fn new(spec: ResourceErrorSpec<'_>) -> Box<Self> {
        let operation = spec.operation.to_string();
        let handle_id = spec.handle_id.map(ToString::to_string);
        let resource_id = spec.resource_id.map(ToString::to_string);
        let type_id = spec.type_id.map(ToString::to_string);
        let holder = spec.holder.map(ToString::to_string);
        let trust_zone = spec.trust_zone.map(ToString::to_string);
        let missing_capability = spec.missing_capability.map(ToString::to_string);
        let audit_event = ResourceAuditEvent::denied(
            &operation,
            handle_id.as_deref(),
            resource_id.as_deref(),
            holder.as_deref(),
            trust_zone.as_deref(),
            spec.reason,
        );
        let denial = ResourceDenial {
            reason: spec.reason,
            operation: operation.clone(),
            handle_id,
            resource_id,
            type_id,
            holder,
            trust_zone,
            missing_capability: missing_capability.clone(),
            audit_event_id: audit_event.event_id.clone(),
        };
        let diagnostic = resource_diagnostic(
            spec.reason,
            &operation,
            spec.expected,
            spec.actual.map(ToString::to_string),
            missing_capability,
        );

        Box::new(Self {
            denial,
            audit_event,
            diagnostic,
        })
    }
}

struct ResourceErrorSpec<'value> {
    reason: ResourceDenialReason,
    operation: &'value str,
    handle_id: Option<&'value str>,
    resource_id: Option<&'value str>,
    type_id: Option<&'value str>,
    holder: Option<&'value str>,
    trust_zone: Option<&'value str>,
    expected: Vec<String>,
    actual: Option<&'value str>,
    missing_capability: Option<&'value str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourceDenial {
    pub reason: ResourceDenialReason,
    pub operation: String,
    pub handle_id: Option<String>,
    pub resource_id: Option<String>,
    pub type_id: Option<String>,
    pub holder: Option<String>,
    pub trust_zone: Option<String>,
    pub missing_capability: Option<String>,
    pub audit_event_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceDenialReason {
    HandleMissing,
    HandleExpired,
    HandleRevoked,
    HandleClosed,
    WrongResourceType,
    WrongTrustZone,
    MissingCapability,
    DelegationDenied,
    SerializationDenied,
    BudgetExhausted,
    ApprovalRequired,
    ResourceUnavailable,
    AdapterFailure,
    OperationUnsupported,
}

impl ResourceDenialReason {
    const fn code(self) -> &'static str {
        match self {
            Self::HandleMissing => "ANVIL_RESOURCE_HANDLE_MISSING",
            Self::HandleExpired => "ANVIL_RESOURCE_HANDLE_EXPIRED",
            Self::HandleRevoked => "ANVIL_RESOURCE_HANDLE_REVOKED",
            Self::HandleClosed => "ANVIL_RESOURCE_HANDLE_CLOSED",
            Self::WrongResourceType => "ANVIL_RESOURCE_WRONG_TYPE",
            Self::WrongTrustZone => "ANVIL_RESOURCE_WRONG_TRUST_ZONE",
            Self::MissingCapability => "ANVIL_RESOURCE_MISSING_CAPABILITY",
            Self::DelegationDenied => "ANVIL_RESOURCE_DELEGATION_DENIED",
            Self::SerializationDenied => "ANVIL_RESOURCE_SERIALIZATION_DENIED",
            Self::BudgetExhausted => "ANVIL_RESOURCE_BUDGET_EXHAUSTED",
            Self::ApprovalRequired => "ANVIL_RESOURCE_APPROVAL_REQUIRED",
            Self::ResourceUnavailable => "ANVIL_RESOURCE_UNAVAILABLE",
            Self::AdapterFailure => "ANVIL_RESOURCE_ADAPTER_FAILURE",
            Self::OperationUnsupported => "ANVIL_RESOURCE_OPERATION_UNSUPPORTED",
        }
    }

    const fn summary(self) -> &'static str {
        match self {
            Self::HandleMissing => "resource handle was not found",
            Self::HandleExpired => "resource handle has expired",
            Self::HandleRevoked => "resource handle has been revoked",
            Self::HandleClosed => "resource handle is closed",
            Self::WrongResourceType => "resource handle type does not match resource",
            Self::WrongTrustZone => "resource handle is not valid in this trust zone",
            Self::MissingCapability => "resource handle is missing a required capability",
            Self::DelegationDenied => "resource handle delegation is not allowed",
            Self::SerializationDenied => "resource handle cannot be serialized here",
            Self::BudgetExhausted => "resource budget is exhausted",
            Self::ApprovalRequired => "resource operation requires approval",
            Self::ResourceUnavailable => "resource is unavailable",
            Self::AdapterFailure => "resource adapter failed",
            Self::OperationUnsupported => "resource operation is not supported",
        }
    }

    const fn suggestion(self) -> &'static str {
        match self {
            Self::HandleMissing => "Open or import the resource before using it.",
            Self::HandleExpired => "Request a fresh handle for this resource.",
            Self::HandleRevoked => "Request a new grant or inspect the revocation event.",
            Self::HandleClosed => "Open the resource again if continued access is intended.",
            Self::WrongResourceType => "Use a handle with the expected resource type.",
            Self::WrongTrustZone => "Run in the matching trust zone or request delegation.",
            Self::MissingCapability => "Request a handle with the required capability.",
            Self::DelegationDenied => "Delegate a narrowed handle that policy allows.",
            Self::SerializationDenied => "Pass a resource requirement instead of a live handle.",
            Self::BudgetExhausted => "Retry with a larger budget or a smaller resource operation.",
            Self::ApprovalRequired => "Submit the operation as a reviewable request.",
            Self::ResourceUnavailable => "Inspect the resource registry or retry later.",
            Self::AdapterFailure => "Inspect the resource adapter diagnostic.",
            Self::OperationUnsupported => "Use one of the supported resource operations.",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourceAuditEvent {
    pub event_id: String,
    pub kind: ResourceAuditKind,
    pub decision: ResourceAuditDecision,
    pub operation: String,
    pub handle_id: Option<String>,
    pub resource_id: Option<String>,
    pub holder: Option<String>,
    pub trust_zone: Option<String>,
    pub reason: Option<ResourceDenialReason>,
}

impl ResourceAuditEvent {
    fn allowed(
        operation: &str,
        handle_id: &str,
        resource_id: &str,
        holder: &str,
        trust_zone: &str,
    ) -> Self {
        Self {
            event_id: audit_event_id(ResourceAuditDecision::Allowed, operation, Some(handle_id)),
            kind: ResourceAuditKind::OperationAllowed,
            decision: ResourceAuditDecision::Allowed,
            operation: operation.to_string(),
            handle_id: Some(handle_id.to_string()),
            resource_id: Some(resource_id.to_string()),
            holder: Some(holder.to_string()),
            trust_zone: Some(trust_zone.to_string()),
            reason: None,
        }
    }

    fn denied(
        operation: &str,
        handle_id: Option<&str>,
        resource_id: Option<&str>,
        holder: Option<&str>,
        trust_zone: Option<&str>,
        reason: ResourceDenialReason,
    ) -> Self {
        Self {
            event_id: audit_event_id(ResourceAuditDecision::Denied, operation, handle_id),
            kind: ResourceAuditKind::OperationDenied,
            decision: ResourceAuditDecision::Denied,
            operation: operation.to_string(),
            handle_id: handle_id.map(ToString::to_string),
            resource_id: resource_id.map(ToString::to_string),
            holder: holder.map(ToString::to_string),
            trust_zone: trust_zone.map(ToString::to_string),
            reason: Some(reason),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceAuditKind {
    OperationAllowed,
    OperationDenied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceAuditDecision {
    Allowed,
    Denied,
}

fn handle_denial(
    reason: ResourceDenialReason,
    operation: &str,
    handle: &HandleEntry,
) -> Box<ResourceError> {
    ResourceError::new(ResourceErrorSpec {
        reason,
        operation,
        handle_id: Some(&handle.handle_id),
        resource_id: Some(&handle.resource_id),
        type_id: Some(&handle.type_id),
        holder: Some(&handle.holder),
        trust_zone: Some(&handle.trust_zone),
        expected: vec!["active handle".to_string()],
        actual: Some(revocation_state_name(handle.revocation_state)),
        missing_capability: None,
    })
}

fn resource_diagnostic(
    reason: ResourceDenialReason,
    operation: &str,
    expected: Vec<String>,
    actual: Option<String>,
    missing_capability: Option<String>,
) -> Box<ResourceDiagnostic> {
    let source = SourceText::new("resource", operation);
    let span = SourceSpan::point(SourceLocation::start());
    let mut expected = expected;
    if let Some(capability) = &missing_capability {
        expected.push(format!("capability:{capability}"));
    }

    Diagnostic::new(DiagnosticSpec {
        code: reason.code(),
        phase: DiagnosticPhase::Resource,
        source: &source,
        message: reason.summary().to_string(),
        span,
        expected,
        actual,
        suggestion: Some(reason.suggestion().to_string()),
    })
}

fn audit_event_id(
    decision: ResourceAuditDecision,
    operation: &str,
    handle_id: Option<&str>,
) -> String {
    let decision = match decision {
        ResourceAuditDecision::Allowed => "allow",
        ResourceAuditDecision::Denied => "deny",
    };
    let handle_id = handle_id.unwrap_or("resource");
    format!(
        "audit:{decision}:{}:{}",
        sanitize_audit_part(operation),
        sanitize_audit_part(handle_id)
    )
}

fn sanitize_audit_part(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn revocation_state_name(state: HandleRevocationState) -> &'static str {
    match state {
        HandleRevocationState::Active => "active",
        HandleRevocationState::Closing => "closing",
        HandleRevocationState::Closed => "closed",
        HandleRevocationState::Expired => "expired",
        HandleRevocationState::Revoked => "revoked",
        HandleRevocationState::Poisoned => "poisoned",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_typed_handles_with_redacted_display() {
        let registry = sample_registry();
        let mut table = HandleTable::new();
        let handle = registry
            .open_handle(
                &mut table,
                ResourceOpenRequest::new(
                    "agent.alpha",
                    "markodb:papers",
                    vec!["read".to_string(), "inspect".to_string()],
                ),
            )
            .expect("handle");

        assert_eq!(handle.handle_id, "handle-1");
        assert_eq!(handle.type_id, "markodb.collection");
        assert!(handle.has_grant("read"));
        assert!(!handle.display_summary().contains("handle-1"));
    }

    #[test]
    fn denies_missing_capability_at_use_site() {
        let registry = sample_registry();
        let mut table = HandleTable::new();
        let handle = registry
            .open_handle(
                &mut table,
                ResourceOpenRequest::new("agent.alpha", "markodb:papers", vec!["read".into()]),
            )
            .expect("handle");

        let error = registry
            .check_operation(&table, &handle.handle_id, "write")
            .expect_err("missing write grant");

        assert_eq!(error.denial.reason, ResourceDenialReason::MissingCapability);
        assert_eq!(error.audit_event.decision, ResourceAuditDecision::Denied);
        assert_eq!(error.diagnostic.phase, DiagnosticPhase::Resource);
    }

    #[test]
    fn delegates_by_narrowing_grants() {
        let registry = sample_registry();
        let mut table = HandleTable::new();
        let handle = registry
            .open_handle(
                &mut table,
                ResourceOpenRequest::new(
                    "agent.alpha",
                    "markodb:papers",
                    vec!["read".into(), "write".into()],
                ),
            )
            .expect("handle");

        let delegated = registry
            .delegate_handle(
                &mut table,
                ResourceDelegationRequest::new(
                    &handle.handle_id,
                    "actor.worker",
                    vec!["read".into()],
                ),
            )
            .expect("delegated handle");

        assert_eq!(delegated.holder, "actor.worker");
        assert!(delegated.has_grant("read"));
        assert!(!delegated.has_grant("write"));
    }

    #[test]
    fn rejects_delegation_that_widens_authority() {
        let registry = sample_registry();
        let mut table = HandleTable::new();
        let handle = registry
            .open_handle(
                &mut table,
                ResourceOpenRequest::new("agent.alpha", "markodb:papers", vec!["read".into()]),
            )
            .expect("handle");

        let error = registry
            .delegate_handle(
                &mut table,
                ResourceDelegationRequest::new(
                    &handle.handle_id,
                    "actor.worker",
                    vec!["read".into(), "write".into()],
                ),
            )
            .expect_err("delegation denied");

        assert_eq!(error.denial.reason, ResourceDenialReason::DelegationDenied);
    }

    #[test]
    fn revocation_blocks_future_operations() {
        let registry = sample_registry();
        let mut table = HandleTable::new();
        let handle = registry
            .open_handle(
                &mut table,
                ResourceOpenRequest::new("agent.alpha", "markodb:papers", vec!["read".into()]),
            )
            .expect("handle");
        table.revoke(&handle.handle_id).expect("revoked");

        let error = registry
            .check_operation(&table, &handle.handle_id, "read")
            .expect_err("revoked handle");

        assert_eq!(error.denial.reason, ResourceDenialReason::HandleRevoked);
    }

    fn sample_registry() -> ResourceRegistry {
        let mut registry = ResourceRegistry::new();
        registry.register(
            ResourceEntry::new(
                "markodb:papers",
                "markodb.collection",
                "runtime",
                "project.markodb",
            )
            .with_operation("read", "read")
            .with_operation("write", "write")
            .with_operation("inspect", "inspect")
            .with_delegation_policy(HandleDelegationPolicy::NarrowOnly),
        );
        registry
    }
}
