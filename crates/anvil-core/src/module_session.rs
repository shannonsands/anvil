use std::collections::{BTreeMap, BTreeSet};

use crate::{
    ast::{AstKind, RequireImport, SpannedAst, lower_source_text_with_resolver},
    capability::CapabilityProfile,
    diagnostic::{Diagnostic, DiagnosticPhase, DiagnosticSpec},
    draft::DraftOverlay,
    host::{HostCallContext, HostCallResult, HostFunctionRegistry, HostFunctionSpec},
    module::{ModuleResolution, ModuleResolver, ModuleRootKind, ModuleSource},
    project::{PackageSnapshot, WorkspaceSnapshot},
    source::{SourceSpan, SourceText},
    vm::{Value, VmBudget, VmOutput, VmSession},
};

pub type ModuleExecutionDiagnostic = Diagnostic;
pub type ModuleExecutionResult<T> = Result<T, Box<ModuleExecutionDiagnostic>>;

#[derive(Debug, Clone)]
pub struct ModuleSession {
    vm: VmSession,
    resolver: ModuleResolver,
    sources: BTreeMap<String, StoredModuleSource>,
    loaded: BTreeSet<String>,
    loading: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct StoredModuleSource {
    source: SourceText,
}

impl ModuleSession {
    pub fn new() -> Self {
        Self {
            vm: VmSession::new(),
            resolver: ModuleResolver::new(),
            sources: BTreeMap::new(),
            loaded: BTreeSet::new(),
            loading: Vec::new(),
        }
    }

    pub fn with_package_snapshot(snapshot: &PackageSnapshot) -> Self {
        let mut session = Self::new();
        session.add_package_snapshot(snapshot);
        session
    }

    pub fn with_workspace_snapshot(snapshot: &WorkspaceSnapshot) -> Self {
        let mut session = Self::new();
        session.add_workspace_snapshot(snapshot);
        session
    }

    pub fn add_package_snapshot(&mut self, snapshot: &PackageSnapshot) {
        for source in snapshot.module_sources() {
            if let Some(file) = snapshot
                .sources
                .iter()
                .find(|file| file.path == source.path)
            {
                self.add_module_source(source, file.source.clone());
            }
        }
    }

    pub fn add_workspace_snapshot(&mut self, snapshot: &WorkspaceSnapshot) {
        self.add_package_snapshot(&snapshot.root);

        for member in &snapshot.members {
            for source in member.package.module_sources() {
                if let Some(file) = member
                    .package
                    .sources
                    .iter()
                    .find(|file| file.path == source.path)
                {
                    self.add_module_source(
                        ModuleSource::new(
                            ModuleRootKind::Workspace,
                            source.root_name,
                            source.module,
                            join_workspace_path(&member.path, &source.path),
                        ),
                        file.source.clone(),
                    );
                }
            }
        }
    }

    pub fn add_draft_overlay(&mut self, overlay: &DraftOverlay) {
        for (source, module) in overlay.module_sources().into_iter().zip(&overlay.modules) {
            self.add_module_source(source, module.source.clone());
        }
    }

    pub fn add_module_source(&mut self, source: ModuleSource, text: impl Into<String>) {
        let source_id = source.source_id();
        let stored = StoredModuleSource {
            source: SourceText::with_path(source_id.clone(), source.path.clone(), text.into()),
        };

        if let Some(existing) = self.sources.get_mut(&source_id) {
            *existing = stored;
            return;
        }

        self.resolver.add_source(source);
        self.sources.insert(source_id, stored);
    }

    pub fn eval_source(&mut self, source: &str) -> ModuleExecutionResult<VmOutput> {
        self.eval_source_text(&SourceText::repl(source))
    }

    pub fn eval_source_text(&mut self, source: &SourceText) -> ModuleExecutionResult<VmOutput> {
        self.eval_source_text_with_budget(source, VmBudget::default())
    }

    pub fn eval_source_with_budget(
        &mut self,
        source: &str,
        budget: VmBudget,
    ) -> ModuleExecutionResult<VmOutput> {
        self.eval_source_text_with_budget(&SourceText::repl(source), budget)
    }

    pub fn eval_source_text_with_budget(
        &mut self,
        source: &SourceText,
        budget: VmBudget,
    ) -> ModuleExecutionResult<VmOutput> {
        let ast = lower_source_text_with_resolver(source, &self.resolver)?;
        self.eval_ast_source_text_with_budget(source, &ast, budget)
    }

