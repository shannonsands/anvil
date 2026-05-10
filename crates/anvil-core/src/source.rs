use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceText {
    id: String,
    path: Option<String>,
    text: String,
}

impl SourceText {
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            path: None,
            text: text.into(),
        }
    }

    pub fn with_path(
        id: impl Into<String>,
        path: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            path: Some(path.into()),
            text: text.into(),
        }
    }

    pub fn repl(text: impl Into<String>) -> Self {
        Self::new("repl", text)
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SourceLocation {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}

impl SourceLocation {
    pub(crate) fn start() -> Self {
        Self {
            offset: 0,
            line: 1,
            column: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SourceSpan {
    pub start: SourceLocation,
    pub end: SourceLocation,
}

impl SourceSpan {
    pub(crate) fn new(start: SourceLocation, end: SourceLocation) -> Self {
        Self { start, end }
    }

    pub(crate) fn point(location: SourceLocation) -> Self {
        Self {
            start: location,
            end: location,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_text_exposes_optional_paths() {
        let source = SourceText::with_path("pkg:planner", "src/planner.anv", "(define x 1)");

        assert_eq!(source.id(), "pkg:planner");
        assert_eq!(source.path(), Some("src/planner.anv"));
        assert_eq!(source.text(), "(define x 1)");
        assert_eq!(SourceText::repl("42").path(), None);
    }
}
