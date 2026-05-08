use std::{collections::BTreeSet, fs, io, path::Path};

use serde::Serialize;

use crate::{
    AnvilManifest, ModuleResolver, ModuleRootKind, ModuleSource,
    diagnostic::{Diagnostic, DiagnosticPhase, DiagnosticSpec},
    parse_manifest_text,
    source::{SourceLocation, SourceSpan, SourceText},
};

pub type ProjectDiagnostic = Diagnostic;
pub type ProjectResult<T> = Result<T, Box<ProjectDiagnostic>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PackageSnapshot {
    pub manifest: AnvilManifest,
    pub sources: Vec<PackageSourceFile>,
}

impl PackageSnapshot {
    pub fn new(manifest: AnvilManifest) -> Self {
        Self {
            manifest,
            sources: Vec::new(),
        }
    }

    pub fn with_source(mut self, path: impl Into<String>, source: impl Into<String>) -> Self {
        self.add_source(path, source);
        self
    }

    pub fn add_source(&mut self, path: impl Into<String>, source: impl Into<String>) {
        let path = normalize_path(&path.into());
        let source = source.into();
        if let Some(existing) = self.sources.iter_mut().find(|source| source.path == path) {
            existing.source = source;
            return;
        }

        self.sources.push(PackageSourceFile { path, source });
    }

    pub fn module_sources(&self) -> Vec<ModuleSource> {
        package_module_sources(
            &self.manifest,
            self.sources.iter().map(|source| &source.path),
        )
    }