    pub fn vm(&self) -> &VmSession {
        &self.vm
    }

    pub fn vm_mut(&mut self) -> &mut VmSession {
        &mut self.vm
    }

    pub fn binding(&self, name: &str) -> Option<&Value> {
        self.vm.binding(name)
    }

    pub fn register_host_function<F>(&mut self, spec: HostFunctionSpec, function: F)
    where
        F: Fn(&HostCallContext, &[Value]) -> HostCallResult + Send + Sync + 'static,
    {
        self.vm.register_host_function(spec, function);
    }

    pub fn with_host_function<F>(mut self, spec: HostFunctionSpec, function: F) -> Self
    where
        F: Fn(&HostCallContext, &[Value]) -> HostCallResult + Send + Sync + 'static,
    {
        self.register_host_function(spec, function);
        self
    }

    pub fn host_functions(&self) -> &HostFunctionRegistry {
        self.vm.host_functions()
    }

    pub fn set_capability_profile(&mut self, profile: CapabilityProfile) {
        self.vm.set_capability_profile(profile);
    }

    pub fn with_capability_profile(mut self, profile: CapabilityProfile) -> Self {
        self.set_capability_profile(profile);
        self
    }

    pub fn resolver(&self) -> &ModuleResolver {
        &self.resolver
    }

    pub fn loaded_source_ids(&self) -> &BTreeSet<String> {
        &self.loaded
    }

    pub fn is_loaded(&self, source_id: &str) -> bool {
        self.loaded.contains(source_id)
    }

    pub fn reset(&mut self) {
        self.vm.reset();
        self.loaded.clear();
        self.loading.clear();
    }

    fn eval_ast_source_text_with_budget(
        &mut self,
        source: &SourceText,
        ast: &[SpannedAst],
        budget: VmBudget,
    ) -> ModuleExecutionResult<VmOutput> {
        let executable_start = self.load_require_prefix(source, ast)?;
        self.vm
            .eval_ast_source_text_with_budget(source, &ast[executable_start..], budget)
    }

    fn load_require_prefix(
        &mut self,
        source: &SourceText,
        ast: &[SpannedAst],
    ) -> ModuleExecutionResult<usize> {
        let mut executable_start = ast.len();
        for (index, expression) in ast.iter().enumerate() {
            match &expression.kind {
                AstKind::Require { imports } if executable_start == ast.len() => {
                    for import in imports {
                        self.load_require_import(source, import)?;
                    }
                }
                AstKind::Require { .. } => {
                    return Err(module_session_error(ModuleSessionDiagnosticSpec {
                        source,
                        code: "ANVIL_MODULE_REQUIRE_ORDER",
                        message: "require forms must appear before executable forms".to_string(),
                        span: expression.span,
                        expected: vec!["top-level require prefix".to_string()],
                        actual: Some("require after executable form".to_string()),
                        suggestion: Some(
                            "Move require forms to the beginning of the module or REPL input."
                                .to_string(),
                        ),
                    }));
                }
                _ if executable_start == ast.len() => {
                    executable_start = index;
                }
                _ => {}
            }
        }

        Ok(executable_start)
    }

    fn load_require_import(
        &mut self,
        source: &SourceText,
        import: &RequireImport,
    ) -> ModuleExecutionResult<()> {
        if import.alias.is_some() {
            return Err(module_session_error(ModuleSessionDiagnosticSpec {
                source,
                code: "ANVIL_MODULE_ALIAS_UNSUPPORTED",
                message: "require aliases are not executable yet".to_string(),
                span: import.span,
                expected: vec!["bare module require".to_string()],
                actual: import.alias.clone(),
                suggestion: Some(
                    "Use a bare require for now; namespace aliases will land with module exports."
                        .to_string(),
                ),
            }));
        }

        let resolution = match &import.resolution {
            Some(resolution) => resolution.clone(),
            None => self
                .resolver
                .resolve_in_source(&import.module, source, import.span)?,
        };

        self.load_module_resolution(source, import.span, resolution)
    }

