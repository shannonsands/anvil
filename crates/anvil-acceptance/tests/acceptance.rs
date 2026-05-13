use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use anvil_core::{
    AnvilManifest, AstDiagnostic, CapabilityProfile, DraftOverlay, EmbeddedRuntime,
    EmbeddedRuntimeSnapshot, EvalResponse, HandleDelegationPolicy, HandleEntry, HandleTable,
    HostCallContext, HostCallFailure, HostFunctionSpec, ManifestDiagnostic, ModuleDiagnostic,
    ModuleResolution, ModuleResolver, ModuleRootKind, ModuleSession, PackageSnapshot,
    PackageSourceFile, ProjectDiagnostic, ReplInteraction, ReplResponse, ReplSession,
    ResourceAdapter, ResourceAdapterFailure, ResourceAdapterOutcome, ResourceAdapterRequest,
    ResourceAdapterResult, ResourceDelegationRequest, ResourceEffect, ResourceEffectRecord,
    ResourceEntry, ResourceError, ResourceExecutionMode, ResourceOpenRequest,
    ResourceOperationAuthorization, ResourceOperationOutcome, ResourceOperationRequest,
    ResourceRegistry, ResponseOptions, SpannedAst, SyntaxDiagnostic, SyntaxObject, Value, Vm,
    VmBudget, VmDiagnostic, VmOutput, VmSession, compile_source, format_ast, format_datums,
    load_package_snapshot, load_workspace_snapshot, lower_source, lower_source_with_resolver,
    parse_manifest, read_repl_input, run_source, syntax_from_source,
};
use cucumber::{World as _, gherkin::Step, given, then, when};

#[derive(Debug, Default, cucumber::World)]
struct AnvilWorld {
    response: Option<String>,
    source: String,
    repl_response: Option<ReplResponse>,
    repl_session: ReplSession,
    repl_interaction: Option<ReplInteraction>,
    json_response: Option<serde_json::Value>,
    rendered_diagnostic: Option<String>,
    ast: Option<Vec<SpannedAst>>,
    ast_diagnostic: Option<Box<AstDiagnostic>>,
    syntax_objects: Option<Vec<SyntaxObject>>,
    syntax_diagnostic: Option<Box<SyntaxDiagnostic>>,
    module_resolver: ModuleResolver,
    module_resolution: Option<ModuleResolution>,
    module_diagnostic: Option<Box<ModuleDiagnostic>>,
    draft_overlay: Option<DraftOverlay>,
    manifest_source: String,
    manifest: Option<AnvilManifest>,
    manifest_diagnostic: Option<Box<ManifestDiagnostic>>,
    package_sources: Vec<PackageSourceFile>,
    filesystem_package_root: Option<PathBuf>,
    project_diagnostic: Option<Box<ProjectDiagnostic>>,
    module_session: ModuleSession,
    vm_session: VmSession,
    vm_output: Option<VmOutput>,
    vm_diagnostic: Option<Box<VmDiagnostic>>,
    eval_response: Option<EvalResponse>,
    embedded_runtime: Option<EmbeddedRuntime>,
    embedded_snapshot: Option<EmbeddedRuntimeSnapshot>,
    resource_registry: ResourceRegistry,
    resource_handle_table: HandleTable,
    resource_handle: Option<HandleEntry>,
    delegated_resource_handle: Option<HandleEntry>,
    resource_authorization: Option<ResourceOperationAuthorization>,
    resource_adapter: Option<RecordingResourceAdapter>,
    resource_operation_outcome: Option<ResourceOperationOutcome>,
    resource_error: Option<Box<ResourceError>>,
    capability_profile: Option<CapabilityProfile>,
    host_function_calls: Arc<AtomicUsize>,
}

impl Drop for AnvilWorld {
    fn drop(&mut self) {
        if let Some(root) = &self.filesystem_package_root {
            let _ = fs::remove_dir_all(root);
        }
    }
}

#[given("a fresh Anvil planning scaffold")]
async fn fresh_scaffold(world: &mut AnvilWorld) {
    world.response = None;
}

#[given("a fresh module resolver")]
async fn fresh_module_resolver(world: &mut AnvilWorld) {
    world.module_resolver = ModuleResolver::new();
    world.module_resolution = None;
    world.module_diagnostic = None;
    world.draft_overlay = None;
}

#[given("a fresh resource registry")]
async fn fresh_resource_registry(world: &mut AnvilWorld) {
    world.resource_registry = ResourceRegistry::new();
    world.resource_handle_table = HandleTable::new();
    world.resource_handle = None;
    world.delegated_resource_handle = None;
    world.resource_authorization = None;
    world.resource_adapter = None;
    world.resource_operation_outcome = None;
    world.resource_error = None;
    world.capability_profile = None;
}

#[given(
    expr = "resource {string} of type {string} exists in trust zone {string} with operations {string}"
)]
async fn resource_exists_with_operations(
    world: &mut AnvilWorld,
    resource_id: String,
    type_id: String,
    trust_zone: String,
    operations: String,
) {
    let mut resource = ResourceEntry::new(resource_id, type_id, "runtime", trust_zone)
        .with_delegation_policy(HandleDelegationPolicy::NarrowOnly);
    for operation in split_csv(&operations) {
        resource.add_operation(operation.clone(), operation);
    }
    world.resource_registry.register(resource);
}

#[given(
    expr = "embedded resource {string} of type {string} exists in trust zone {string} with operations {string}"
)]
async fn embedded_resource_exists_with_operations(
    world: &mut AnvilWorld,
    resource_id: String,
    type_id: String,
    trust_zone: String,
    operations: String,
) {
    let mut resource = ResourceEntry::new(resource_id, type_id, "runtime", trust_zone)
        .with_delegation_policy(HandleDelegationPolicy::NarrowOnly);
    for operation in split_csv(&operations) {
        resource.add_operation(operation.clone(), operation);
    }
    embedded_runtime(world).register_resource(resource);
}

#[given(
    expr = "resource {string} of type {string} exists in trust zone {string} with operation {string} requiring capability {string}"
)]
async fn resource_exists_with_operation_and_capability(
    world: &mut AnvilWorld,
    resource_id: String,
    type_id: String,
    trust_zone: String,
    operation: String,
    capability: String,
) {
    let resource = ResourceEntry::new(resource_id, type_id, "runtime", trust_zone)
        .with_operation(operation, capability)
        .with_delegation_policy(HandleDelegationPolicy::NarrowOnly);
    world.resource_registry.register(resource);
}

#[given(
    expr = "capability profile {string} for principal {string} in trust zone {string} with capabilities {string}"
)]
async fn capability_profile_for_principal(
    world: &mut AnvilWorld,
    profile_id: String,
    principal: String,
    trust_zone: String,
    capabilities: String,
) {
    world.capability_profile = Some(
        CapabilityProfile::new(profile_id, principal, trust_zone)
            .with_capabilities(split_csv(&capabilities)),
    );
}

#[given(expr = "resource adapter {string} handles type {string} with operations {string}")]
async fn resource_adapter_handles_type(
    world: &mut AnvilWorld,
    adapter_id: String,
    type_id: String,
    operations: String,
) {
    world.resource_adapter = Some(RecordingResourceAdapter::new(
        adapter_id,
        type_id,
        split_csv(&operations),
    ));
    world.resource_operation_outcome = None;
    world.resource_error = None;
}

#[given(expr = "the resource adapter will fail with {string}")]
async fn resource_adapter_will_fail(world: &mut AnvilWorld, message: String) {
    world
        .resource_adapter
        .as_mut()
        .expect("resource adapter")
        .failure = Some(ResourceAdapterFailure::new(message).with_expected("adapter result"));
}

#[given(expr = "a fresh draft overlay {string} owned by {string}")]
async fn fresh_draft_overlay(world: &mut AnvilWorld, draft_id: String, owner: String) {
    world.draft_overlay = Some(DraftOverlay::new(draft_id, owner));
}

#[when("the agent asks for the project shape")]
async fn asks_for_project_shape(world: &mut AnvilWorld) {
    let shape = anvil_core::project_shape();
    world.response = Some(format!("{}: {}", shape.name, shape.status));
}

#[then("the response says Anvil is in phase 0 planning")]
async fn response_says_phase_zero(world: &mut AnvilWorld) {
    assert_eq!(
        world.response.as_deref(),
        Some("Anvil: phase 0 planning scaffold"),
    );
}

#[given(expr = "the agent input {string}")]
async fn agent_input(world: &mut AnvilWorld, source: String) {
    world.source = source;
    world.repl_response = None;
    world.json_response = None;
    world.ast = None;
    world.ast_diagnostic = None;
    world.syntax_objects = None;
    world.syntax_diagnostic = None;
    world.vm_output = None;
    world.vm_diagnostic = None;
    world.eval_response = None;
    world.resource_error = None;
}

#[given("the agent input")]
async fn agent_doc_input(world: &mut AnvilWorld, #[step] step: &Step) {
    world.source = trim_docstring(step.docstring().expect("agent input docstring")).to_string();
    world.repl_response = None;
    world.json_response = None;
    world.ast = None;
    world.ast_diagnostic = None;
    world.syntax_objects = None;
    world.syntax_diagnostic = None;
    world.vm_output = None;
    world.vm_diagnostic = None;
    world.eval_response = None;
}

#[given("the manifest input")]
async fn manifest_input(world: &mut AnvilWorld, #[step] step: &Step) {
    world.manifest_source = trim_docstring(step.docstring().expect("manifest input")).to_string();
    world.manifest = None;
    world.manifest_diagnostic = None;
    world.package_sources.clear();
}

#[given(expr = "package source {string} contains {string}")]
async fn package_source_contains(world: &mut AnvilWorld, path: String, source: String) {
    world
        .package_sources
        .push(PackageSourceFile { path, source });
}