    pub fn module_resolver(&self) -> ModuleResolver {
        let mut resolver = ModuleResolver::new();
        for source in self.module_sources() {
            resolver.add_source(source);
        }
        resolver
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PackageSourceFile {
    pub path: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceSnapshot {
    pub root: PackageSnapshot,
    pub members: Vec<WorkspaceMemberSnapshot>,
}

impl WorkspaceSnapshot {
    pub fn module_resolver(&self) -> ModuleResolver {
        let mut resolver = self.root.module_resolver();
        for member in &self.members {
            for source in member.package.module_sources() {
                resolver.add_source(ModuleSource::new(
                    ModuleRootKind::Workspace,
                    source.root_name,
                    source.module,
                    join_workspace_path(&member.path, &source.path),
                ));
            }
        }

        resolver
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceMemberSnapshot {
    pub path: String,
    pub package: PackageSnapshot,
}

pub fn load_package_snapshot(root: impl AsRef<Path>) -> ProjectResult<PackageSnapshot> {
    let root = root.as_ref();
    let manifest_path = root.join("Anvil.toml");
    let manifest_text = read_project_file(
        &manifest_path,
        "ANVIL_PROJECT_MANIFEST_NOT_FOUND",
        "ANVIL_PROJECT_READ_MANIFEST",
        "Anvil.toml",
    )?;
    let manifest_source =
        SourceText::with_path("Anvil.toml", display_path(&manifest_path), manifest_text);
    let manifest = parse_manifest_text(&manifest_source)?;
    let mut snapshot = PackageSnapshot::new(manifest);

    for source_root in snapshot.manifest.source.roots.clone() {
        collect_source_root(root, &source_root, &mut snapshot)?;
    }
    collect_library_source(root, &mut snapshot)?;

    Ok(snapshot)
}

pub fn load_workspace_snapshot(root: impl AsRef<Path>) -> ProjectResult<WorkspaceSnapshot> {
    let root = root.as_ref();
    let root_package = load_package_snapshot(root)?;
    let mut member_paths = Vec::new();
    if let Some(workspace) = &root_package.manifest.workspace {
        for pattern in &workspace.members {
            member_paths.extend(expand_workspace_member_pattern(root, pattern)?);
        }
    }
    member_paths.sort();
    member_paths.dedup();

    let mut members = Vec::new();
    for member_path in member_paths {
        let member_root = root.join(&member_path);
        let manifest_path = member_root.join("Anvil.toml");
        if !manifest_path.exists() {
            return Err(project_error(ProjectDiagnosticSpec {
                source_id: "project",
                path: display_path(&manifest_path),
                code: "ANVIL_PROJECT_WORKSPACE_MEMBER_MANIFEST_NOT_FOUND",
                message: format!("workspace member {:?} is missing Anvil.toml", member_path),
                expected: vec!["workspace member manifest".to_string()],
                actual: Some("missing".to_string()),
                suggestion: Some(
                    "Add Anvil.toml to the member package or remove it from [workspace].members."
                        .to_string(),
                ),
            }));
        }

        members.push(WorkspaceMemberSnapshot {
            path: member_path,
            package: load_package_snapshot(member_root)?,
        });
    }

    Ok(WorkspaceSnapshot {
        root: root_package,
        members,
    })
}

pub fn package_module_sources<I, P>(manifest: &AnvilManifest, paths: I) -> Vec<ModuleSource>
where
    I: IntoIterator<Item = P>,
    P: AsRef<str>,
{
    let root_name = manifest.package.name.clone();
    let mut sources = Vec::new();
    let mut seen = BTreeSet::new();
    push_module_source(
        &mut sources,
        &mut seen,
        &root_name,
        &manifest.lib.module,
        &normalize_path(&manifest.lib.path),
    );

    let mut paths = paths
        .into_iter()
        .map(|path| normalize_path(path.as_ref()))
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();

    for path in paths {
        if path == normalize_path(&manifest.lib.path) || !path.ends_with(".anv") {
            continue;
        }
        let Some(module) = module_name_for_source_path(manifest, &path) else {
            continue;
        };

        push_module_source(&mut sources, &mut seen, &root_name, &module, &path);
    }

    sources
}

fn collect_library_source(
    package_root: &Path,
    snapshot: &mut PackageSnapshot,
) -> ProjectResult<()> {
    let lib_path = normalize_path(&snapshot.manifest.lib.path);
    if snapshot
        .sources
        .iter()
        .any(|source| source.path == lib_path)
    {
        return Ok(());
    }

    let path = package_root.join(&lib_path);
    let source = read_project_file(
        &path,
        "ANVIL_PROJECT_LIB_NOT_FOUND",
        "ANVIL_PROJECT_READ_LIB",
        "library source file",
    )?;

    snapshot.add_source(lib_path, source);
    Ok(())
}

fn collect_source_root(
    package_root: &Path,
    source_root: &str,
    snapshot: &mut PackageSnapshot,
) -> ProjectResult<()> {
    let root_path = package_root.join(source_root);
    if !root_path.exists() {
        return Err(project_error(ProjectDiagnosticSpec {
            source_id: "project",
            path: display_path(&root_path),
            code: "ANVIL_PROJECT_SOURCE_ROOT_NOT_FOUND",
            message: format!("source root {source_root:?} was not found"),
            expected: vec!["declared source root directory".to_string()],
            actual: Some("missing".to_string()),
            suggestion: Some(
                "Create the source root or remove it from [source].roots.".to_string(),
            ),
        }));
    }
    if !root_path.is_dir() {
        return Err(project_error(ProjectDiagnosticSpec {
            source_id: "project",
            path: display_path(&root_path),
            code: "ANVIL_PROJECT_SOURCE_ROOT_NOT_DIRECTORY",
            message: format!("source root {source_root:?} is not a directory"),
            expected: vec!["directory".to_string()],
            actual: Some("file".to_string()),
            suggestion: Some("Point [source].roots at directories.".to_string()),
        }));
    }

    collect_source_files(package_root, &root_path, snapshot)
}

fn collect_source_files(
    package_root: &Path,
    directory: &Path,
    snapshot: &mut PackageSnapshot,
) -> ProjectResult<()> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| project_io_error("ANVIL_PROJECT_READ_SOURCE_ROOT", directory, error))?
        .collect::<Result<Vec<_>, io::Error>>()
        .map_err(|error| project_io_error("ANVIL_PROJECT_READ_SOURCE_ROOT", directory, error))?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| project_io_error("ANVIL_PROJECT_READ_SOURCE_ROOT", &path, error))?;
        if file_type.is_dir() {
            collect_source_files(package_root, &path, snapshot)?;
        } else if file_type.is_file()
            && path.extension().is_some_and(|extension| extension == "anv")
        {
            let source = read_project_file(
                &path,
                "ANVIL_PROJECT_READ_SOURCE",
                "ANVIL_PROJECT_READ_SOURCE",
                "source file",
            )?;
            let relative_path = package_relative_path(package_root, &path);
            snapshot.add_source(relative_path, source);
        }
    }

    Ok(())
}

fn expand_workspace_member_pattern(
    workspace_root: &Path,
    pattern: &str,
) -> ProjectResult<Vec<String>> {
    let pattern = normalize_path(pattern);
    let segments = pattern
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let Some(wildcard_index) = segments.iter().position(|segment| segment.contains('*')) else {
        let member_root = workspace_root.join(&pattern);
        if !member_root.exists() {
            return Err(project_error(ProjectDiagnosticSpec {
                source_id: "project",
                path: display_path(&member_root),
                code: "ANVIL_PROJECT_WORKSPACE_MEMBER_NOT_FOUND",
                message: format!("workspace member {pattern:?} was not found"),
                expected: vec!["workspace member directory".to_string()],
                actual: Some("missing".to_string()),
                suggestion: Some("Check [workspace].members and the package path.".to_string()),
            }));
        }

        return Ok(vec![pattern]);
    };

    let base = join_segments(&segments[..wildcard_index]);
    let base_path = workspace_root.join(&base);
    if !base_path.exists() {
        return Err(project_error(ProjectDiagnosticSpec {
            source_id: "project",
            path: display_path(&base_path),
            code: "ANVIL_PROJECT_WORKSPACE_MEMBER_ROOT_NOT_FOUND",
            message: format!("workspace member root {base:?} was not found"),
            expected: vec!["workspace member root directory".to_string()],
            actual: Some("missing".to_string()),
            suggestion: Some(
                "Check [workspace].members and create the root directory.".to_string(),
            ),
        }));
    }
    if !base_path.is_dir() {
        return Err(project_error(ProjectDiagnosticSpec {
            source_id: "project",
            path: display_path(&base_path),
            code: "ANVIL_PROJECT_WORKSPACE_MEMBER_ROOT_NOT_DIRECTORY",
            message: format!("workspace member root {base:?} is not a directory"),
            expected: vec!["directory".to_string()],
            actual: Some("file".to_string()),
            suggestion: Some("Point wildcard workspace members at directories.".to_string()),
        }));
    }

    let wildcard = segments[wildcard_index];
    let suffix = join_segments(&segments[wildcard_index + 1..]);
    let mut members = Vec::new();
    let mut entries = fs::read_dir(&base_path)
        .map_err(|error| {
            project_io_error(
                "ANVIL_PROJECT_READ_WORKSPACE_MEMBER_ROOT",
                &base_path,
                error,
            )
        })?
        .collect::<Result<Vec<_>, io::Error>>()
        .map_err(|error| {
            project_io_error(
                "ANVIL_PROJECT_READ_WORKSPACE_MEMBER_ROOT",
                &base_path,
                error,
            )
        })?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let file_name = entry.file_name().to_string_lossy().to_string();
        if !matches_workspace_segment(wildcard, &file_name) {
            continue;
        }
        let mut member = join_segments(&[base.as_str(), file_name.as_str()]);
        if !suffix.is_empty() {
            member = join_workspace_path(&member, &suffix);
        }
        let member_path = workspace_root.join(&member);
        if member_path.is_dir() {
            members.push(member);
        }
    }