    fn load_module_resolution(
        &mut self,
        request_source: &SourceText,
        request_span: SourceSpan,
        resolution: ModuleResolution,
    ) -> ModuleExecutionResult<()> {
        if self.loaded.contains(&resolution.source_id) {
            return Ok(());
        }

        if self.loading.contains(&resolution.source_id) {
            return Err(module_session_error(ModuleSessionDiagnosticSpec {
                source: request_source,
                code: "ANVIL_MODULE_REQUIRE_CYCLE",
                message: format!("module require cycle includes {}", resolution.module),
                span: request_span,
                expected: self.loading.clone(),
                actual: Some(resolution.source_id),
                suggestion: Some(
                    "Break the cycle by moving shared definitions into a third module.".to_string(),
                ),
            }));
        }

        let stored = self
            .sources
            .get(&resolution.source_id)
            .cloned()
            .ok_or_else(|| {
                module_session_error(ModuleSessionDiagnosticSpec {
                    source: request_source,
                    code: "ANVIL_MODULE_SOURCE_NOT_AVAILABLE",
                    message: format!("module source is not available for {}", resolution.module),
                    span: request_span,
                    expected: vec![resolution.path.clone()],
                    actual: Some(resolution.source_id.clone()),
                    suggestion: Some(
                        "Load a package, workspace, or draft overlay that includes the module text."
                            .to_string(),
                    ),
                })
            })?;

        self.loading.push(resolution.source_id.clone());
        let result = self.eval_module_source(&stored);
        self.loading.pop();

        result?;
        self.loaded.insert(resolution.source_id);
        Ok(())
    }

    fn eval_module_source(
        &mut self,
        stored: &StoredModuleSource,
    ) -> ModuleExecutionResult<VmOutput> {
        let ast = lower_source_text_with_resolver(&stored.source, &self.resolver)?;
        self.eval_ast_source_text_with_budget(&stored.source, &ast, VmBudget::default())
    }
}

impl Default for ModuleSession {
    fn default() -> Self {
        Self::new()
    }
}

struct ModuleSessionDiagnosticSpec<'source> {
    source: &'source SourceText,
    code: &'static str,
    message: String,
    span: SourceSpan,
    expected: Vec<String>,
    actual: Option<String>,
    suggestion: Option<String>,
}

fn module_session_error(spec: ModuleSessionDiagnosticSpec<'_>) -> Box<ModuleExecutionDiagnostic> {
    Diagnostic::new(DiagnosticSpec {
        code: spec.code,
        phase: DiagnosticPhase::Module,
        source: spec.source,
        message: spec.message,
        span: spec.span,
        expected: spec.expected,
        actual: spec.actual,
        suggestion: spec.suggestion,
    })
}