#[given("an empty filesystem package")]
async fn empty_filesystem_package(world: &mut AnvilWorld) {
    world.filesystem_package_root = Some(create_temp_package_root());
    world.project_diagnostic = None;
}

#[given("a filesystem package with manifest")]
async fn filesystem_package_with_manifest(world: &mut AnvilWorld, #[step] step: &Step) {
    let root = create_temp_package_root();
    write_package_file(
        &root,
        "Anvil.toml",
        trim_docstring(step.docstring().expect("manifest input")),
    );
    world.filesystem_package_root = Some(root);
    world.project_diagnostic = None;
}

#[given(expr = "filesystem package source {string} contains {string}")]
async fn filesystem_package_source_contains(world: &mut AnvilWorld, path: String, source: String) {
    let root = world
        .filesystem_package_root
        .as_ref()
        .expect("filesystem package root");

    write_package_file(root, &path, &source);
}

#[given(expr = "filesystem package file {string}")]
async fn filesystem_package_file(world: &mut AnvilWorld, path: String, #[step] step: &Step) {
    let root = world
        .filesystem_package_root
        .as_ref()
        .expect("filesystem package root");
    let source = trim_docstring(step.docstring().expect("filesystem package file"));

    write_package_file(root, &path, source);
}

#[when(expr = "holder {string} opens resource {string} with grants {string}")]
async fn holder_opens_resource(
    world: &mut AnvilWorld,
    holder: String,
    resource_id: String,
    grants: String,
) {
    match world.resource_registry.open_handle(
        &mut world.resource_handle_table,
        ResourceOpenRequest::new(holder, resource_id, split_csv(&grants)),
    ) {
        Ok(handle) => {
            world.resource_handle = Some(handle);
            world.resource_error = None;
        }
        Err(error) => {
            world.resource_handle = None;
            world.resource_error = Some(error);
        }
    }
}

#[when(
    expr = "holder {string} opens resource {string} under the capability profile with grants {string}"
)]
async fn holder_opens_resource_under_capability_profile(
    world: &mut AnvilWorld,
    holder: String,
    resource_id: String,
    grants: String,
) {
    let profile = world
        .capability_profile
        .clone()
        .expect("capability profile");
    match world.resource_registry.open_handle_with_profile(
        &mut world.resource_handle_table,
        &profile,
        ResourceOpenRequest::new(holder, resource_id, split_csv(&grants)),
    ) {
        Ok(handle) => {
            world.resource_handle = Some(handle);
            world.resource_error = None;
        }
        Err(error) => {
            world.resource_handle = None;
            world.resource_error = Some(error);
        }
    }
}

#[when(expr = "the holder uses the resource handle for operation {string}")]
async fn holder_uses_resource_handle(world: &mut AnvilWorld, operation: String) {
    let handle_id = world
        .resource_handle
        .as_ref()
        .expect("resource handle")
        .handle_id
        .clone();
    match world.resource_registry.check_operation(
        &world.resource_handle_table,
        &handle_id,
        &operation,
    ) {
        Ok(authorization) => {
            world.resource_authorization = Some(authorization);
            world.resource_error = None;
        }
        Err(error) => {
            world.resource_authorization = None;
            world.resource_error = Some(error);
        }
    }
}

#[when(
    expr = "the holder executes resource operation {string} through the adapter returning {string}"
)]
async fn holder_executes_resource_operation_through_adapter(
    world: &mut AnvilWorld,
    operation: String,
    value: String,
) {
    let handle_id = world
        .resource_handle
        .as_ref()
        .expect("resource handle")
        .handle_id
        .clone();
    let adapter = world.resource_adapter.as_mut().expect("resource adapter");
    adapter.value = Value::String(value);

    match world.resource_registry.execute_operation(
        &world.resource_handle_table,
        adapter,
        ResourceOperationRequest::new(handle_id, operation),
    ) {
        Ok(outcome) => {
            world.resource_operation_outcome = Some(outcome);
            world.resource_error = None;
        }
        Err(error) => {
            world.resource_operation_outcome = None;
            world.resource_error = Some(error);
        }
    }
}

#[when(
    expr = "the holder executes resource operation {string} through the adapter under the capability profile returning {string}"
)]
async fn holder_executes_resource_operation_through_adapter_under_capability_profile(
    world: &mut AnvilWorld,
    operation: String,
    value: String,
) {
    let profile = world
        .capability_profile
        .clone()
        .expect("capability profile");
    let handle_id = world
        .resource_handle
        .as_ref()
        .expect("resource handle")
        .handle_id
        .clone();
    let adapter = world.resource_adapter.as_mut().expect("resource adapter");
    adapter.value = Value::String(value);

    match world.resource_registry.execute_operation_with_profile(
        &world.resource_handle_table,
        &profile,
        adapter,
        ResourceOperationRequest::new(handle_id, operation),
    ) {
        Ok(outcome) => {
            world.resource_operation_outcome = Some(outcome);
            world.resource_error = None;
        }
        Err(error) => {
            world.resource_operation_outcome = None;
            world.resource_error = Some(error);
        }
    }
}

#[when(expr = "the holder delegates the resource handle to {string} with grants {string}")]
async fn holder_delegates_resource_handle(
    world: &mut AnvilWorld,
    delegate_to: String,
    grants: String,
) {
    let handle_id = world
        .resource_handle
        .as_ref()
        .expect("resource handle")
        .handle_id
        .clone();
    match world.resource_registry.delegate_handle(
        &mut world.resource_handle_table,
        ResourceDelegationRequest::new(handle_id, delegate_to, split_csv(&grants)),
    ) {
        Ok(handle) => {
            world.delegated_resource_handle = Some(handle);
            world.resource_error = None;
        }
        Err(error) => {
            world.delegated_resource_handle = None;
            world.resource_error = Some(error);
        }
    }
}

#[when(
    expr = "the holder delegates the resource handle to {string} under the capability profile with grants {string}"
)]
async fn holder_delegates_resource_handle_under_capability_profile(
    world: &mut AnvilWorld,
    delegate_to: String,
    grants: String,
) {
    let profile = world
        .capability_profile
        .clone()
        .expect("capability profile");
    let handle_id = world
        .resource_handle
        .as_ref()
        .expect("resource handle")
        .handle_id
        .clone();
    match world.resource_registry.delegate_handle_with_profile(
        &mut world.resource_handle_table,
        &profile,
        ResourceDelegationRequest::new(handle_id, delegate_to, split_csv(&grants)),
    ) {
        Ok(handle) => {
            world.delegated_resource_handle = Some(handle);
            world.resource_error = None;
        }
        Err(error) => {
            world.delegated_resource_handle = None;
            world.resource_error = Some(error);
        }
    }
}

#[when("the supervisor revokes the resource handle")]
async fn supervisor_revokes_resource_handle(world: &mut AnvilWorld) {
    let handle_id = world
        .resource_handle
        .as_ref()
        .expect("resource handle")
        .handle_id
        .clone();

    match world.resource_handle_table.revoke(&handle_id) {
        Ok(handle) => {
            world.resource_handle = Some(handle);
            world.resource_error = None;
        }
        Err(error) => {
            world.resource_error = Some(error);
        }
    }
}

#[when("the capability profile revokes the resource handle")]
async fn capability_profile_revokes_resource_handle(world: &mut AnvilWorld) {
    let profile = world
        .capability_profile
        .clone()
        .expect("capability profile");
    let handle_id = world
        .resource_handle
        .as_ref()
        .expect("resource handle")
        .handle_id
        .clone();

    match world.resource_registry.revoke_handle_with_profile(
        &mut world.resource_handle_table,
        &profile,
        &handle_id,
    ) {
        Ok(handle) => {
            world.resource_handle = Some(handle);
            world.resource_error = None;
        }
        Err(error) => {
            world.resource_error = Some(error);
        }
    }
}

#[when("the reader-backed REPL reads the input")]
async fn reader_repl_reads_input(world: &mut AnvilWorld) {
    world.repl_response = Some(read_repl_input(&world.source));
}

#[when("the bytecode VM runs the input")]
async fn bytecode_vm_runs_input(world: &mut AnvilWorld) {
    match run_source(&world.source) {
        Ok(output) => {
            world.vm_output = Some(output);
            world.vm_diagnostic = None;
        }
        Err(diagnostic) => {
            world.vm_output = None;
            world.vm_diagnostic = Some(diagnostic);
        }
    }
}

#[given("a fresh VM session")]
async fn fresh_vm_session(world: &mut AnvilWorld) {
    world.vm_session = VmSession::new();
    world.vm_output = None;
    world.vm_diagnostic = None;
    world.eval_response = None;
    reset_host_call_count(world);
}

#[given("a fresh module session")]
async fn fresh_module_session(world: &mut AnvilWorld) {
    world.module_session = ModuleSession::new();
    world.vm_output = None;
    world.vm_diagnostic = None;
    world.module_diagnostic = None;
    world.eval_response = None;
    reset_host_call_count(world);
}

#[given(expr = "a fresh embedded runtime {string}")]
async fn fresh_embedded_runtime(world: &mut AnvilWorld, runtime_id: String) {
    world.embedded_runtime = Some(EmbeddedRuntime::new(runtime_id));
    world.embedded_snapshot = None;
    world.eval_response = None;
    world.resource_handle = None;
    world.resource_error = None;
    reset_host_call_count(world);
}

#[given(expr = "host function {string} is registered")]
async fn host_function_is_registered(world: &mut AnvilWorld, name: String) {
    register_host_add(
        &mut world.vm_session,
        &name,
        Arc::clone(&world.host_function_calls),
    );
    register_module_host_add(
        &mut world.module_session,
        &name,
        Arc::clone(&world.host_function_calls),
    );
}