    Ok(members)
}

fn push_module_source(
    sources: &mut Vec<ModuleSource>,
    seen: &mut BTreeSet<(String, String)>,
    root_name: &str,
    module: &str,
    path: &str,
) {
    if !seen.insert((module.to_string(), path.to_string())) {
        return;
    }

    sources.push(ModuleSource::new(
        ModuleRootKind::Package,
        root_name,
        module,
        path,
    ));
}

fn module_name_for_source_path(manifest: &AnvilManifest, path: &str) -> Option<String> {
    for root in &manifest.source.roots {
        let root = normalize_path(root);
        let Some(relative_path) = strip_source_root(path, &root) else {
            continue;
        };
        let Some(stem) = relative_path.strip_suffix(".anv") else {
            continue;
        };
        if stem.is_empty() {
            continue;
        }

        return Some(stem.replace('/', "."));
    }

    None
}

fn strip_source_root<'path>(path: &'path str, root: &str) -> Option<&'path str> {
    if root.is_empty() {
        return Some(path).filter(|relative| !relative.is_empty());
    }

    path.strip_prefix(root)
        .and_then(|relative| relative.strip_prefix('/'))
        .filter(|relative| !relative.is_empty())
}

fn normalize_path(path: &str) -> String {
    let mut normalized = path.replace('\\', "/");
    while normalized.starts_with("./") {
        normalized.drain(..2);
    }
    normalized
}

