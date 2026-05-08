use std::fmt;

use serde::Serialize;

use crate::{
    diagnostic::Diagnostic,
    reader::{SpannedDatum, format_datums, read_source_text},
    source::{SourceSpan, SourceText},
};

pub type SyntaxDiagnostic = Diagnostic;
pub type SyntaxResult<T> = Result<T, Box<SyntaxDiagnostic>>;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SyntaxObject {
    pub id: String,
    pub source_id: String,
    pub datum: SpannedDatum,
    pub span: SourceSpan,
    pub context: SyntaxContext,
}

impl fmt::Display for SyntaxObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.datum.fmt(f)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SyntaxContext {
    pub scopes: Vec<String>,
    pub marks: Vec<String>,
}

pub fn syntax_from_source(source: &str) -> SyntaxResult<Vec<SyntaxObject>> {
    syntax_from_source_text(&SourceText::repl(source))
}

pub fn syntax_from_source_text(source: &SourceText) -> SyntaxResult<Vec<SyntaxObject>> {
    let datums = read_source_text(source)?;
    syntax_from_datums(source, &datums)
}

pub fn syntax_from_datums(
    source: &SourceText,
    datums: &[SpannedDatum],
) -> SyntaxResult<Vec<SyntaxObject>> {
    Ok(datums
        .iter()
        .enumerate()
        .map(|(index, datum)| SyntaxObject {
            id: format!("{}:{}", source.id(), index + 1),
            source_id: source.id().to_string(),
            datum: datum.clone(),
            span: datum.span,
            context: SyntaxContext::default(),
        })
        .collect())
}

pub fn format_syntax_objects(objects: &[SyntaxObject]) -> String {
    let datums = objects
        .iter()
        .map(|object| object.datum.clone())
        .collect::<Vec<_>>();

    format_datums(&datums)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_reader_datums_with_source_identity() {
        let objects = syntax_from_source("(define answer 42)").unwrap();

        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].id, "repl:1");
        assert_eq!(objects[0].source_id, "repl");
        assert_eq!(objects[0].span.start.column, 1);
        assert_eq!(objects[0].to_string(), "(define answer 42)");
    }

    #[test]
    fn starts_with_empty_hygiene_context() {
        let objects = syntax_from_source("(fn [x] x)").unwrap();

        assert!(objects[0].context.scopes.is_empty());
        assert!(objects[0].context.marks.is_empty());
    }

    #[test]
    fn returns_reader_diagnostics() {
        let diagnostic = syntax_from_source("(define answer 42").unwrap_err();

        assert_eq!(diagnostic.code, "ANVIL_READER_UNCLOSED_DELIMITER");
    }
}