#[given(
    expr = "host function {string} requiring capability {string} in trust zone {string} is registered"
)]
async fn host_function_requiring_capability_is_registered(
    world: &mut AnvilWorld,
    name: String,
    capability: String,
    trust_zone: String,
) {
    let spec = HostFunctionSpec::new(name.clone())
        .with_exact_arity(0)
        .with_required_capability(capability)
        .with_trust_zone(trust_zone);
    let calls = Arc::clone(&world.host_function_calls);
    world
        .vm_session
        .register_host_function(spec.clone(), move |_context, _args| {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok(Value::Keyword("authorized".to_string()))
        });

    let calls = Arc::clone(&world.host_function_calls);
    world
        .module_session
        .register_host_function(spec, move |_context, _args| {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok(Value::Keyword("authorized".to_string()))
        });
}

#[given(expr = "embedded host function {string} is registered")]
async fn embedded_host_function_is_registered(world: &mut AnvilWorld, name: String) {
    let calls = Arc::clone(&world.host_function_calls);
    embedded_runtime(world).register_host_function(
        HostFunctionSpec::new(name).with_exact_arity(2),
        move |_context: &HostCallContext, args: &[Value]| {
            calls.fetch_add(1, Ordering::Relaxed);
            host_add(args)
        },
    );
}

#[given(
    expr = "embedded host function {string} requiring capability {string} in trust zone {string} is registered"
)]
async fn embedded_host_function_requiring_capability_is_registered(
    world: &mut AnvilWorld,
    name: String,
    capability: String,
    trust_zone: String,
) {
    let calls = Arc::clone(&world.host_function_calls);
    embedded_runtime(world).register_host_function(
        HostFunctionSpec::new(name)
            .with_exact_arity(0)
            .with_required_capability(capability)
            .with_trust_zone(trust_zone),
        move |_context, _args| {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok(Value::Keyword("authorized".to_string()))
        },
    );
}

#[given(expr = "failing host function {string} is registered with message {string}")]
async fn failing_host_function_is_registered(
    world: &mut AnvilWorld,
    name: String,
    message: String,
) {
    let spec = HostFunctionSpec::new(name).with_exact_arity(0);
    let calls = Arc::clone(&world.host_function_calls);
    let vm_message = message.clone();
    world
        .vm_session
        .register_host_function(spec.clone(), move |_context, _args| {
            calls.fetch_add(1, Ordering::Relaxed);
            Err(HostCallFailure::new(vm_message.clone()))
        });

    let calls = Arc::clone(&world.host_function_calls);
    world
        .module_session
        .register_host_function(spec, move |_context, _args| {
            calls.fetch_add(1, Ordering::Relaxed);
            Err(HostCallFailure::new(message.clone()))
        });
}

#[given(
    expr = "the VM session uses capability profile {string} for principal {string} in trust zone {string} with capabilities {string}"
)]
async fn vm_session_uses_capability_profile(
    world: &mut AnvilWorld,
    profile_id: String,
    principal: String,
    trust_zone: String,
    capabilities: String,
) {
    world.vm_session.set_capability_profile(capability_profile(
        profile_id,
        principal,
        trust_zone,
        capabilities,
    ));
}

#[given(
    expr = "the module session uses capability profile {string} for principal {string} in trust zone {string} with capabilities {string}"
)]
async fn module_session_uses_capability_profile(
    world: &mut AnvilWorld,
    profile_id: String,
    principal: String,
    trust_zone: String,
    capabilities: String,
) {
    world
        .module_session
        .set_capability_profile(capability_profile(
            profile_id,
            principal,
            trust_zone,
            capabilities,
        ));
}

#[given(
    expr = "embedded capability profile {string} for principal {string} in trust zone {string} with capabilities {string}"
)]
async fn embedded_capability_profile(
    world: &mut AnvilWorld,
    profile_id: String,
    principal: String,
    trust_zone: String,
    capabilities: String,
) {
    embedded_runtime(world).register_profile(capability_profile(
        profile_id,
        principal,
        trust_zone,
        capabilities,
    ));
}

#[given(expr = "embedded composed capability profile {string} from profiles {string}")]
async fn embedded_composed_capability_profile(
    world: &mut AnvilWorld,
    profile_id: String,
    component_ids: String,
) {
    embedded_runtime(world)
        .register_composed_profile(profile_id.clone(), split_csv(&component_ids))
        .unwrap_or_else(|error| panic!("compose profile {profile_id}: {error:?}"));
}

#[when("the VM session evaluates the input")]
async fn vm_session_evaluates_input(world: &mut AnvilWorld) {
    match world.vm_session.eval_source(&world.source) {
        Ok(output) => {
            world.vm_output = Some(output);
            world.vm_diagnostic = None;
        }
        Err(diagnostic) => {
            world.vm_output = None;
            world.vm_diagnostic = Some(diagnostic);
        }
    }
}

#[when(expr = "the VM session evaluates {string}")]
async fn vm_session_evaluates_source(world: &mut AnvilWorld, source: String) {
    match world.vm_session.eval_source(&source) {
        Ok(output) => {
            world.vm_output = Some(output);
            world.vm_diagnostic = None;
        }
        Err(diagnostic) => {
            world.vm_output = None;
            world.vm_diagnostic = Some(diagnostic);
        }
    }
}

#[when(expr = "the VM session evaluates {string} as a response envelope")]
async fn vm_session_evaluates_source_as_response_envelope(world: &mut AnvilWorld, source: String) {
    world.eval_response = Some(world.vm_session.eval_response(&source));
}

#[when(expr = "the VM session evaluates {string} as a debug response envelope")]
async fn vm_session_evaluates_source_as_debug_response_envelope(
    world: &mut AnvilWorld,
    source: String,
) {
    world.eval_response = Some(
        world
            .vm_session
            .eval_response_with_options(&source, ResponseOptions::debug()),
    );
}

#[when(expr = "the embedded runtime evaluates {string}")]
async fn embedded_runtime_evaluates_source(world: &mut AnvilWorld, source: String) {
    world.eval_response = Some(embedded_runtime(world).eval(&source));
}

#[when("the embedded runtime facade is inspected")]
async fn embedded_runtime_facade_is_inspected(world: &mut AnvilWorld) {
    world.embedded_snapshot = Some(embedded_runtime(world).snapshot());
}

#[when(expr = "the embedded runtime activates profile {string}")]
async fn embedded_runtime_activates_profile(world: &mut AnvilWorld, profile_id: String) {
    embedded_runtime(world)
        .activate_profile(&profile_id)
        .unwrap_or_else(|error| panic!("activate profile {profile_id}: {error:?}"));
}

#[when(expr = "the embedded runtime opens resource {string} with grants {string}")]
async fn embedded_runtime_opens_resource_with_grants(
    world: &mut AnvilWorld,
    resource_id: String,
    grants: String,
) {
    let result = embedded_runtime(world).open_resource(resource_id, split_csv(&grants));
    match result {
        Ok(handle) => {
            world.resource_handle = Some(handle);
            world.resource_error = None;
        }
        Err(error) => {
            world.resource_handle = None;
            world.resource_error = Some(error);
        }
    }
}

#[when(expr = "the VM session evaluates the input with {int} instruction fuel")]
async fn vm_session_evaluates_input_with_instruction_fuel(world: &mut AnvilWorld, fuel: usize) {
    match world
        .vm_session
        .eval_source_with_budget(&world.source, VmBudget::with_instruction_fuel(fuel))
    {
        Ok(output) => {
            world.vm_output = Some(output);
            world.vm_diagnostic = None;
        }
        Err(diagnostic) => {
            world.vm_output = None;
            world.vm_diagnostic = Some(diagnostic);
        }
    }
}

#[when("the module session evaluates the input")]
async fn module_session_evaluates_input(world: &mut AnvilWorld) {
    match world.module_session.eval_source(&world.source) {
        Ok(output) => {
            world.vm_output = Some(output);
            world.vm_diagnostic = None;
            world.module_diagnostic = None;
        }
        Err(diagnostic) => {
            world.vm_output = None;
            world.module_diagnostic = module_diagnostic_clone(&diagnostic);
            world.vm_diagnostic = Some(diagnostic);
        }
    }
}

#[when(expr = "the module session evaluates {string}")]
async fn module_session_evaluates_source(world: &mut AnvilWorld, source: String) {
    match world.module_session.eval_source(&source) {
        Ok(output) => {
            world.vm_output = Some(output);
            world.vm_diagnostic = None;
            world.module_diagnostic = None;
        }
        Err(diagnostic) => {
            world.vm_output = None;
            world.module_diagnostic = module_diagnostic_clone(&diagnostic);
            world.vm_diagnostic = Some(diagnostic);
        }
    }
}

#[when(expr = "the bytecode VM runs the input with {int} instruction fuel")]
async fn bytecode_vm_runs_input_with_instruction_fuel(world: &mut AnvilWorld, fuel: usize) {
    match compile_source(&world.source)
        .and_then(|program| Vm::with_budget(VmBudget::with_instruction_fuel(fuel)).run(&program))
    {
        Ok(output) => {
            world.vm_output = Some(output);
            world.vm_diagnostic = None;
        }
        Err(diagnostic) => {
            world.vm_output = None;
            world.vm_diagnostic = Some(diagnostic);
        }
    }
}

#[when("the syntax object layer wraps the input")]
async fn syntax_object_layer_wraps_input(world: &mut AnvilWorld) {
    match syntax_from_source(&world.source) {
        Ok(objects) => {
            world.syntax_objects = Some(objects);
            world.syntax_diagnostic = None;
        }
        Err(diagnostic) => {
            world.syntax_objects = None;
            world.syntax_diagnostic = Some(diagnostic);
        }
    }
}

