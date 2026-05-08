use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use anvil_core::{
    AnvilManifest, AstDiagnostic, DraftOverlay, ManifestDiagnostic, ModuleDiagnostic,
    ModuleResolution, ModuleResolver, ModuleRootKind, PackageSnapshot, PackageSourceFile,
    ProjectDiagnostic, ReplInteraction, ReplResponse, ReplSession, SpannedAst, SyntaxDiagnostic,
    SyntaxObject, format_ast, format_datums, load_package_snapshot, load_workspace_snapshot,
    lower_source, lower_source_with_resolver, parse_manifest, read_repl_input, syntax_from_source,
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

#[when("the reader-backed REPL reads the input")]
async fn reader_repl_reads_input(world: &mut AnvilWorld) {
    world.repl_response = Some(read_repl_input(&world.source));
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

fn trim_docstring(value: &str) -> &str {
    value.trim_matches('\n')
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
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
