use serde::Serialize;

use crate::{
    diagnostic::{Diagnostic, DiagnosticPhase, DiagnosticSpec},
    draft::DraftOverlay,
    source::{SourceLocation, SourceSpan, SourceText},
};

pub type ModuleDiagnostic = Diagnostic;
pub type ModuleResult<T> = Result<T, Box<ModuleDiagnostic>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleRootKind {
    Package,
    Draft,
    Workspace,
    LockedDependency,
    VendoredDependency,
    StandardLibrary,
    Host,
}

impl ModuleRootKind {
    const fn rank(self) -> u8 {
        match self {
            Self::Package => 0,
            Self::Draft => 1,
            Self::Workspace => 2,
            Self::LockedDependency => 3,
            Self::VendoredDependency => 4,
            Self::StandardLibrary => 5,
            Self::Host => 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModuleSource {
    pub module: String,
    pub root_kind: ModuleRootKind,
    pub root_name: String,
    pub path: String,
    #[serde(skip)]
    registration_index: usize,
}

impl ModuleSource {
    pub fn new(
        root_kind: ModuleRootKind,
        root_name: impl Into<String>,
        module: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        Self {
            module: module.into(),
            root_kind,
            root_name: root_name.into(),
            path: path.into(),
            registration_index: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModuleResolution {
    pub module: String,
    pub source_id: String,
    pub root_kind: ModuleRootKind,
    pub root_name: String,
    pub path: String,
    pub shadowed: Option<ModuleCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModuleCandidate {
    pub module: String,
    pub source_id: String,
    pub root_kind: ModuleRootKind,
    pub root_name: String,
    pub path: String,
}

#[derive(Debug, Clone, Default)]
pub struct ModuleResolver {
    sources: Vec<ModuleSource>,
}

impl ModuleResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_module(
        mut self,
        root_kind: ModuleRootKind,
        root_name: impl Into<String>,
        module: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        self.add_module(root_kind, root_name, module, path);
        self
    }

    pub fn with_default_path_module(
        self,
        root_kind: ModuleRootKind,
        root_name: impl Into<String>,
        module: impl Into<String>,
    ) -> Self {
        let module = module.into();
        let path = default_module_path(&module);
        self.with_module(root_kind, root_name, module, path)
    }

    pub fn with_draft_overlay(mut self, overlay: &DraftOverlay) -> Self {
        self.add_draft_overlay(overlay);
        self
    }

    pub fn add_module(
        &mut self,
        root_kind: ModuleRootKind,
        root_name: impl Into<String>,
        module: impl Into<String>,
        path: impl Into<String>,
    ) {
        self.add_source(ModuleSource::new(root_kind, root_name, module, path));
    }

    pub fn add_draft_overlay(&mut self, overlay: &DraftOverlay) {
        for source in overlay.module_sources() {
            self.add_source(source);
        }
    }

    pub fn add_source(&mut self, mut source: ModuleSource) {
        source.registration_index = self.sources.len();
        self.sources.push(source);
    }

    pub fn resolve(&self, module: &str) -> ModuleResult<ModuleResolution> {
        let request_source = SourceText::new("module-request", module);
        let request_span = request_span(module);

        self.resolve_in_source(module, &request_source, request_span)
    }

    pub fn resolve_in_source(
        &self,
        module: &str,
        request_source: &SourceText,
        request_span: SourceSpan,
    ) -> ModuleResult<ModuleResolution> {
        if !is_valid_module_name(module) {
            return Err(module_error(ModuleDiagnosticSpec {
                source: request_source,
                code: "ANVIL_MODULE_INVALID_NAME",
                message: format!("invalid module name {module:?}"),
                span: request_span,
                expected: vec!["dot-separated module name".to_string()],
                actual: Some(module.to_string()),
                suggestion: Some(
                    "Use non-empty module segments separated by dots, such as planner.search."
                        .to_string(),
                ),
            }));
        }

        let exact = self
            .sources
            .iter()
            .filter(|source| source.module == module)
            .collect::<Vec<_>>();

        if !exact.is_empty() {
            return self.resolve_exact(module, exact, request_source, request_span);
        }

        let short_candidates = self.module_candidates(module);
        if short_candidates.len() > 1 {
            return Err(module_error(ModuleDiagnosticSpec {
                source: request_source,
                code: "ANVIL_MODULE_AMBIGUOUS",
                message: format!("module name {module:?} is ambiguous"),
                span: request_span,
                expected: short_candidates
                    .iter()
                    .map(|candidate| candidate.module.clone())
                    .collect(),
                actual: Some(module.to_string()),
                suggestion: Some(
                    "Use a fully qualified module name or an explicit alias.".to_string(),
                ),
            }));
        }

        Err(module_error(ModuleDiagnosticSpec {
            source: request_source,
            code: "ANVIL_MODULE_NOT_FOUND",
            message: format!("module {module:?} was not found"),
            span: request_span,
            expected: short_candidates
                .iter()
                .map(|candidate| candidate.module.clone())
                .collect(),
            actual: Some(module.to_string()),
            suggestion: Some(
                "Check the package manifest, source roots, and module spelling.".to_string(),
            ),
        }))
    }

    pub fn module_candidates(&self, query: &str) -> Vec<ModuleCandidate> {
        let mut candidates = self
            .sources
            .iter()
            .filter(|source| source.module == query || module_short_name(&source.module) == query)
            .map(ModuleCandidate::from)
            .collect::<Vec<_>>();

        candidates.sort_by(|left, right| {
            (
                left.root_kind.rank(),
                left.root_name.as_str(),
                left.module.as_str(),
                left.path.as_str(),
            )
                .cmp(&(
                    right.root_kind.rank(),
                    right.root_name.as_str(),
                    right.module.as_str(),
                    right.path.as_str(),
                ))
        });
        candidates
    }

    pub fn sources(&self) -> &[ModuleSource] {
        &self.sources
    }

    fn resolve_exact(
        &self,
        module: &str,
        mut exact: Vec<&ModuleSource>,
        request_source: &SourceText,
        request_span: SourceSpan,
    ) -> ModuleResult<ModuleResolution> {
        exact.sort_by_key(|source| {
            (
                source.root_kind.rank(),
                source.registration_index,
                source.root_name.as_str(),
                source.path.as_str(),
            )
        });

        let best_rank = exact[0].root_kind.rank();
        let same_rank = exact
            .iter()
            .filter(|source| source.root_kind.rank() == best_rank)
            .collect::<Vec<_>>();

        if same_rank.len() > 1 {
            return Err(module_error(ModuleDiagnosticSpec {
                source: request_source,
                code: "ANVIL_MODULE_AMBIGUOUS",
                message: format!("module {module:?} has multiple candidates at the same precedence"),
                span: request_span,
                expected: same_rank
                    .iter()
                    .map(|source| source.source_id())
                    .collect(),
                actual: Some(module.to_string()),
                suggestion: Some("Use a more specific package/workspace selection or remove the duplicate module.".to_string()),
            }));
        }

        Ok(resolution_from_source(
            exact[0],
            shadowed_source(exact[0], &exact),
        ))
    }
}

impl ModuleSource {
    pub fn source_id(&self) -> String {
        format!(
            "{}:{}:{}",
            self.root_kind_label(),
            self.root_name,
            self.module
        )
    }

    fn root_kind_label(&self) -> &'static str {
        match self.root_kind {
            ModuleRootKind::Package => "package",
            ModuleRootKind::Draft => "draft",
            ModuleRootKind::Workspace => "workspace",
            ModuleRootKind::LockedDependency => "locked_dependency",
            ModuleRootKind::VendoredDependency => "vendored_dependency",
            ModuleRootKind::StandardLibrary => "standard_library",
            ModuleRootKind::Host => "host",
        }
    }
}

impl From<&ModuleSource> for ModuleResolution {
    fn from(source: &ModuleSource) -> Self {
        resolution_from_source(source, None)
    }
}

impl From<&ModuleSource> for ModuleCandidate {
    fn from(source: &ModuleSource) -> Self {
        Self {
            module: source.module.clone(),
            source_id: source.source_id(),
            root_kind: source.root_kind,
            root_name: source.root_name.clone(),
            path: source.path.clone(),
        }
    }
}

fn resolution_from_source(
    source: &ModuleSource,
    shadowed: Option<ModuleCandidate>,
) -> ModuleResolution {
    ModuleResolution {
        module: source.module.clone(),
        source_id: source.source_id(),
        root_kind: source.root_kind,
        root_name: source.root_name.clone(),
        path: source.path.clone(),
        shadowed,
    }
}

fn shadowed_source(source: &ModuleSource, candidates: &[&ModuleSource]) -> Option<ModuleCandidate> {
    if source.root_kind != ModuleRootKind::Draft {
        return None;
    }

    candidates
        .iter()
        .find(|candidate| candidate.root_kind.rank() > source.root_kind.rank())
        .map(|candidate| ModuleCandidate::from(*candidate))
}

struct ModuleDiagnosticSpec<'source> {
    source: &'source SourceText,
    code: &'static str,
    message: String,
    span: SourceSpan,
    expected: Vec<String>,
    actual: Option<String>,
    suggestion: Option<String>,
}

fn module_error(spec: ModuleDiagnosticSpec<'_>) -> Box<ModuleDiagnostic> {
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

fn request_span(module: &str) -> SourceSpan {
    SourceSpan {
        start: SourceLocation {
            offset: 0,
            line: 1,
            column: 1,
        },
        end: SourceLocation {
            offset: module.len(),
            line: 1,
            column: module.chars().count() + 1,
        },
    }
}

fn default_module_path(module: &str) -> String {
    format!("src/{}.anv", module.replace('.', "/"))
}

fn module_short_name(module: &str) -> &str {
    module.rsplit_once('.').map_or(module, |(_, short)| short)
}

fn is_valid_module_name(module: &str) -> bool {
    !module.is_empty()
        && module
            .split('.')
            .all(|segment| !segment.is_empty() && segment.chars().all(is_module_segment_char))
}

fn is_module_segment_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_package_before_workspace() {
        let resolver = ModuleResolver::new()
            .with_module(
                ModuleRootKind::Workspace,
                "tools",
                "planner.search",
                "packages/tools/src/planner/search.anv",
            )
            .with_module(
                ModuleRootKind::Package,
                "planner-tools",
                "planner.search",
                "src/planner/search.anv",
            );

        let resolution = resolver.resolve("planner.search").unwrap();

        assert_eq!(resolution.root_kind, ModuleRootKind::Package);
        assert_eq!(resolution.path, "src/planner/search.anv");
    }

    #[test]
    fn lets_drafts_shadow_workspace_modules() {
        let overlay = DraftOverlay::new("session-1", "agent.alpha")
            .with_module("planner.search", "(define answer 42)");
        let resolver = ModuleResolver::new()
            .with_default_path_module(ModuleRootKind::Workspace, "workspace", "planner.search")
            .with_draft_overlay(&overlay);

        let resolution = resolver.resolve("planner.search").unwrap();

        assert_eq!(resolution.root_kind, ModuleRootKind::Draft);
        assert_eq!(
            resolution.shadowed.map(|source| source.root_kind),
            Some(ModuleRootKind::Workspace)
        );
    }

    #[test]
    fn rejects_ambiguous_short_names() {
        let resolver = ModuleResolver::new()
            .with_default_path_module(ModuleRootKind::Workspace, "planner", "planner.search")
            .with_default_path_module(ModuleRootKind::Workspace, "agent", "agent.search");

        let diagnostic = resolver.resolve("search").unwrap_err();

        assert_eq!(diagnostic.code, "ANVIL_MODULE_AMBIGUOUS");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Module);
        assert_eq!(diagnostic.expected, vec!["agent.search", "planner.search"]);
    }

    #[test]
    fn reports_missing_modules() {
        let resolver = ModuleResolver::new();
        let diagnostic = resolver.resolve("missing.module").unwrap_err();

        assert_eq!(diagnostic.code, "ANVIL_MODULE_NOT_FOUND");
    }
}