#[when("the syntax layer lowers the input")]
async fn syntax_layer_lowers_input(world: &mut AnvilWorld) {
    match lower_source(&world.source) {
        Ok(ast) => {
            world.ast = Some(ast);
            world.ast_diagnostic = None;
            world.module_diagnostic = None;
        }
        Err(diagnostic) => {
            world.ast = None;
            world.module_diagnostic = module_diagnostic_clone(&diagnostic);
            world.ast_diagnostic = Some(diagnostic);
        }
    }
}

#[when("the syntax layer lowers the input with the module resolver")]
async fn syntax_layer_lowers_input_with_module_resolver(world: &mut AnvilWorld) {
    match lower_source_with_resolver(&world.source, &world.module_resolver) {
        Ok(ast) => {
            world.ast = Some(ast);
            world.ast_diagnostic = None;
            world.module_diagnostic = None;
        }
        Err(diagnostic) => {
            world.ast = None;
            world.module_diagnostic = module_diagnostic_clone(&diagnostic);
            world.ast_diagnostic = Some(diagnostic);
        }
    }
}

#[given(expr = "module {string} exists in {word} root {string} at {string}")]
async fn module_exists_in_root(
    world: &mut AnvilWorld,
    module: String,
    root_kind: String,
    root_name: String,
    path: String,
) {
    world
        .module_resolver
        .add_module(parse_module_root_kind(&root_kind), root_name, module, path);
}

#[given(
    expr = "draft overlay {string} owned by {string} overrides module {string} with source {string}"
)]
async fn draft_overlay_overrides_module(
    world: &mut AnvilWorld,
    draft_id: String,
    owner: String,
    module: String,
    source: String,
) {
    let overlay = DraftOverlay::new(draft_id, owner).with_module(module, source);
    world.module_resolver.add_draft_overlay(&overlay);
    world.draft_overlay = Some(overlay);
}

#[when(expr = "the draft overlay adds module {string} with source {string}")]
async fn draft_overlay_adds_module(world: &mut AnvilWorld, module: String, source: String) {
    world
        .draft_overlay
        .as_mut()
        .expect("draft overlay")
        .add_module(module, source);
}

#[when("the manifest is parsed")]
async fn manifest_is_parsed(world: &mut AnvilWorld) {
    match parse_manifest(&world.manifest_source) {
        Ok(manifest) => {
            world.manifest = Some(manifest);
            world.manifest_diagnostic = None;
        }
        Err(diagnostic) => {
            world.manifest = None;
            world.manifest_diagnostic = Some(diagnostic);
        }
    }
}

#[when("the package snapshot builds a module resolver")]
async fn package_snapshot_builds_module_resolver(world: &mut AnvilWorld) {
    let manifest = world.manifest.clone().unwrap_or_else(|| {
        let manifest = parse_manifest(&world.manifest_source).expect("manifest parses");
        world.manifest = Some(manifest.clone());
        manifest
    });
    let mut snapshot = PackageSnapshot::new(manifest);
    for source in &world.package_sources {
        snapshot.add_source(source.path.clone(), source.source.clone());
    }

    world.module_resolver = snapshot.module_resolver();
    world.module_resolution = None;
    world.module_diagnostic = None;
}

#[when("the filesystem package snapshot is loaded")]
async fn filesystem_package_snapshot_is_loaded(world: &mut AnvilWorld) {
    let root = world
        .filesystem_package_root
        .as_ref()
        .expect("filesystem package root");

    match load_package_snapshot(root) {
        Ok(snapshot) => {
            world.module_resolver = snapshot.module_resolver();
            world.project_diagnostic = None;
            world.module_resolution = None;
            world.module_diagnostic = None;
        }
        Err(diagnostic) => {
            world.project_diagnostic = Some(diagnostic);
        }
    }
}

#[when("the filesystem workspace snapshot is loaded")]
async fn filesystem_workspace_snapshot_is_loaded(world: &mut AnvilWorld) {
    let root = world
        .filesystem_package_root
        .as_ref()
        .expect("filesystem package root");

    match load_workspace_snapshot(root) {
        Ok(snapshot) => {
            world.module_resolver = snapshot.module_resolver();
            world.project_diagnostic = None;
            world.module_resolution = None;
            world.module_diagnostic = None;
        }
        Err(diagnostic) => {
            world.project_diagnostic = Some(diagnostic);
        }
    }
}

#[when("the filesystem package module session is loaded")]
async fn filesystem_package_module_session_is_loaded(world: &mut AnvilWorld) {
    let root = world
        .filesystem_package_root
        .as_ref()
        .expect("filesystem package root");

    match load_workspace_snapshot(root) {
        Ok(snapshot) => {
            world.module_resolver = snapshot.module_resolver();
            world.module_session = ModuleSession::with_workspace_snapshot(&snapshot);
            world.project_diagnostic = None;
            world.module_resolution = None;
            world.module_diagnostic = None;
            world.vm_output = None;
            world.vm_diagnostic = None;
        }
        Err(diagnostic) => {
            world.project_diagnostic = Some(diagnostic);
        }
    }
}

#[when(expr = "the module resolver resolves {string}")]
async fn module_resolver_resolves(world: &mut AnvilWorld, module: String) {
    match world.module_resolver.resolve(&module) {
        Ok(resolution) => {
            world.module_resolution = Some(resolution);
            world.module_diagnostic = None;
        }
        Err(diagnostic) => {
            world.module_resolution = None;
            world.module_diagnostic = Some(diagnostic);
        }
    }
}

#[given("an empty REPL session")]
async fn empty_repl_session(world: &mut AnvilWorld) {
    world.repl_session = ReplSession::new();
    world.repl_interaction = None;
    world.repl_response = None;
}

#[when(expr = "the REPL session reads the line {string}")]
async fn repl_session_reads_line(world: &mut AnvilWorld, source: String) {
    let mut line = source;
    line.push('\n');

    let interaction = world.repl_session.push_line(&line);
    if let Some(response) = interaction.response() {
        world.repl_response = Some(response.clone());
    } else {
        world.repl_response = None;
    }
    world.repl_interaction = Some(interaction);
}

#[when("the REPL response is serialized as JSON")]
async fn repl_response_is_serialized_as_json(world: &mut AnvilWorld) {
    let response = world
        .repl_response
        .as_ref()
        .expect("reader-backed REPL response");

    world.json_response = Some(serde_json::to_value(response).expect("JSON response"));
}

#[when("the AST response is serialized as JSON")]
async fn ast_response_is_serialized_as_json(world: &mut AnvilWorld) {
    let ast = world.ast.as_ref().expect("AST response");

    world.json_response = Some(serde_json::json!({
        "status": "ast",
        "expressions": ast,
    }));
}

#[when("the syntax object response is serialized as JSON")]
async fn syntax_object_response_is_serialized_as_json(world: &mut AnvilWorld) {
    let objects = world
        .syntax_objects
        .as_ref()
        .expect("syntax object response");

    world.json_response = Some(serde_json::json!({
        "status": "syntax",
        "objects": objects,
    }));
}

#[when("the reader diagnostic is rendered as text")]
async fn reader_diagnostic_is_rendered_as_text(world: &mut AnvilWorld) {
    let response = world
        .repl_response
        .as_ref()
        .expect("reader-backed REPL response");
    let diagnostic = response.diagnostic().expect("reader diagnostic");

    world.rendered_diagnostic = Some(diagnostic.render_code_frame());
}

#[when("the REPL interaction is serialized as JSON")]
async fn repl_interaction_is_serialized_as_json(world: &mut AnvilWorld) {
    let interaction = world
        .repl_interaction
        .as_ref()
        .expect("REPL session interaction");

    world.json_response = Some(serde_json::to_value(interaction).expect("JSON interaction"));
}

#[then("the REPL session is waiting for more input")]
async fn repl_session_waiting_for_more_input(world: &mut AnvilWorld) {
    let interaction = world
        .repl_interaction
        .as_ref()
        .expect("REPL session interaction");

    assert!(interaction.is_pending());
    assert!(world.repl_session.is_pending());
}

#[then("the response contains one datum")]
async fn response_contains_one_datum(world: &mut AnvilWorld) {
    assert_response_datum_count(world, 1);
}

#[then(expr = "the response contains {int} datums")]
async fn response_contains_n_datums(world: &mut AnvilWorld, expected: usize) {
    assert_response_datum_count(world, expected);
}

fn assert_response_datum_count(world: &mut AnvilWorld, expected: usize) {
    let response = world
        .repl_response
        .as_ref()
        .expect("reader-backed REPL response");

    assert_eq!(response.datums().len(), expected);
}

#[then(expr = "the first datum prints as {string}")]
async fn first_datum_prints_as(world: &mut AnvilWorld, expected: String) {
    assert_first_datum_prints_as(world, &expected);
}

#[then("the first datum prints as")]
async fn first_datum_prints_as_docstring(world: &mut AnvilWorld, #[step] step: &Step) {
    let expected = trim_docstring(step.docstring().expect("expected datum docstring"));

    assert_first_datum_prints_as(world, expected);
}

fn assert_first_datum_prints_as(world: &mut AnvilWorld, expected: &str) {
    let response = world
        .repl_response
        .as_ref()
        .expect("reader-backed REPL response");
    let first = response.datums().first().expect("first datum");

    assert_eq!(first.to_string(), expected);
}

#[then("the datums print as")]
async fn datums_print_as_docstring(world: &mut AnvilWorld, #[step] step: &Step) {
    let expected = trim_docstring(step.docstring().expect("expected datums docstring"));
    let response = world
        .repl_response
        .as_ref()
        .expect("reader-backed REPL response");

    assert_eq!(format_datums(response.datums()), expected);
}

