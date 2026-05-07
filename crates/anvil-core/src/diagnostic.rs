use serde::Serialize;

use crate::source::{SourceSpan, SourceText};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticPhase {
    Reader,
    Syntax,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticLabel {
    pub span: SourceSpan,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticSuggestion {
    pub message: String,
    pub replacement: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticCodeFrame {
    pub source_id: String,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub snippet: String,
    pub marker: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: DiagnosticSeverity,
    pub phase: DiagnosticPhase,
    pub message: String,
    pub source_id: String,
    pub primary_span: SourceSpan,
    pub span: SourceSpan,
    pub labels: Vec<DiagnosticLabel>,
    pub expected: Vec<String>,
    pub actual: Option<String>,
    pub suggestion: Option<String>,
    pub suggestions: Vec<DiagnosticSuggestion>,
    pub code_frame: Option<DiagnosticCodeFrame>,
}

pub(crate) struct DiagnosticSpec<'source> {
    pub code: &'static str,
    pub phase: DiagnosticPhase,
    pub source: &'source SourceText,
    pub message: String,
    pub span: SourceSpan,
    pub expected: Vec<String>,
    pub actual: Option<String>,
    pub suggestion: Option<String>,
}

impl Diagnostic {
    pub(crate) fn new(spec: DiagnosticSpec<'_>) -> Box<Self> {
        let suggestions = spec
            .suggestion
            .iter()
            .map(|message| DiagnosticSuggestion {
                message: message.clone(),
                replacement: None,
            })
            .collect();

        Box::new(Self {
            code: spec.code,
            severity: DiagnosticSeverity::Error,
            phase: spec.phase,
            message: spec.message.clone(),
            source_id: spec.source.id().to_string(),
            primary_span: spec.span,
            span: spec.span,
            labels: vec![DiagnosticLabel {
                span: spec.span,
                message: spec.message.clone(),
            }],
            expected: spec.expected,
            actual: spec.actual,
            suggestion: spec.suggestion,
            suggestions,
            code_frame: build_code_frame(spec.source.id(), spec.source.text(), spec.span),
        })
    }

    pub fn is_incomplete_input(&self) -> bool {
        matches!(
            self.code,
            "ANVIL_READER_UNCLOSED_DELIMITER"
                | "ANVIL_READER_UNTERMINATED_STRING"
                | "ANVIL_READER_QUOTE_WITHOUT_DATUM"
        )
    }

    pub fn render_code_frame(&self) -> String {
        let severity = match self.severity {
            DiagnosticSeverity::Error => "error",
        };
        let mut rendered = format!(
            "{severity} {}: {}\n --> {}:{}:{}",
            self.code,
            self.message,
            self.source_id,
            self.primary_span.start.line,
            self.primary_span.start.column,
        );

        if let Some(frame) = &self.code_frame {
            rendered.push_str("\n  |");
            rendered.push_str(&format!("\n{} | {}", frame.line, frame.snippet));
            rendered.push_str(&format!("\n  | {}", frame.marker));
        }

        if !self.expected.is_empty() {
            rendered.push_str(&format!("\n  = expected {}", self.expected.join(", ")));
        }
        if let Some(actual) = &self.actual {
            rendered.push_str(&format!("\n  = actual {actual}"));
        }
        if let Some(suggestion) = &self.suggestion {
            rendered.push_str(&format!("\n  = suggestion {suggestion}"));
        }

        rendered
    }
}

fn build_code_frame(
    source_id: &str,
    source_text: &str,
    span: SourceSpan,
) -> Option<DiagnosticCodeFrame> {
    let line = source_text
        .lines()
        .nth(span.start.line.saturating_sub(1))
        .unwrap_or("");
    let marker_width = if span.start.line == span.end.line {
        span.end.column.saturating_sub(span.start.column).max(1)
    } else {
        1
    };
    let marker = format!(
        "{}{}",
        " ".repeat(span.start.column.saturating_sub(1)),
        "^".repeat(marker_width)
    );

    Some(DiagnosticCodeFrame {
        source_id: source_id.to_string(),
        line: span.start.line,
        column: span.start.column,
        end_line: span.end.line,
        end_column: span.end.column,
        snippet: line.to_string(),
        marker,
    })
}
