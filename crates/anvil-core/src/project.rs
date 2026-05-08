use std::collections::BTreeSet;

use serde::Serialize;

use crate::{AnvilManifest, ModuleResolver, ModuleRootKind, ModuleSource};

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
        self.sources.push(PackageSourceFile {
            path: path.into(),
            source: source.into(),
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_manifest;

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
}