#[then(expr = "the REPL evaluation value prints as {string}")]
async fn repl_evaluation_value_prints_as(world: &mut AnvilWorld, expected: String) {
    let response = world.repl_response.as_ref().expect("REPL response");
    let Some(evaluation) = response.evaluation() else {
        panic!("REPL response has no evaluation");
    };
    let Some(value) = evaluation.value() else {
        panic!("REPL evaluation has no value");
    };

    assert_eq!(value.to_string(), expected);
}

#[then("the AST contains one expression")]
async fn ast_contains_one_expression(world: &mut AnvilWorld) {
    let ast = world.ast.as_ref().expect("AST response");

    assert_eq!(ast.len(), 1);
}

#[then(expr = "the first AST kind is {string}")]
async fn first_ast_kind_is(world: &mut AnvilWorld, expected: String) {
    let ast = world.ast.as_ref().expect("AST response");
    let json = serde_json::to_value(ast.first().expect("first AST")).expect("AST JSON");

    assert_eq!(json["kind"], expected);
}

#[then(expr = "the first AST prints as {string}")]
async fn first_ast_prints_as(world: &mut AnvilWorld, expected: String) {
    let ast = world.ast.as_ref().expect("AST response");

    assert_eq!(format_ast(ast), expected);
}

#[then(expr = "the first require import module is {string}")]
async fn first_require_import_module_is(world: &mut AnvilWorld, expected: String) {
    let json = first_ast_json(world);

    assert_eq!(json["imports"][0]["module"], expected);
}

#[then(expr = "the first require import alias is {string}")]
async fn first_require_import_alias_is(world: &mut AnvilWorld, expected: String) {
    let json = first_ast_json(world);

    assert_eq!(json["imports"][0]["alias"], expected);
}

#[then(expr = "the first require import resolution root kind is {string}")]
async fn first_require_import_resolution_root_kind_is(world: &mut AnvilWorld, expected: String) {
    let json = first_ast_json(world);

    assert_eq!(json["imports"][0]["resolution"]["root_kind"], expected);
}

#[then(expr = "the first require import resolution path is {string}")]
async fn first_require_import_resolution_path_is(world: &mut AnvilWorld, expected: String) {
    let json = first_ast_json(world);

    assert_eq!(json["imports"][0]["resolution"]["path"], expected);
}

fn first_ast_json(world: &mut AnvilWorld) -> serde_json::Value {
    let ast = world.ast.as_ref().expect("AST response");

    serde_json::to_value(ast.first().expect("first AST")).expect("AST JSON")
}

fn eval_response_json(world: &mut AnvilWorld) -> serde_json::Value {
    serde_json::to_value(world.eval_response.as_ref().expect("eval response"))
        .expect("eval response JSON")
}

fn embedded_snapshot(world: &mut AnvilWorld) -> &EmbeddedRuntimeSnapshot {
    world
        .embedded_snapshot
        .as_ref()
        .expect("embedded runtime snapshot")
}

#[then(expr = "the VM value prints as {string}")]
async fn vm_value_prints_as(world: &mut AnvilWorld, expected: String) {
    let output = world.vm_output.as_ref().expect("VM output");

    assert_eq!(output.value.to_string(), expected);
}

#[then(expr = "the VM session binding {string} prints as {string}")]
async fn vm_session_binding_prints_as(world: &mut AnvilWorld, name: String, expected: String) {
    let value = world
        .vm_session
        .binding(&name)
        .unwrap_or_else(|| panic!("VM session binding {name}"));

    assert_eq!(value.to_string(), expected);
}

#[then(expr = "the module session binding {string} prints as {string}")]
async fn module_session_binding_prints_as(world: &mut AnvilWorld, name: String, expected: String) {
    let value = world
        .module_session
        .binding(&name)
        .unwrap_or_else(|| panic!("module session binding {name}"));

    assert_eq!(value.to_string(), expected);
}

#[then(expr = "the module session has loaded {string}")]
async fn module_session_has_loaded(world: &mut AnvilWorld, source_id: String) {
    assert!(
        world.module_session.is_loaded(&source_id),
        "loaded modules: {:?}",
        world.module_session.loaded_source_ids()
    );
}

#[then(expr = "the VM max call depth is {int}")]
async fn vm_max_call_depth_is(world: &mut AnvilWorld, expected: usize) {
    let output = world.vm_output.as_ref().expect("VM output");

    assert_eq!(output.max_call_depth, expected);
}

#[then(expr = "the syntax object count is {int}")]
async fn syntax_object_count_is(world: &mut AnvilWorld, expected: usize) {
    let objects = world
        .syntax_objects
        .as_ref()
        .expect("syntax object response");

    assert_eq!(objects.len(), expected);
}

#[then(expr = "the first syntax object id is {string}")]
async fn first_syntax_object_id_is(world: &mut AnvilWorld, expected: String) {
    let object = first_syntax_object(world);

    assert_eq!(object.id, expected);
}

#[then(expr = "the first syntax object source id is {string}")]
async fn first_syntax_object_source_id_is(world: &mut AnvilWorld, expected: String) {
    let object = first_syntax_object(world);

    assert_eq!(object.source_id, expected);
}

#[then(expr = "the first syntax object span starts at line {int} column {int}")]
async fn first_syntax_object_span_starts_at(
    world: &mut AnvilWorld,
    expected_line: usize,
    expected_column: usize,
) {
    let object = first_syntax_object(world);

    assert_eq!(object.span.start.line, expected_line);
    assert_eq!(object.span.start.column, expected_column);
}

#[then(expr = "the first syntax object datum prints as {string}")]
async fn first_syntax_object_datum_prints_as(world: &mut AnvilWorld, expected: String) {
    let object = first_syntax_object(world);

    assert_eq!(object.to_string(), expected);
}

fn first_syntax_object(world: &mut AnvilWorld) -> &SyntaxObject {
    world
        .syntax_objects
        .as_ref()
        .expect("syntax object response")
        .first()
        .expect("first syntax object")
}

#[then(expr = "the module resolution root kind is {string}")]
async fn module_resolution_root_kind_is(world: &mut AnvilWorld, expected: String) {
    let resolution = world.module_resolution.as_ref().expect("module resolution");
    let json = serde_json::to_value(resolution).expect("module resolution JSON");

    assert_eq!(json["root_kind"], expected);
}

#[then(expr = "the module resolution root name is {string}")]
async fn module_resolution_root_name_is(world: &mut AnvilWorld, expected: String) {
    let resolution = world.module_resolution.as_ref().expect("module resolution");

    assert_eq!(resolution.root_name, expected);
}

#[then(expr = "the module resolution path is {string}")]
async fn module_resolution_path_is(world: &mut AnvilWorld, expected: String) {
    let resolution = world.module_resolution.as_ref().expect("module resolution");

    assert_eq!(resolution.path, expected);
}

#[then(expr = "the module resolution shadows root kind {string}")]
async fn module_resolution_shadows_root_kind(world: &mut AnvilWorld, expected: String) {
    let shadowed = module_resolution_shadowed(world);
    let json = serde_json::to_value(shadowed).expect("shadowed module JSON");

    assert_eq!(json["root_kind"], expected);
}

#[then(expr = "the module resolution shadows root name {string}")]
async fn module_resolution_shadows_root_name(world: &mut AnvilWorld, expected: String) {
    let shadowed = module_resolution_shadowed(world);

    assert_eq!(shadowed.root_name, expected);
}

#[then(expr = "the module resolution shadows path {string}")]
async fn module_resolution_shadows_path(world: &mut AnvilWorld, expected: String) {
    let shadowed = module_resolution_shadowed(world);

    assert_eq!(shadowed.path, expected);
}

fn module_resolution_shadowed(world: &mut AnvilWorld) -> &anvil_core::ModuleCandidate {
    world
        .module_resolution
        .as_ref()
        .expect("module resolution")
        .shadowed
        .as_ref()
        .expect("shadowed module")
}

#[then(expr = "the draft overlay status is {string}")]
async fn draft_overlay_status_is(world: &mut AnvilWorld, expected: String) {
    let overlay = world.draft_overlay.as_ref().expect("draft overlay");
    let json = serde_json::to_value(overlay).expect("draft overlay JSON");

    assert_eq!(json["status"], expected);
}

#[then(expr = "the draft overlay owner is {string}")]
async fn draft_overlay_owner_is(world: &mut AnvilWorld, expected: String) {
    let overlay = world.draft_overlay.as_ref().expect("draft overlay");

    assert_eq!(overlay.owner, expected);
}

#[then(expr = "the first draft module name is {string}")]
async fn first_draft_module_name_is(world: &mut AnvilWorld, expected: String) {
    let module = first_draft_module(world);

    assert_eq!(module.module, expected);
}

#[then(expr = "the first draft module source is {string}")]
async fn first_draft_module_source_is(world: &mut AnvilWorld, expected: String) {
    let module = first_draft_module(world);

    assert_eq!(module.source, expected);
}

#[then(expr = "the first draft module path is {string}")]
async fn first_draft_module_path_is(world: &mut AnvilWorld, expected: String) {
    let module = first_draft_module(world);

    assert_eq!(module.path, expected);
}

#[then(expr = "the first draft module has {int} diagnostics")]
async fn first_draft_module_has_diagnostics(world: &mut AnvilWorld, expected: usize) {
    let module = first_draft_module(world);

    assert_eq!(module.diagnostics.len(), expected);
}

fn first_draft_module(world: &mut AnvilWorld) -> &anvil_core::DraftModule {
    world
        .draft_overlay
        .as_ref()
        .expect("draft overlay")
        .modules
        .first()
        .expect("first draft module")
}

#[then(expr = "the manifest package name is {string}")]
async fn manifest_package_name_is(world: &mut AnvilWorld, expected: String) {
    let manifest = world.manifest.as_ref().expect("manifest");

    assert_eq!(manifest.package.name, expected);
}