fn join_segments(segments: &[&str]) -> String {
    segments
        .iter()
        .copied()
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

fn join_workspace_path(prefix: &str, suffix: &str) -> String {
    let prefix = normalize_path(prefix).trim_matches('/').to_string();
    let suffix = normalize_path(suffix).trim_matches('/').to_string();
    match (prefix.is_empty(), suffix.is_empty()) {
        (true, true) => String::new(),
        (true, false) => suffix,
        (false, true) => prefix,
        (false, false) => format!("{prefix}/{suffix}"),
    }
}

fn matches_workspace_segment(pattern: &str, value: &str) -> bool {
    let Some((prefix, suffix)) = pattern.split_once('*') else {
        return pattern == value;
    };

    value.starts_with(prefix) && value.ends_with(suffix)
}

fn read_project_file(
    path: &Path,
    not_found_code: &'static str,
    read_error_code: &'static str,
    expected: &str,
) -> ProjectResult<String> {
    fs::read_to_string(path).map_err(|error| {
        let code = if error.kind() == io::ErrorKind::NotFound {
            not_found_code
        } else {
            read_error_code
        };

        project_error(ProjectDiagnosticSpec {
            source_id: "project",
            path: display_path(path),
            code,
            message: format!("could not read {}", display_path(path)),
            expected: vec![expected.to_string()],
            actual: Some(error.to_string()),
            suggestion: Some("Check that the file exists and is readable.".to_string()),
        })
    })
}

struct ProjectDiagnosticSpec {
    source_id: &'static str,
    path: String,
    code: &'static str,
    message: String,
    expected: Vec<String>,
    actual: Option<String>,
    suggestion: Option<String>,
}

fn project_io_error(code: &'static str, path: &Path, error: io::Error) -> Box<ProjectDiagnostic> {
    project_error(ProjectDiagnosticSpec {
        source_id: "project",
        path: display_path(path),
        code,
        message: format!("could not inspect {}", display_path(path)),
        expected: vec!["readable project path".to_string()],
        actual: Some(error.to_string()),
        suggestion: Some("Check the project path and filesystem permissions.".to_string()),
    })
}

fn project_error(spec: ProjectDiagnosticSpec) -> Box<ProjectDiagnostic> {
    let source = SourceText::with_path(spec.source_id, spec.path.clone(), "");

    Diagnostic::new(DiagnosticSpec {
        code: spec.code,
        phase: DiagnosticPhase::Project,
        source: &source,
        message: spec.message,
        span: SourceSpan::point(SourceLocation::start()),
        expected: spec.expected,
        actual: spec.actual,
        suggestion: spec.suggestion,
    })
}

fn package_relative_path(package_root: &Path, path: &Path) -> String {
    path.strip_prefix(package_root)
        .map_or_else(|_| display_path(path), display_path)
}

fn display_path(path: impl AsRef<Path>) -> String {
    normalize_path(&path.as_ref().to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_manifest;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn registers_manifest_library_module_without_sources() {
        let snapshot = PackageSnapshot::new(test_manifest());
        let resolver = snapshot.module_resolver();

        let resolution = resolver.resolve("planner.tools").unwrap();

        assert_eq!(resolution.root_kind, ModuleRootKind::Package);
        assert_eq!(resolution.root_name, "planner-tools");
        assert_eq!(resolution.path, "src/lib.anv");
    }

    #[test]
    fn derives_modules_from_source_roots() {
        let snapshot = PackageSnapshot::new(test_manifest())
            .with_source("src/lib.anv", "(define answer 42)")
            .with_source("src/planner/search.anv", "(define search true)")
            .with_source("agents/agent/tool.anv", "(define tool true)");
        let resolver = snapshot.module_resolver();

        assert_eq!(
            resolver.resolve("planner.search").unwrap().path,
            "src/planner/search.anv"
        );
        assert_eq!(
            resolver.resolve("agent.tool").unwrap().path,
            "agents/agent/tool.anv"
        );
        assert_eq!(resolver.sources().len(), 3);
    }

    #[test]
    fn ignores_files_outside_source_roots() {
        let snapshot = PackageSnapshot::new(test_manifest())
            .with_source("scratch/planner/search.anv", "(define ignored true)")
            .with_source("src/planner/notes.md", "# ignored");
        let resolver = snapshot.module_resolver();

        let diagnostic = resolver.resolve("planner.search").unwrap_err();

        assert_eq!(diagnostic.code, "ANVIL_MODULE_NOT_FOUND");
        assert_eq!(resolver.sources().len(), 1);
    }

    #[test]
    fn loads_package_snapshot_from_filesystem() {
        let project = TestProject::new();
        project.write(
            "Anvil.toml",
            r#"
            [package]
            name = "planner-tools"
            version = "0.1.0"

            [lib]
            module = "planner.tools"
            path = "src/lib.anv"

            [source]
            roots = ["src", "agents"]
            "#,
        );
        project.write("src/lib.anv", "(define answer 42)");
        project.write("src/planner/search.anv", "(define search true)");
        project.write("agents/agent/tool.anv", "(define tool true)");

        let snapshot = load_package_snapshot(project.path()).unwrap();
        let resolver = snapshot.module_resolver();

        assert_eq!(snapshot.sources.len(), 3);
        assert_eq!(
            resolver.resolve("planner.search").unwrap().path,
            "src/planner/search.anv"
        );
        assert_eq!(
            resolver.resolve("agent.tool").unwrap().path,
            "agents/agent/tool.anv"
        );
    }

    #[test]
    fn reports_missing_filesystem_manifest() {
        let project = TestProject::new();
        let diagnostic = load_package_snapshot(project.path()).unwrap_err();

        assert_eq!(diagnostic.code, "ANVIL_PROJECT_MANIFEST_NOT_FOUND");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Project);
    }

    #[test]
    fn reports_missing_declared_source_root() {
        let project = TestProject::new();
        project.write(
            "Anvil.toml",
            r#"
            [package]
            name = "planner-tools"
            version = "0.1.0"

            [lib]
            module = "planner.tools"
            path = "src/lib.anv"

            [source]
            roots = ["src"]
            "#,
        );

        let diagnostic = load_package_snapshot(project.path()).unwrap_err();

        assert_eq!(diagnostic.code, "ANVIL_PROJECT_SOURCE_ROOT_NOT_FOUND");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Project);
    }

    #[test]
    fn reports_missing_declared_library_source() {
        let project = TestProject::new();
        project.write(
            "Anvil.toml",
            r#"
            [package]
            name = "planner-tools"
            version = "0.1.0"

            [lib]
            module = "planner.tools"
            path = "src/lib.anv"
            "#,
        );
        fs::create_dir_all(project.path().join("src")).expect("source root");

        let diagnostic = load_package_snapshot(project.path()).unwrap_err();

        assert_eq!(diagnostic.code, "ANVIL_PROJECT_LIB_NOT_FOUND");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Project);
    }

    #[test]
    fn loads_workspace_members_from_globs() {
        let project = TestProject::new();
        write_root_workspace(&project);
        write_member_package(
            &project,
            "packages/planner",
            "planner-tools",
            "planner.tools",
        );

        let workspace = load_workspace_snapshot(project.path()).unwrap();
        let resolver = workspace.module_resolver();

        assert_eq!(workspace.members.len(), 1);
        assert_eq!(
            resolver.resolve("planner.search").unwrap().root_kind,
            ModuleRootKind::Workspace
        );
        assert_eq!(
            resolver.resolve("planner.search").unwrap().path,
            "packages/planner/src/planner/search.anv"
        );
    }

    #[test]
    fn root_package_modules_shadow_workspace_members() {
        let project = TestProject::new();
        write_root_workspace(&project);
        project.write("src/planner/search.anv", "(define root-search true)");
        write_member_package(
            &project,
            "packages/planner",
            "planner-tools",
            "planner.tools",
        );

        let resolver = load_workspace_snapshot(project.path())
            .unwrap()
            .module_resolver();
        let resolution = resolver.resolve("planner.search").unwrap();

        assert_eq!(resolution.root_kind, ModuleRootKind::Package);
        assert_eq!(resolution.root_name, "root-tools");
        assert_eq!(resolution.path, "src/planner/search.anv");
    }

    #[test]
    fn reports_missing_workspace_member_manifest() {
        let project = TestProject::new();
        write_root_workspace(&project);
        project.write("packages/broken/src/lib.anv", "(define broken true)");

        let diagnostic = load_workspace_snapshot(project.path()).unwrap_err();

        assert_eq!(
            diagnostic.code,
            "ANVIL_PROJECT_WORKSPACE_MEMBER_MANIFEST_NOT_FOUND"
        );
        assert_eq!(diagnostic.phase, DiagnosticPhase::Project);
    }

    #[test]
    fn reports_ambiguous_workspace_modules_through_resolver() {
        let project = TestProject::new();
        write_root_workspace(&project);
        write_member_package(&project, "packages/alpha", "alpha-tools", "alpha.tools");
        write_member_package(&project, "packages/beta", "beta-tools", "beta.tools");

        let resolver = load_workspace_snapshot(project.path())
            .unwrap()
            .module_resolver();
        let diagnostic = resolver.resolve("planner.search").unwrap_err();

        assert_eq!(diagnostic.code, "ANVIL_MODULE_AMBIGUOUS");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Module);
    }

    fn test_manifest() -> AnvilManifest {
        parse_manifest(
            r#"
            [package]
            name = "planner-tools"
            version = "0.1.0"

            [lib]
            module = "planner.tools"
            path = "src/lib.anv"

            [source]
            roots = ["src", "agents"]
            "#,
        )
        .unwrap()
    }

    fn write_root_workspace(project: &TestProject) {
        project.write(
            "Anvil.toml",
            r#"
            [package]
            name = "root-tools"
            version = "0.1.0"

            [lib]
            module = "root.tools"
            path = "src/lib.anv"

            [workspace]
            members = ["packages/*"]
            "#,
        );
        project.write("src/lib.anv", "(define root true)");
    }

    fn write_member_package(
        project: &TestProject,
        path: &str,
        package_name: &str,
        lib_module: &str,
    ) {
        project.write(
            &format!("{path}/Anvil.toml"),
            &format!(
                r#"
                [package]
                name = "{package_name}"
                version = "0.1.0"

                [lib]
                module = "{lib_module}"
                path = "src/lib.anv"
                "#
            ),
        );
        project.write(&format!("{path}/src/lib.anv"), "(define member true)");
        project.write(
            &format!("{path}/src/planner/search.anv"),
            "(define search true)",
        );
    }

    struct TestProject {
        path: PathBuf,
    }

    impl TestProject {
        fn new() -> Self {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

            let unique_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "anvil-core-project-test-{}-{nanos}-{unique_id}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("test project directory");

            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write(&self, path: &str, source: &str) {
            let path = self.path.join(path);
            fs::create_dir_all(path.parent().expect("file parent")).expect("parent directory");
            fs::write(path, source).expect("test project file");
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