fn join_workspace_path(member_path: &str, source_path: &str) -> String {
    let member_path = member_path.trim_end_matches('/');
    let source_path = source_path.trim_start_matches('/');
    if member_path.is_empty() {
        source_path.to_string()
    } else if source_path.is_empty() {
        member_path.to_string()
    } else {
        format!("{member_path}/{source_path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        draft::DraftOverlay, host::HostCallFailure, parse_manifest,
        project::WorkspaceMemberSnapshot,
    };

    fn planner_package() -> PackageSnapshot {
        let manifest = parse_manifest(
            r#"
            [package]
            name = "planner-tools"
            version = "0.1.0"

            [lib]
            module = "planner.tools"
            path = "src/lib.anv"
            "#,
        )
        .expect("manifest");

        PackageSnapshot::new(manifest)
            .with_source("src/lib.anv", "(define root true)")
            .with_source("src/planner/math.anv", "(define double (fn [x] (+ x x)))")
            .with_source(
                "src/planner/search.anv",
                "(require planner.math) (define answer (double 21))",
            )
    }

    fn workspace_snapshot() -> WorkspaceSnapshot {
        let root = planner_package();
        let member_manifest = parse_manifest(
            r#"
            [package]
            name = "planning-algos"
            version = "0.1.0"

            [lib]
            module = "planning.algos"
            path = "src/lib.anv"
            "#,
        )
        .expect("member manifest");

        let member_package = PackageSnapshot::new(member_manifest)
            .with_source("src/lib.anv", "(define workspace-root true)")
            .with_source("src/planning/graph.anv", "(define graph-weight 7)");

        WorkspaceSnapshot {
            root,
            members: vec![WorkspaceMemberSnapshot {
                path: "packages/planning-algos".to_string(),
                package: member_package,
            }],
        }
    }

    #[test]
    fn require_loads_package_module_bindings_into_session() {
        let package = planner_package();
        let mut session = ModuleSession::with_package_snapshot(&package);

        let output = session
            .eval_source("(require planner.search) answer")
            .expect("module session eval");

        assert_eq!(output.value.to_string(), "42");
        assert!(session.binding("double").is_some());
        assert_eq!(session.binding("answer").unwrap().to_string(), "42");
    }

    #[test]
    fn require_loads_workspace_member_modules() {
        let workspace = workspace_snapshot();
        let mut session = ModuleSession::with_workspace_snapshot(&workspace);

        let output = session
            .eval_source("(require planning.graph) graph-weight")
            .expect("workspace module eval");

        assert_eq!(output.value, Value::Integer(7));
        assert!(session.is_loaded("workspace:planning-algos:planning.graph"));
        assert_eq!(
            session
                .resolver()
                .resolve("planning.graph")
                .expect("workspace resolution")
                .path,
            "packages/planning-algos/src/planning/graph.anv"
        );
    }

    #[test]
    fn require_loads_draft_overlay_modules() {
        let overlay = DraftOverlay::new("session-1", "agent.alpha")
            .with_module("draft.tool", "(define draft-answer 99)");
        let mut session = ModuleSession::new();
        session.add_draft_overlay(&overlay);

        let output = session
            .eval_source("(require draft.tool) draft-answer")
            .expect("draft module eval");

        assert_eq!(output.value, Value::Integer(99));
        assert!(session.is_loaded("draft:session-1:draft.tool"));
    }

    #[test]
    fn add_module_source_replaces_unloaded_source_text() {
        let source = ModuleSource::new(
            ModuleRootKind::Package,
            "planner-tools",
            "planner.replace",
            "src/planner/replace.anv",
        );
        let mut session = ModuleSession::new();
        session.add_module_source(source.clone(), "(define replacement-answer 1)");
        session.add_module_source(source, "(define replacement-answer 2)");

        let output = session
            .eval_source("(require planner.replace) replacement-answer")
            .expect("replacement module eval");

        assert_eq!(output.value, Value::Integer(2));
        assert_eq!(session.resolver().sources().len(), 1);
    }

    #[test]
    fn required_modules_execute_once_per_session() {
        let source = ModuleSource::new(
            ModuleRootKind::Package,
            "planner-tools",
            "planner.once",
            "src/planner/once.anv",
        );
        let mut session = ModuleSession::new();
        session.add_module_source(source.clone(), "(define stable-answer 42)");
        session
            .eval_source("(require planner.once) stable-answer")
            .expect("initial require");

        session.add_module_source(source, "(define stable-answer missing)");
        let output = session
            .eval_source("(require planner.once) stable-answer")
            .expect("loaded module is not re-executed");

        assert_eq!(output.value, Value::Integer(42));
        assert_eq!(session.loaded_source_ids().len(), 1);
    }

    #[test]
    fn eval_source_text_and_budgeted_eval_use_vm_session_semantics() {
        let mut session = ModuleSession::new();
        let source = SourceText::with_path(
            "script:main",
            "scripts/main.anv",
            "(define answer 42) answer",
        );

        let output = session
            .eval_source_text(&source)
            .expect("source text evaluation");
        assert_eq!(output.value, Value::Integer(42));

        let diagnostic = session
            .eval_source_with_budget("answer", VmBudget::with_instruction_fuel(0))
            .expect_err("budget diagnostic");
        assert_eq!(diagnostic.code, "ANVIL_RUNTIME_FUEL_EXHAUSTED");

        let text_source = SourceText::new("budgeted", "answer");
        let output = session
            .eval_source_text_with_budget(&text_source, VmBudget::unlimited())
            .expect("budgeted source text evaluation");
        assert_eq!(output.value, Value::Integer(42));
        assert_eq!(session.vm().binding("answer"), Some(&Value::Integer(42)));
    }

    #[test]
    fn host_functions_and_profiles_pass_through_to_vm_session() {
        let profile = CapabilityProfile::new("math", "agent.alpha", "project.markodb")
            .with_capability("host/math");
        let mut session = ModuleSession::new()
            .with_host_function(
                HostFunctionSpec::new("host/add")
                    .with_exact_arity(2)
                    .with_required_capability("host/math")
                    .with_trust_zone("project.markodb"),
                |_context, args| {
                    let [Value::Integer(left), Value::Integer(right)] = args else {
                        return Err(HostCallFailure::new("expected integer arguments"));
                    };
                    Ok(Value::Integer(left + right))
                },
            )
            .with_capability_profile(profile);

        assert!(session.host_functions().contains("host/add"));
        assert!(session.vm().capability_profile().is_some());
        assert!(session.vm_mut().host_functions().contains("host/add"));

        let output = session
            .eval_source("(host/add 40 2)")
            .expect("module host function call");
        assert_eq!(output.value, Value::Integer(42));
    }

    #[test]
    fn reset_clears_vm_bindings_loaded_modules_and_loading_stack() {
        let package = planner_package();
        let mut session = ModuleSession::with_package_snapshot(&package);
        session
            .eval_source("(require planner.search) answer")
            .expect("module load");

        assert!(session.binding("answer").is_some());
        assert!(!session.loaded_source_ids().is_empty());

        session.reset();

        assert!(session.binding("answer").is_none());
        assert!(session.loaded_source_ids().is_empty());

        let output = session
            .eval_source("(require planner.search) answer")
            .expect("module reload after reset");
        assert_eq!(output.value, Value::Integer(42));
    }

    #[test]
    fn require_after_executable_form_is_diagnosed() {
        let package = planner_package();
        let mut session = ModuleSession::with_package_snapshot(&package);

        let diagnostic = session
            .eval_source("(define local 1) (require planner.search)")
            .expect_err("require order diagnostic");

        assert_eq!(diagnostic.code, "ANVIL_MODULE_REQUIRE_ORDER");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Module);
    }

    #[test]
    fn require_aliases_are_explicit_future_work() {
        let package = planner_package();
        let mut session = ModuleSession::with_package_snapshot(&package);

        let diagnostic = session
            .eval_source("(require [planner.search :as search])")
            .expect_err("alias diagnostic");

        assert_eq!(diagnostic.code, "ANVIL_MODULE_ALIAS_UNSUPPORTED");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Module);
    }

    #[test]
    fn missing_module_source_text_is_diagnosed() {
        let mut session = ModuleSession::new();
        session.resolver.add_source(ModuleSource::new(
            ModuleRootKind::Package,
            "planner-tools",
            "planner.missing_text",
            "src/planner/missing_text.anv",
        ));

        let diagnostic = session
            .eval_source("(require planner.missing_text)")
            .expect_err("missing text diagnostic");

        assert_eq!(diagnostic.code, "ANVIL_MODULE_SOURCE_NOT_AVAILABLE");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Module);
    }

    #[test]
    fn failed_required_module_does_not_corrupt_existing_session_state() {
        let mut package = planner_package();
        package.add_source("src/planner/bad.anv", "(define broken missing)");
        let mut session = ModuleSession::with_package_snapshot(&package);

        session
            .eval_source("(define answer 42)")
            .expect("initial eval");
        let diagnostic = session
            .eval_source("(require planner.bad)")
            .expect_err("bad module diagnostic");

        assert_eq!(diagnostic.code, "ANVIL_RUNTIME_UNBOUND_SYMBOL");
        assert_eq!(
            session
                .eval_source("answer")
                .expect("session survived")
                .value
                .to_string(),
            "42"
        );
    }

    #[test]
    fn detects_require_cycles() {
        let mut package = planner_package();
        package.add_source("src/planner/a.anv", "(require planner.b) (define a 1)");
        package.add_source("src/planner/b.anv", "(require planner.a) (define b 2)");
        let mut session = ModuleSession::with_package_snapshot(&package);

        let diagnostic = session
            .eval_source("(require planner.a)")
            .expect_err("cycle diagnostic");

        assert_eq!(diagnostic.code, "ANVIL_MODULE_REQUIRE_CYCLE");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Module);
    }

    #[test]
    fn workspace_path_join_handles_empty_segments() {
        assert_eq!(join_workspace_path("", "src/lib.anv"), "src/lib.anv");
        assert_eq!(join_workspace_path("packages/tools", ""), "packages/tools");
        assert_eq!(
            join_workspace_path("packages/tools/", "/src/lib.anv"),
            "packages/tools/src/lib.anv"
        );
    }
}