#[then(expr = "the manifest package version is {string}")]
async fn manifest_package_version_is(world: &mut AnvilWorld, expected: String) {
    let manifest = world.manifest.as_ref().expect("manifest");

    assert_eq!(manifest.package.version, expected);
}

#[then(expr = "the manifest lib module is {string}")]
async fn manifest_lib_module_is(world: &mut AnvilWorld, expected: String) {
    let manifest = world.manifest.as_ref().expect("manifest");

    assert_eq!(manifest.lib.module, expected);
}

#[then(expr = "the manifest lib path is {string}")]
async fn manifest_lib_path_is(world: &mut AnvilWorld, expected: String) {
    let manifest = world.manifest.as_ref().expect("manifest");

    assert_eq!(manifest.lib.path, expected);
}

#[then(expr = "the manifest source roots are {string}")]
async fn manifest_source_roots_are(world: &mut AnvilWorld, expected: String) {
    let manifest = world.manifest.as_ref().expect("manifest");

    assert_eq!(manifest.source.roots, split_csv(&expected));
}

#[then(expr = "the manifest test roots are {string}")]
async fn manifest_test_roots_are(world: &mut AnvilWorld, expected: String) {
    let manifest = world.manifest.as_ref().expect("manifest");

    assert_eq!(manifest.source.tests, split_csv(&expected));
}

#[then(expr = "the manifest eval roots are {string}")]
async fn manifest_eval_roots_are(world: &mut AnvilWorld, expected: String) {
    let manifest = world.manifest.as_ref().expect("manifest");

    assert_eq!(manifest.source.evals, split_csv(&expected));
}

#[then(expr = "the manifest example roots are {string}")]
async fn manifest_example_roots_are(world: &mut AnvilWorld, expected: String) {
    let manifest = world.manifest.as_ref().expect("manifest");

    assert_eq!(manifest.source.examples, split_csv(&expected));
}

#[then(expr = "the manifest workspace members are {string}")]
async fn manifest_workspace_members_are(world: &mut AnvilWorld, expected: String) {
    let manifest = world.manifest.as_ref().expect("manifest");
    let workspace = manifest.workspace.as_ref().expect("workspace manifest");

    assert_eq!(workspace.members, split_csv(&expected));
}

#[then(expr = "the manifest diagnostic code is {string}")]
async fn manifest_diagnostic_code_is(world: &mut AnvilWorld, expected: String) {
    let diagnostic = world
        .manifest_diagnostic
        .as_ref()
        .expect("manifest diagnostic");

    assert_eq!(diagnostic.code, expected);
}

#[then(expr = "the manifest diagnostic phase is {string}")]
async fn manifest_diagnostic_phase_is(world: &mut AnvilWorld, expected: String) {
    let diagnostic = world
        .manifest_diagnostic
        .as_ref()
        .expect("manifest diagnostic");
    let json = serde_json::to_value(diagnostic).expect("diagnostic JSON");

    assert_eq!(json["phase"], expected);
}

#[then(expr = "the project diagnostic code is {string}")]
async fn project_diagnostic_code_is(world: &mut AnvilWorld, expected: String) {
    let diagnostic = world
        .project_diagnostic
        .as_ref()
        .expect("project diagnostic");

    assert_eq!(diagnostic.code, expected);
}

#[then(expr = "the project diagnostic phase is {string}")]
async fn project_diagnostic_phase_is(world: &mut AnvilWorld, expected: String) {
    let diagnostic = world
        .project_diagnostic
        .as_ref()
        .expect("project diagnostic");
    let json = serde_json::to_value(diagnostic).expect("diagnostic JSON");

    assert_eq!(json["phase"], expected);
}

#[then(expr = "the VM diagnostic code is {string}")]
async fn vm_diagnostic_code_is(world: &mut AnvilWorld, expected: String) {
    let diagnostic = world.vm_diagnostic.as_ref().expect("VM diagnostic");

    assert_eq!(diagnostic.code, expected);
}

#[then(expr = "the VM diagnostic phase is {string}")]
async fn vm_diagnostic_phase_is(world: &mut AnvilWorld, expected: String) {
    let diagnostic = world.vm_diagnostic.as_ref().expect("VM diagnostic");
    let json = serde_json::to_value(diagnostic).expect("diagnostic JSON");

    assert_eq!(json["phase"], expected);
}

#[then(expr = "the VM diagnostic primary span starts at line {int} column {int}")]
async fn vm_diagnostic_primary_span_starts_at(
    world: &mut AnvilWorld,
    expected_line: usize,
    expected_column: usize,
) {
    let diagnostic = world.vm_diagnostic.as_ref().expect("VM diagnostic");

    assert_eq!(diagnostic.primary_span.start.line, expected_line);
    assert_eq!(diagnostic.primary_span.start.column, expected_column);
}

#[then(expr = "the resource handle type is {string}")]
async fn resource_handle_type_is(world: &mut AnvilWorld, expected: String) {
    let handle = world.resource_handle.as_ref().expect("resource handle");

    assert_eq!(handle.type_id, expected);
}

#[then(expr = "the resource handle holder is {string}")]
async fn resource_handle_holder_is(world: &mut AnvilWorld, expected: String) {
    let handle = world.resource_handle.as_ref().expect("resource handle");

    assert_eq!(handle.holder, expected);
}

#[then(expr = "the resource handle grants include {string}")]
async fn resource_handle_grants_include(world: &mut AnvilWorld, expected: String) {
    let handle = world.resource_handle.as_ref().expect("resource handle");

    assert!(handle.has_grant(&expected));
}

#[then("the resource handle display hides the raw token")]
async fn resource_handle_display_hides_raw_token(world: &mut AnvilWorld) {
    let handle = world.resource_handle.as_ref().expect("resource handle");

    assert!(!handle.display_summary().contains(&handle.handle_id));
}

#[then(expr = "the delegated resource handle holder is {string}")]
async fn delegated_resource_handle_holder_is(world: &mut AnvilWorld, expected: String) {
    let handle = delegated_resource_handle(world);

    assert_eq!(handle.holder, expected);
}

#[then(expr = "the delegated resource handle grants include {string}")]
async fn delegated_resource_handle_grants_include(world: &mut AnvilWorld, expected: String) {
    let handle = delegated_resource_handle(world);

    assert!(handle.has_grant(&expected));
}

#[then(expr = "the delegated resource handle grants do not include {string}")]
async fn delegated_resource_handle_grants_do_not_include(world: &mut AnvilWorld, expected: String) {
    let handle = delegated_resource_handle(world);

    assert!(!handle.has_grant(&expected));
}

fn delegated_resource_handle(world: &mut AnvilWorld) -> &HandleEntry {
    world
        .delegated_resource_handle
        .as_ref()
        .expect("delegated resource handle")
}

#[then(expr = "the resource denial reason is {string}")]
async fn resource_denial_reason_is(world: &mut AnvilWorld, expected: String) {
    let error = resource_error(world);
    let reason = serde_json::to_value(error.denial.reason).expect("reason JSON");

    assert_eq!(reason, expected);
}

#[then(expr = "the resource denial missing capability is {string}")]
async fn resource_denial_missing_capability_is(world: &mut AnvilWorld, expected: String) {
    let error = resource_error(world);

    assert_eq!(
        error.denial.missing_capability.as_deref(),
        Some(expected.as_str())
    );
}

#[then(expr = "the resource denial phase is {string}")]
async fn resource_denial_phase_is(world: &mut AnvilWorld, expected: String) {
    let error = resource_error(world);
    let diagnostic = serde_json::to_value(error.diagnostic.as_ref()).expect("diagnostic JSON");

    assert_eq!(diagnostic["phase"], expected);
}

#[then(expr = "the resource audit decision is {string}")]
async fn resource_audit_decision_is(world: &mut AnvilWorld, expected: String) {
    let error = resource_error(world);
    let decision = serde_json::to_value(error.audit_event.decision).expect("decision JSON");

    assert_eq!(decision, expected);
}

#[then(expr = "the resource operation audit decision is {string}")]
async fn resource_operation_audit_decision_is(world: &mut AnvilWorld, expected: String) {
    let outcome = resource_operation_outcome(world);
    let audit_event = outcome.audit_events.first().expect("audit event");
    let decision = serde_json::to_value(audit_event.decision).expect("decision JSON");

    assert_eq!(decision, expected);
}

#[then(expr = "the resource adapter call count is {int}")]
async fn resource_adapter_call_count_is(world: &mut AnvilWorld, expected: usize) {
    let adapter = world.resource_adapter.as_ref().expect("resource adapter");

    assert_eq!(adapter.calls, expected);
}

#[then(expr = "the host function call count is {int}")]
async fn host_function_call_count_is(world: &mut AnvilWorld, expected: usize) {
    assert_eq!(world.host_function_calls.load(Ordering::Relaxed), expected);
}

#[then(expr = "the resource adapter output status is {string}")]
async fn resource_adapter_output_status_is(world: &mut AnvilWorld, expected: String) {
    let outcome = resource_operation_outcome(world);
    let status = serde_json::to_value(outcome.adapter.status).expect("status JSON");

    assert_eq!(status, expected);
}

#[then(expr = "the resource adapter string value is {string}")]
async fn resource_adapter_string_value_is(world: &mut AnvilWorld, expected: String) {
    let outcome = resource_operation_outcome(world);

    assert_eq!(outcome.adapter.value, Value::String(expected));
}

#[then(expr = "the resource execution mode is {string}")]
async fn resource_execution_mode_is(world: &mut AnvilWorld, expected: String) {
    let outcome = resource_operation_outcome(world);
    let mode = serde_json::to_value(outcome.execution_mode).expect("execution mode JSON");

    assert_eq!(mode, expected);
}

