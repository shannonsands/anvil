use serde::{Deserialize, Serialize};

use crate::{
    diagnostic::{Diagnostic, DiagnosticPhase, DiagnosticSpec},
    source::{SourceLocation, SourceSpan, SourceText},
};

pub type ManifestDiagnostic = Diagnostic;
pub type ManifestResult<T> = Result<T, Box<ManifestDiagnostic>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnvilManifest {
    pub package: PackageManifest,
    pub lib: LibraryManifest,
    pub source: SourceRoots,
    pub workspace: Option<WorkspaceManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub edition: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryManifest {
    pub module: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRoots {
    #[serde(default = "default_source_roots")]
    pub roots: Vec<String>,
    #[serde(default = "default_test_roots")]
    pub tests: Vec<String>,
    #[serde(default = "default_eval_roots")]
    pub evals: Vec<String>,
    #[serde(default = "default_example_roots")]
    pub examples: Vec<String>,
}

impl Default for SourceRoots {
    fn default() -> Self {
        Self {
            roots: default_source_roots(),
            tests: default_test_roots(),
            evals: default_eval_roots(),
            examples: default_example_roots(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceManifest {
    #[serde(default)]
    pub members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawManifest {
    package: Option<PackageManifest>,
    lib: Option<LibraryManifest>,
    #[serde(default)]
    source: SourceRoots,
    workspace: Option<WorkspaceManifest>,
}

pub fn parse_manifest(source: &str) -> ManifestResult<AnvilManifest> {
    parse_manifest_text(&SourceText::new("Anvil.toml", source))
}

pub fn parse_manifest_text(source: &SourceText) -> ManifestResult<AnvilManifest> {
    let raw = toml::from_str::<RawManifest>(source.text()).map_err(|error| {
        manifest_error(ManifestDiagnosticSpec {
            source,
            code: "ANVIL_MANIFEST_PARSE",
            message: "Anvil.toml could not be parsed".to_string(),
            span: parse_error_span(source.text(), &error),
            expected: vec!["valid TOML".to_string()],
            actual: Some(error.message().to_string()),
            suggestion: Some("Fix the TOML syntax before loading the manifest.".to_string()),
        })
    })?;
    let package = raw.package.ok_or_else(|| {
        manifest_error(ManifestDiagnosticSpec {
            source,
            code: "ANVIL_MANIFEST_MISSING_FIELD",
            message: "Anvil.toml is missing [package]".to_string(),
            span: manifest_start_span(),
            expected: vec!["[package]".to_string()],
            actual: Some("missing".to_string()),
            suggestion: Some("Add a [package] table with name and version.".to_string()),
        })
    })?;
    let lib = raw.lib.ok_or_else(|| {
        manifest_error(ManifestDiagnosticSpec {
            source,
            code: "ANVIL_MANIFEST_MISSING_FIELD",
            message: "Anvil.toml is missing [lib]".to_string(),
            span: manifest_start_span(),
            expected: vec!["[lib]".to_string()],
            actual: Some("missing".to_string()),
            suggestion: Some("Add a [lib] table with module and path.".to_string()),
        })
    })?;

    Ok(AnvilManifest {
        package,
        lib,
        source: raw.source,
        workspace: raw.workspace,
    })
}

struct ManifestDiagnosticSpec<'source> {
    source: &'source SourceText,
    code: &'static str,
    message: String,
    span: SourceSpan,
    expected: Vec<String>,
    actual: Option<String>,
    suggestion: Option<String>,
}

fn manifest_error(spec: ManifestDiagnosticSpec<'_>) -> Box<ManifestDiagnostic> {
    Diagnostic::new(DiagnosticSpec {
        code: spec.code,
        phase: DiagnosticPhase::Manifest,
        source: spec.source,
        message: spec.message,
        span: spec.span,
        expected: spec.expected,
        actual: spec.actual,
        suggestion: spec.suggestion,
    })
}

fn parse_error_span(source: &str, error: &toml::de::Error) -> SourceSpan {
    let Some(span) = error.span() else {
        return manifest_start_span();
    };

    SourceSpan::new(
        location_at_offset(source, span.start),
        location_at_offset(source, span.end),
    )
}

fn location_at_offset(source: &str, offset: usize) -> SourceLocation {
    let mut location = SourceLocation::start();
    for (char_offset, ch) in source.char_indices() {
        if char_offset >= offset {
            break;
        }
        location.offset = char_offset + ch.len_utf8();
        if ch == '\n' {
            location.line += 1;
            location.column = 1;
        } else {
            location.column += 1;
        }
    }
    location
}

fn manifest_start_span() -> SourceSpan {
    SourceSpan::point(SourceLocation::start())
}

fn default_source_roots() -> Vec<String> {
    vec!["src".to_string()]
}

fn default_test_roots() -> Vec<String> {
    vec!["tests".to_string()]
}

fn default_eval_roots() -> Vec<String> {
    vec!["evals".to_string()]
}

fn default_example_roots() -> Vec<String> {
    vec!["examples".to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_manifest() {
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
        .unwrap();

        assert_eq!(manifest.package.name, "planner-tools");
        assert_eq!(manifest.lib.module, "planner.tools");
        assert_eq!(manifest.source.roots, vec!["src"]);
    }

    #[test]
    fn parses_source_and_workspace_roots() {
        let manifest = parse_manifest(
            r#"
            [package]
            name = "planner-tools"
            version = "0.1.0"

            [lib]
            module = "planner.tools"
            path = "src/lib.anv"

            [source]
            roots = ["src", "agents"]
            tests = ["tests"]
            evals = ["evals"]
            examples = ["examples"]

            [workspace]
            members = ["packages/*", "tools/*"]
            "#,
        )
        .unwrap();

        assert_eq!(manifest.source.roots, vec!["src", "agents"]);
        assert_eq!(
            manifest.workspace.expect("workspace manifest").members,
            vec!["packages/*", "tools/*"]
        );
    }

    #[test]
    fn reports_missing_package_table() {
        let diagnostic = parse_manifest(
            r#"
            [lib]
            module = "planner.tools"
            path = "src/lib.anv"
            "#,
        )
        .unwrap_err();

        assert_eq!(diagnostic.code, "ANVIL_MANIFEST_MISSING_FIELD");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Manifest);
    }

    #[test]
    fn reports_malformed_toml() {
        let diagnostic = parse_manifest(
            r#"
            [package]
            name = "planner-tools"
            version = "0.1.0"

            [lib
            module = "planner.tools"
            path = "src/lib.anv"
            "#,
        )
        .unwrap_err();

        assert_eq!(diagnostic.code, "ANVIL_MANIFEST_PARSE");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Manifest);
    }
}
