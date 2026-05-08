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