fn resource_operation_outcome(world: &mut AnvilWorld) -> &ResourceOperationOutcome {
    world
        .resource_operation_outcome
        .as_ref()
        .expect("resource operation outcome")
}

fn resource_error(world: &mut AnvilWorld) -> &ResourceError {
    world.resource_error.as_ref().expect("resource error")
}

fn embedded_runtime(world: &mut AnvilWorld) -> &mut EmbeddedRuntime {
    world.embedded_runtime.as_mut().expect("embedded runtime")
}

fn reset_host_call_count(world: &mut AnvilWorld) {
    world.host_function_calls.store(0, Ordering::Relaxed);
}

fn register_host_add(session: &mut VmSession, name: &str, calls: Arc<AtomicUsize>) {
    session.register_host_function(
        HostFunctionSpec::new(name).with_exact_arity(2),
        move |_context: &HostCallContext, args: &[Value]| {
            calls.fetch_add(1, Ordering::Relaxed);
            host_add(args)
        },
    );
}

fn register_module_host_add(session: &mut ModuleSession, name: &str, calls: Arc<AtomicUsize>) {
    session.register_host_function(
        HostFunctionSpec::new(name).with_exact_arity(2),
        move |_context: &HostCallContext, args: &[Value]| {
            calls.fetch_add(1, Ordering::Relaxed);
            host_add(args)
        },
    );
}

fn host_add(args: &[Value]) -> Result<Value, HostCallFailure> {
    match args {
        [Value::Integer(left), Value::Integer(right)] => Ok(Value::Integer(left + right)),
        _ => Err(HostCallFailure::new("host/add expected integer arguments")
            .with_expected("integer")
            .with_actual(format!("{args:?}"))),
    }
}

fn capability_profile(
    profile_id: String,
    principal: String,
    trust_zone: String,
    capabilities: String,
) -> CapabilityProfile {
    CapabilityProfile::new(profile_id, principal, trust_zone)
        .with_capabilities(split_csv(&capabilities))
}

#[derive(Debug, Clone)]
struct RecordingResourceAdapter {
    adapter_id: String,
    type_id: String,
    operations: Vec<String>,
    value: Value,
    failure: Option<ResourceAdapterFailure>,
    calls: usize,
}

impl RecordingResourceAdapter {
    fn new(adapter_id: String, type_id: String, operations: Vec<String>) -> Self {
        Self {
            adapter_id,
            type_id,
            operations,
            value: Value::Nil,
            failure: None,
            calls: 0,
        }
    }
}

impl ResourceAdapter for RecordingResourceAdapter {
    fn adapter_id(&self) -> &str {
        &self.adapter_id
    }

    fn type_id(&self) -> &str {
        &self.type_id
    }

    fn supported_operations(&self) -> Vec<String> {
        self.operations.clone()
    }

    fn execution_mode(&self, _operation: &str) -> ResourceExecutionMode {
        ResourceExecutionMode::Effectful
    }

    fn execute(&mut self, request: ResourceAdapterRequest<'_>) -> ResourceAdapterResult {
        self.calls += 1;

        if let Some(failure) = self.failure.clone() {
            return Err(failure);
        }

        Ok(
            ResourceAdapterOutcome::completed(self.value.clone()).with_effect(
                ResourceEffectRecord::new(
                    resource_effect_from_operation(&request.authorization.operation),
                    &request.authorization.resource_id,
                    &request.authorization.operation,
                )
                .committed(),
            ),
        )
    }
}

fn trim_docstring(value: &str) -> &str {
    value.trim_matches('\n')
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn resource_effect_from_operation(operation: &str) -> ResourceEffect {
    ResourceEffect::from_operation(operation)
}

fn create_temp_package_root() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    let unique_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "anvil-acceptance-package-{}-{nanos}-{unique_id}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("temporary package root");

    root
}

fn write_package_file(root: &Path, path: &str, source: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().expect("file parent")).expect("package file parent");
    fs::write(path, source).expect("package file");
}

#[then("the response is a reader error")]
async fn response_is_reader_error(world: &mut AnvilWorld) {
    let response = world
        .repl_response
        .as_ref()
        .expect("reader-backed REPL response");

    assert!(response.diagnostic().is_some());
}

#[then(expr = "the diagnostic code is {string}")]
async fn diagnostic_code_is(world: &mut AnvilWorld, expected: String) {
    let response = world
        .repl_response
        .as_ref()
        .expect("reader-backed REPL response");
    let diagnostic = response.diagnostic().expect("reader diagnostic");

    assert_eq!(diagnostic.code, expected);
}

#[then(expr = "the JSON status is {string}")]
async fn json_status_is(world: &mut AnvilWorld, expected: String) {
    let json = world.json_response.as_ref().expect("JSON response");

    assert_eq!(json["status"], expected);
}

#[then(expr = "the response envelope status is {string}")]
async fn response_envelope_status_is(world: &mut AnvilWorld, expected: String) {
    let json = eval_response_json(world);

    assert_eq!(json["status"], expected);
}

#[then(expr = "the response envelope kind is {string}")]
async fn response_envelope_kind_is(world: &mut AnvilWorld, expected: String) {
    let json = eval_response_json(world);

    assert_eq!(json["kind"], expected);
}

#[then(expr = "the response envelope summary is {string}")]
async fn response_envelope_summary_is(world: &mut AnvilWorld, expected: String) {
    let json = eval_response_json(world);

    assert_eq!(json["summary"], expected);
}

#[then(expr = "the response envelope value display is {string}")]
async fn response_envelope_value_display_is(world: &mut AnvilWorld, expected: String) {
    let json = eval_response_json(world);

    assert_eq!(json["value"]["display"], expected);
}

#[then(expr = "the response envelope value kind is {string}")]
async fn response_envelope_value_kind_is(world: &mut AnvilWorld, expected: String) {
    let json = eval_response_json(world);

    assert_eq!(json["value"]["kind"], expected);
}

#[then(expr = "the response envelope diagnostic code is {string}")]
async fn response_envelope_diagnostic_code_is(world: &mut AnvilWorld, expected: String) {
    let json = eval_response_json(world);

    assert_eq!(json["diagnostics"][0]["code"], expected);
}

#[then(expr = "the response envelope diagnostic phase is {string}")]
async fn response_envelope_diagnostic_phase_is(world: &mut AnvilWorld, expected: String) {
    let json = eval_response_json(world);

    assert_eq!(json["diagnostics"][0]["phase"], expected);
}

#[then(expr = "the response envelope diagnostic primary span starts at line {int} column {int}")]
async fn response_envelope_diagnostic_primary_span_starts_at(
    world: &mut AnvilWorld,
    expected_line: usize,
    expected_column: usize,
) {
    let json = eval_response_json(world);

    assert_eq!(
        json["diagnostics"][0]["primary_span"]["start"]["line"],
        expected_line
    );
    assert_eq!(
        json["diagnostics"][0]["primary_span"]["start"]["column"],
        expected_column
    );
}

#[then("the response envelope metadata includes VM execution metrics")]
async fn response_envelope_metadata_includes_vm_execution_metrics(world: &mut AnvilWorld) {
    let json = eval_response_json(world);

    assert!(
        json["metadata"]["instructions_executed"]
            .as_u64()
            .expect("instructions_executed")
            > 0
    );
    assert!(
        json["metadata"]["max_call_depth"]
            .as_u64()
            .expect("max_call_depth")
            > 0
    );
}

#[then("the response envelope omits debug facets")]
async fn response_envelope_omits_debug_facets(world: &mut AnvilWorld) {
    let json = eval_response_json(world);

    assert!(json.get("facets").is_none());
}

#[then(expr = "the response envelope has facet {string}")]
async fn response_envelope_has_facet(world: &mut AnvilWorld, expected: String) {
    let json = eval_response_json(world);
    let facets = json["facets"].as_array().expect("facets array");

    assert!(
        facets
            .iter()
            .any(|facet| facet["name"].as_str() == Some(expected.as_str())),
        "missing response facet {expected:?}: {json}"
    );
}

#[then(expr = "the embedded runtime snapshot protocol is {string}")]
async fn embedded_runtime_snapshot_protocol_is(world: &mut AnvilWorld, expected: String) {
    let snapshot = embedded_snapshot(world);

    assert_eq!(snapshot.protocol, expected);
}

#[then("the embedded runtime active profile is absent")]
async fn embedded_runtime_active_profile_is_absent(world: &mut AnvilWorld) {
    let snapshot = embedded_snapshot(world);

    assert!(snapshot.active_profile_id.is_none());
}

#[then(expr = "the embedded runtime snapshot includes host function {string}")]
async fn embedded_runtime_snapshot_includes_host_function(
    world: &mut AnvilWorld,
    expected: String,
) {
    let snapshot = embedded_snapshot(world);

    assert!(
        snapshot
            .host_functions
            .iter()
            .any(|function| function.name == expected),
        "missing host function {expected:?}: {snapshot:?}"
    );
}

#[then(expr = "the embedded runtime snapshot includes profile {string}")]
async fn embedded_runtime_snapshot_includes_profile(world: &mut AnvilWorld, expected: String) {
    let snapshot = embedded_snapshot(world);

    assert!(
        snapshot
            .profiles
            .iter()
            .any(|profile| profile.profile_id == expected),
        "missing profile {expected:?}: {snapshot:?}"
    );
}

#[then(expr = "the embedded runtime host function {string} exact arity is {int}")]
async fn embedded_runtime_host_function_exact_arity_is(
    world: &mut AnvilWorld,
    name: String,
    expected: usize,
) {
    let snapshot = embedded_snapshot(world);
    let function = snapshot
        .host_functions
        .iter()
        .find(|function| function.name == name)
        .unwrap_or_else(|| panic!("host function {name}"));

    assert_eq!(function.arity.min, expected);
    assert_eq!(function.arity.max, Some(expected));
}

#[then(expr = "the embedded runtime snapshot includes resource {string}")]
async fn embedded_runtime_snapshot_includes_resource(world: &mut AnvilWorld, expected: String) {
    let snapshot = embedded_snapshot(world);

    assert!(
        snapshot
            .resources
            .iter()
            .any(|resource| resource.resource_id == expected),
        "missing resource {expected:?}: {snapshot:?}"
    );
}

#[then(expr = "the embedded runtime snapshot includes handle for resource {string}")]
async fn embedded_runtime_snapshot_includes_handle_for_resource(
    world: &mut AnvilWorld,
    expected: String,
) {
    let snapshot = embedded_snapshot(world);

    assert!(
        snapshot
            .handles
            .iter()
            .any(|handle| handle.resource_id == expected),
        "missing handle for resource {expected:?}: {snapshot:?}"
    );
}

#[then(expr = "the embedded runtime audit contains {string} decision {string}")]
async fn embedded_runtime_audit_contains_decision(
    world: &mut AnvilWorld,
    expected_kind: String,
    expected_decision: String,
) {
    let snapshot = embedded_snapshot(world);
    let events = serde_json::to_value(&snapshot.audit_events).expect("audit events JSON");
    let events = events.as_array().expect("audit events array");

    assert!(
        events.iter().any(|event| {
            event["kind"].as_str() == Some(expected_kind.as_str())
                && event["decision"].as_str() == Some(expected_decision.as_str())
        }),
        "missing audit event kind {expected_kind:?} decision {expected_decision:?}: {events:?}"
    );
}

#[then(expr = "the embedded runtime audit contains diagnostic code {string}")]
async fn embedded_runtime_audit_contains_diagnostic_code(world: &mut AnvilWorld, expected: String) {
    let snapshot = embedded_snapshot(world);

    assert!(
        snapshot
            .audit_events
            .iter()
            .any(|event| event.diagnostic_code.as_deref() == Some(expected.as_str())),
        "missing audit diagnostic code {expected:?}: {:?}",
        snapshot.audit_events
    );
}

#[then(expr = "the JSON diagnostic code is {string}")]
async fn json_diagnostic_code_is(world: &mut AnvilWorld, expected: String) {
    let json = world.json_response.as_ref().expect("JSON response");

    assert_eq!(json["diagnostic"]["code"], expected);
}

#[then(expr = "the JSON diagnostic source id is {string}")]
async fn json_diagnostic_source_id_is(world: &mut AnvilWorld, expected: String) {
    let json = world.json_response.as_ref().expect("JSON response");

    assert_eq!(json["diagnostic"]["source_id"], expected);
}

#[then(expr = "the JSON diagnostic severity is {string}")]
async fn json_diagnostic_severity_is(world: &mut AnvilWorld, expected: String) {
    let json = world.json_response.as_ref().expect("JSON response");

    assert_eq!(json["diagnostic"]["severity"], expected);
}

#[then(expr = "the JSON diagnostic phase is {string}")]
async fn json_diagnostic_phase_is(world: &mut AnvilWorld, expected: String) {
    let json = world.json_response.as_ref().expect("JSON response");

    assert_eq!(json["diagnostic"]["phase"], expected);
}

#[then(expr = "the JSON diagnostic primary span starts at line {int} column {int}")]
async fn json_diagnostic_primary_span_starts_at(
    world: &mut AnvilWorld,
    expected_line: usize,
    expected_column: usize,
) {
    let json = world.json_response.as_ref().expect("JSON response");

    assert_eq!(
        json["diagnostic"]["primary_span"]["start"]["line"],
        expected_line
    );
    assert_eq!(
        json["diagnostic"]["primary_span"]["start"]["column"],
        expected_column
    );
}

#[then(expr = "the JSON diagnostic has {int} suggestion")]
async fn json_diagnostic_has_suggestion_count(world: &mut AnvilWorld, expected: usize) {
    let json = world.json_response.as_ref().expect("JSON response");
    let suggestions = json["diagnostic"]["suggestions"]
        .as_array()
        .expect("suggestions array");

    assert_eq!(suggestions.len(), expected);
}

#[then(expr = "the JSON first AST kind is {string}")]
async fn json_first_ast_kind_is(world: &mut AnvilWorld, expected: String) {
    let json = world.json_response.as_ref().expect("JSON response");

    assert_eq!(json["expressions"][0]["kind"], expected);
}

#[then(expr = "the JSON first syntax context has {int} scopes")]
async fn json_first_syntax_context_has_scopes(world: &mut AnvilWorld, expected: usize) {
    let json = world.json_response.as_ref().expect("JSON response");
    let scopes = json["objects"][0]["context"]["scopes"]
        .as_array()
        .expect("syntax context scopes");

    assert_eq!(scopes.len(), expected);
}

#[then(expr = "the JSON first syntax context has {int} marks")]
async fn json_first_syntax_context_has_marks(world: &mut AnvilWorld, expected: usize) {
    let json = world.json_response.as_ref().expect("JSON response");
    let marks = json["objects"][0]["context"]["marks"]
        .as_array()
        .expect("syntax context marks");

    assert_eq!(marks.len(), expected);
}

#[then(expr = "the syntax diagnostic code is {string}")]
async fn syntax_diagnostic_code_is(world: &mut AnvilWorld, expected: String) {
    let diagnostic = world.ast_diagnostic.as_ref().expect("syntax diagnostic");

    assert_eq!(diagnostic.code, expected);
}

#[then(expr = "the syntax diagnostic phase is {string}")]
async fn syntax_diagnostic_phase_is(world: &mut AnvilWorld, expected: String) {
    let diagnostic = world.ast_diagnostic.as_ref().expect("syntax diagnostic");
    let json = serde_json::to_value(diagnostic).expect("diagnostic JSON");

    assert_eq!(json["phase"], expected);
}

#[then(expr = "the syntax object diagnostic code is {string}")]
async fn syntax_object_diagnostic_code_is(world: &mut AnvilWorld, expected: String) {
    let diagnostic = world
        .syntax_diagnostic
        .as_ref()
        .expect("syntax object diagnostic");

    assert_eq!(diagnostic.code, expected);
}

#[then(expr = "the syntax object diagnostic phase is {string}")]
async fn syntax_object_diagnostic_phase_is(world: &mut AnvilWorld, expected: String) {
    let diagnostic = world
        .syntax_diagnostic
        .as_ref()
        .expect("syntax object diagnostic");
    let json = serde_json::to_value(diagnostic).expect("diagnostic JSON");

    assert_eq!(json["phase"], expected);
}

#[then(expr = "the module diagnostic code is {string}")]
async fn module_diagnostic_code_is(world: &mut AnvilWorld, expected: String) {
    let diagnostic = world.module_diagnostic.as_ref().expect("module diagnostic");

    assert_eq!(diagnostic.code, expected);
}

#[then(expr = "the module diagnostic phase is {string}")]
async fn module_diagnostic_phase_is(world: &mut AnvilWorld, expected: String) {
    let diagnostic = world.module_diagnostic.as_ref().expect("module diagnostic");
    let json = serde_json::to_value(diagnostic).expect("diagnostic JSON");

    assert_eq!(json["phase"], expected);
}

#[then(expr = "the module diagnostic expected candidates include {string}")]
async fn module_diagnostic_expected_candidates_include(world: &mut AnvilWorld, expected: String) {
    let diagnostic = world.module_diagnostic.as_ref().expect("module diagnostic");

    assert!(diagnostic.expected.contains(&expected));
}

#[then(expr = "the module diagnostic primary span starts at line {int} column {int}")]
async fn module_diagnostic_primary_span_starts_at(
    world: &mut AnvilWorld,
    expected_line: usize,
    expected_column: usize,
) {
    let diagnostic = world.module_diagnostic.as_ref().expect("module diagnostic");

    assert_eq!(diagnostic.primary_span.start.line, expected_line);
    assert_eq!(diagnostic.primary_span.start.column, expected_column);
}

#[then(expr = "the rendered diagnostic contains {string}")]
async fn rendered_diagnostic_contains(world: &mut AnvilWorld, expected: String) {
    let rendered = world
        .rendered_diagnostic
        .as_ref()
        .expect("rendered diagnostic");

    assert!(
        rendered.contains(&expected),
        "rendered diagnostic did not contain {expected:?}:\n{rendered}"
    );
}

#[then(expr = "the JSON buffered line count is {int}")]
async fn json_buffered_line_count_is(world: &mut AnvilWorld, expected: usize) {
    let json = world.json_response.as_ref().expect("JSON response");

    assert_eq!(json["buffered_lines"], expected);
}

fn parse_module_root_kind(value: &str) -> ModuleRootKind {
    match value {
        "package" => ModuleRootKind::Package,
        "draft" => ModuleRootKind::Draft,
        "workspace" => ModuleRootKind::Workspace,
        "locked-dependency" => ModuleRootKind::LockedDependency,
        "vendored-dependency" => ModuleRootKind::VendoredDependency,
        "standard-library" => ModuleRootKind::StandardLibrary,
        "host" => ModuleRootKind::Host,
        other => panic!("unknown module root kind {other}"),
    }
}

fn module_diagnostic_clone(diagnostic: &AstDiagnostic) -> Option<Box<ModuleDiagnostic>> {
    if diagnostic.phase == anvil_core::DiagnosticPhase::Module {
        Some(Box::new(diagnostic.clone()))
    } else {
        None
    }
}

#[tokio::main]
async fn main() {
    let specs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../specs");
    AnvilWorld::run(specs).await;
}
