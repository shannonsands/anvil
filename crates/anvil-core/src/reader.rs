use std::fmt;

use serde::Serialize;

use crate::{
    diagnostic::{Diagnostic, DiagnosticPhase, DiagnosticSpec},
    source::{SourceLocation, SourceSpan, SourceText},
};

pub type ReaderDiagnostic = Diagnostic;
pub type ReaderResult<T> = Result<T, Box<ReaderDiagnostic>>;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpannedDatum {
    pub datum: Datum,
    pub span: SourceSpan,
}

impl fmt::Display for SpannedDatum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.datum.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Datum {
    Nil,
    Bool(bool),
    Integer(i64),
    Float64(f64),
    String(String),
    Symbol(String),
    Keyword(String),
    List(Vec<SpannedDatum>),
    Vector(Vec<SpannedDatum>),
    Map(Vec<(SpannedDatum, SpannedDatum)>),
    Quote(Box<SpannedDatum>),
}

impl fmt::Display for Datum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nil => f.write_str("nil"),
            Self::Bool(value) => write!(f, "{value}"),
            Self::Integer(value) => write!(f, "{value}"),
            Self::Float64(value) => write!(f, "{value}"),
            Self::String(value) => write!(f, "\"{}\"", escape_string(value)),
            Self::Symbol(value) => f.write_str(value),
            Self::Keyword(value) => write!(f, ":{value}"),
            Self::List(items) => write_sequence(f, "(", items.iter(), ")"),
            Self::Vector(items) => write_sequence(f, "[", items.iter(), "]"),
            Self::Map(pairs) => {
                f.write_str("{")?;
                for (index, (key, value)) in pairs.iter().enumerate() {
                    if index > 0 {
                        f.write_str(" ")?;
                    }
                    write!(f, "{key} {value}")?;
                }
                f.write_str("}")
            }
            Self::Quote(datum) => write!(f, "'{datum}"),
        }
    }
}

fn write_sequence<'a>(
    f: &mut fmt::Formatter<'_>,
    open: &str,
    mut items: impl Iterator<Item = &'a SpannedDatum>,
    close: &str,
) -> fmt::Result {
    f.write_str(open)?;
    if let Some(first) = items.next() {
        write!(f, "{first}")?;
        for item in items {
            write!(f, " {item}")?;
        }
    }
    f.write_str(close)
}

fn escape_string(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

pub fn read_source(source: &str) -> ReaderResult<Vec<SpannedDatum>> {
    read_source_text(&SourceText::repl(source))
}

pub fn read_source_text(source: &SourceText) -> ReaderResult<Vec<SpannedDatum>> {
    let lexed = lex_source(source)?;
    Parser::new(source, lexed.tokens, lexed.eof).parse_all()
}

pub fn format_datums(datums: &[SpannedDatum]) -> String {
    datums
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LexedSource {
    tokens: Vec<Token>,
    eof: SourceLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    kind: TokenKind,
    span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenKind {
    Open(Delimiter),
    Close(Delimiter),
    Quote,
    Atom(String),
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Delimiter {
    List,
    Vector,
    Map,
}

impl Delimiter {
    fn open(self) -> &'static str {
        match self {
            Self::List => "(",
            Self::Vector => "[",
            Self::Map => "{",
        }
    }

    fn close(self) -> &'static str {
        match self {
            Self::List => ")",
            Self::Vector => "]",
            Self::Map => "}",
        }
    }
}

fn lex_source(source: &SourceText) -> ReaderResult<LexedSource> {
    let chars: Vec<(usize, char)> = source.text().char_indices().collect();
    let mut tokens = Vec::new();
    let mut index = 0;
    let mut location = SourceLocation::start();

    while index < chars.len() {
        let (_, ch) = chars[index];

        if is_reader_whitespace(ch) {
            advance(&chars, &mut index, &mut location);
            continue;
        }

        if ch == ';' {
            while index < chars.len() {
                let (_, comment_ch) = chars[index];
                advance(&chars, &mut index, &mut location);
                if comment_ch == '\n' {
                    break;
                }
            }
            continue;
        }

        let start = location;
        match ch {
            '(' => {
                advance(&chars, &mut index, &mut location);
                tokens.push(token(TokenKind::Open(Delimiter::List), start, location));
            }
            ')' => {
                advance(&chars, &mut index, &mut location);
                tokens.push(token(TokenKind::Close(Delimiter::List), start, location));
            }
            '[' => {
                advance(&chars, &mut index, &mut location);
                tokens.push(token(TokenKind::Open(Delimiter::Vector), start, location));
            }
            ']' => {
                advance(&chars, &mut index, &mut location);
                tokens.push(token(TokenKind::Close(Delimiter::Vector), start, location));
            }
            '{' => {
                advance(&chars, &mut index, &mut location);
                tokens.push(token(TokenKind::Open(Delimiter::Map), start, location));
            }
            '}' => {
                advance(&chars, &mut index, &mut location);
                tokens.push(token(TokenKind::Close(Delimiter::Map), start, location));
            }
            '\'' => {
                advance(&chars, &mut index, &mut location);
                tokens.push(token(TokenKind::Quote, start, location));
            }
            '"' => tokens.push(read_string(
                source,
                &chars,
                &mut index,
                &mut location,
                start,
            )?),
            _ => tokens.push(read_atom(
                source.text(),
                &chars,
                &mut index,
                &mut location,
                start,
            )),
        }
    }

    Ok(LexedSource {
        tokens,
        eof: location,
    })
}

fn token(kind: TokenKind, start: SourceLocation, end: SourceLocation) -> Token {
    Token {
        kind,
        span: SourceSpan::new(start, end),
    }
}

fn read_string(
    source: &SourceText,
    chars: &[(usize, char)],
    index: &mut usize,
    location: &mut SourceLocation,
    start: SourceLocation,
) -> ReaderResult<Token> {
    advance(chars, index, location);
    let mut value = String::new();

    while *index < chars.len() {
        let (_, ch) = chars[*index];

        match ch {
            '"' => {
                advance(chars, index, location);
                return Ok(token(TokenKind::String(value), start, *location));
            }
            '\\' => {
                advance(chars, index, location);
                if *index >= chars.len() {
                    return Err(Diagnostic::new(DiagnosticSpec {
                        phase: DiagnosticPhase::Reader,
                        code: "ANVIL_READER_UNTERMINATED_STRING",
                        source,
                        message: "string escape reaches end of input".to_string(),
                        span: SourceSpan::new(start, *location),
                        expected: vec!["escaped character".to_string()],
                        actual: Some("end of input".to_string()),
                        suggestion: Some(
                            "Add an escaped character or close the string.".to_string(),
                        ),
                    }));
                }

                let (_, escaped) = chars[*index];
                let escaped_value = match escaped {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '"' => '"',
                    '\\' => '\\',
                    other => {
                        return Err(Diagnostic::new(DiagnosticSpec {
                            phase: DiagnosticPhase::Reader,
                            code: "ANVIL_READER_UNKNOWN_ESCAPE",
                            source,
                            message: format!("unknown string escape \\{other}"),
                            span: SourceSpan::new(
                                *location,
                                next_location(source.text(), chars, *index),
                            ),
                            expected: vec![
                                "n".to_string(),
                                "r".to_string(),
                                "t".to_string(),
                                "\"".to_string(),
                                "\\".to_string(),
                            ],
                            actual: Some(other.to_string()),
                            suggestion: Some(
                                "Use a supported escape or remove the backslash.".to_string(),
                            ),
                        }));
                    }
                };
                value.push(escaped_value);
                advance(chars, index, location);
            }
            other => {
                value.push(other);
                advance(chars, index, location);
            }
        }
    }

    Err(Diagnostic::new(DiagnosticSpec {
        phase: DiagnosticPhase::Reader,
        code: "ANVIL_READER_UNTERMINATED_STRING",
        source,
        message: "string literal is missing a closing quote".to_string(),
        span: SourceSpan::new(start, *location),
        expected: vec!["\"".to_string()],
        actual: Some("end of input".to_string()),
        suggestion: Some("Add a closing quote.".to_string()),
    }))
}

fn read_atom(
    source: &str,
    chars: &[(usize, char)],
    index: &mut usize,
    location: &mut SourceLocation,
    start: SourceLocation,
) -> Token {
    let start_offset = start.offset;

    while *index < chars.len() {
        let (_, ch) = chars[*index];
        if is_reader_whitespace(ch) || is_syntax_boundary(ch) || ch == ';' || ch == '"' {
            break;
        }
        advance(chars, index, location);
    }

    let text = source[start_offset..location.offset].to_string();
    token(TokenKind::Atom(text), start, *location)
}

fn is_reader_whitespace(ch: char) -> bool {
    ch.is_whitespace() || ch == ','
}

fn is_syntax_boundary(ch: char) -> bool {
    matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | '\'')
}

fn advance(chars: &[(usize, char)], index: &mut usize, location: &mut SourceLocation) {
    let (_, ch) = chars[*index];
    *index += 1;
    let next_offset = chars
        .get(*index)
        .map(|(offset, _)| *offset)
        .unwrap_or_else(|| location.offset + ch.len_utf8());

    location.offset = next_offset;
    if ch == '\n' {
        location.line += 1;
        location.column = 1;
    } else {
        location.column += 1;
    }
}

fn next_location(source: &str, chars: &[(usize, char)], index: usize) -> SourceLocation {
    let mut location = SourceLocation::start();
    let mut cursor = 0;
    while cursor <= index && cursor < chars.len() {
        advance(chars, &mut cursor, &mut location);
    }
    if index >= chars.len() {
        location.offset = source.len();
    }
    location
}

struct Parser<'source> {
    source: &'source SourceText,
    tokens: Vec<Token>,
    cursor: usize,
    eof: SourceLocation,
}

impl<'source> Parser<'source> {
    fn new(source: &'source SourceText, tokens: Vec<Token>, eof: SourceLocation) -> Self {
        Self {
            source,
            tokens,
            cursor: 0,
            eof,
        }
    }

    fn parse_all(&mut self) -> ReaderResult<Vec<SpannedDatum>> {
        let mut datums = Vec::new();
        while self.cursor < self.tokens.len() {
            datums.push(self.parse_datum()?);
        }
        Ok(datums)
    }

    fn parse_datum(&mut self) -> ReaderResult<SpannedDatum> {
        let token = self.next_token()?;

        match token.kind {
            TokenKind::Open(delimiter) => self.parse_collection(delimiter, token.span),
            TokenKind::Close(delimiter) => Err(Diagnostic::new(DiagnosticSpec {
                phase: DiagnosticPhase::Reader,
                code: "ANVIL_READER_UNEXPECTED_DELIMITER",
                source: self.source,
                message: format!("unexpected closing delimiter {}", delimiter.close()),
                span: token.span,
                expected: vec!["datum".to_string()],
                actual: Some(delimiter.close().to_string()),
                suggestion: Some(format!(
                    "Remove this {} or add a matching {} earlier.",
                    delimiter.close(),
                    delimiter.open()
                )),
            })),
            TokenKind::Quote => {
                if self.cursor >= self.tokens.len() {
                    return Err(Diagnostic::new(DiagnosticSpec {
                        phase: DiagnosticPhase::Reader,
                        code: "ANVIL_READER_QUOTE_WITHOUT_DATUM",
                        source: self.source,
                        message: "quote marker is missing a datum".to_string(),
                        span: token.span,
                        expected: vec!["datum".to_string()],
                        actual: Some("end of input".to_string()),
                        suggestion: Some("Add a datum after the quote marker.".to_string()),
                    }));
                }
                let quoted = self.parse_datum()?;
                let span = SourceSpan::new(token.span.start, quoted.span.end);
                Ok(SpannedDatum {
                    datum: Datum::Quote(Box::new(quoted)),
                    span,
                })
            }
            TokenKind::Atom(text) => Ok(SpannedDatum {
                datum: parse_atom_datum(&text),
                span: token.span,
            }),
            TokenKind::String(value) => Ok(SpannedDatum {
                datum: Datum::String(value),
                span: token.span,
            }),
        }
    }

    fn parse_collection(
        &mut self,
        delimiter: Delimiter,
        open_span: SourceSpan,
    ) -> ReaderResult<SpannedDatum> {
        let mut items = Vec::new();

        loop {
            if self.cursor >= self.tokens.len() {
                return Err(Diagnostic::new(DiagnosticSpec {
                    phase: DiagnosticPhase::Reader,
                    code: "ANVIL_READER_UNCLOSED_DELIMITER",
                    source: self.source,
                    message: format!(
                        "{} is missing closing delimiter {}",
                        delimiter.open(),
                        delimiter.close()
                    ),
                    span: SourceSpan::new(open_span.start, self.eof),
                    expected: vec![delimiter.close().to_string()],
                    actual: Some("end of input".to_string()),
                    suggestion: Some(format!("Add a matching {}.", delimiter.close())),
                }));
            }

            if let TokenKind::Close(close_delimiter) = self.tokens[self.cursor].kind {
                let close = self.next_token()?;
                if close_delimiter != delimiter {
                    return Err(Diagnostic::new(DiagnosticSpec {
                        phase: DiagnosticPhase::Reader,
                        code: "ANVIL_READER_MISMATCHED_DELIMITER",
                        source: self.source,
                        message: format!(
                            "expected closing delimiter {}, found {}",
                            delimiter.close(),
                            close_delimiter.close()
                        ),
                        span: close.span,
                        expected: vec![delimiter.close().to_string()],
                        actual: Some(close_delimiter.close().to_string()),
                        suggestion: Some(format!("Replace it with {}.", delimiter.close())),
                    }));
                }

                let span = SourceSpan::new(open_span.start, close.span.end);
                return self.finish_collection(delimiter, items, span);
            }

            items.push(self.parse_datum()?);
        }
    }

    fn finish_collection(
        &self,
        delimiter: Delimiter,
        items: Vec<SpannedDatum>,
        span: SourceSpan,
    ) -> ReaderResult<SpannedDatum> {
        let datum = match delimiter {
            Delimiter::List => Datum::List(items),
            Delimiter::Vector => Datum::Vector(items),
            Delimiter::Map => {
                if !items.len().is_multiple_of(2) {
                    return Err(Diagnostic::new(DiagnosticSpec {
                        phase: DiagnosticPhase::Reader,
                        code: "ANVIL_READER_ODD_MAP",
                        source: self.source,
                        message: "map literal requires an even number of forms".to_string(),
                        span,
                        expected: vec!["key value pairs".to_string()],
                        actual: Some(format!("{} form(s)", items.len())),
                        suggestion: Some(
                            "Add a value for the final key or remove the final key.".to_string(),
                        ),
                    }));
                }

                let pairs = items
                    .chunks_exact(2)
                    .map(|pair| (pair[0].clone(), pair[1].clone()))
                    .collect();
                Datum::Map(pairs)
            }
        };

        Ok(SpannedDatum { datum, span })
    }

    fn next_token(&mut self) -> ReaderResult<Token> {
        let token = self.tokens.get(self.cursor).cloned().ok_or_else(|| {
            Diagnostic::new(DiagnosticSpec {
                phase: DiagnosticPhase::Reader,
                code: "ANVIL_READER_UNEXPECTED_EOF",
                source: self.source,
                message: "expected a datum, found end of input".to_string(),
                span: SourceSpan::point(self.eof),
                expected: vec!["datum".to_string()],
                actual: Some("end of input".to_string()),
                suggestion: None,
            })
        })?;
        self.cursor += 1;
        Ok(token)
    }
}

fn parse_atom_datum(text: &str) -> Datum {
    match text {
        "nil" => Datum::Nil,
        "true" => Datum::Bool(true),
        "false" => Datum::Bool(false),
        _ if text.starts_with(':') && text.len() > 1 => Datum::Keyword(text[1..].to_string()),
        _ if starts_like_number(text) && looks_float(text) => text
            .parse::<f64>()
            .map(Datum::Float64)
            .unwrap_or_else(|_| Datum::Symbol(text.to_string())),
        _ if starts_like_number(text) => text
            .parse::<i64>()
            .map(Datum::Integer)
            .unwrap_or_else(|_| Datum::Symbol(text.to_string())),
        _ => Datum::Symbol(text.to_string()),
    }
}

fn starts_like_number(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if first.is_ascii_digit() => true,
        Some('+' | '-') => chars.next().is_some_and(|ch| ch.is_ascii_digit()),
        _ => false,
    }
}

fn looks_float(text: &str) -> bool {
    text.contains('.') || text.contains('e') || text.contains('E')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_list_vector_map_and_quote() {
        let datums = read_source("(define answer [1 2 {:ok true}] 'answer)").unwrap();

        assert_eq!(datums.len(), 1);
        assert_eq!(
            format_datums(&datums),
            "(define answer [1 2 {:ok true}] 'answer)"
        );
    }

    #[test]
    fn round_trips_multiple_top_level_datums() {
        let source = "(define answer 42)\n[answer {:ok true}]";
        let datums = read_source(source).unwrap();
        let formatted = format_datums(&datums);
        let reparsed = read_source(&formatted).unwrap();

        assert_eq!(datums.len(), 2);
        assert_eq!(formatted, source);
        assert_eq!(format_datums(&reparsed), source);
    }

    #[test]
    fn ignores_comments_and_commas_as_whitespace() {
        let datums = read_source("; hello\n[1, 2, 3]").unwrap();

        assert_eq!(datums.len(), 1);
        assert_eq!(datums[0].to_string(), "[1 2 3]");
    }

    #[test]
    fn reports_unclosed_delimiter_with_span() {
        let diagnostic = read_source("(define answer 42").unwrap_err();

        assert_eq!(diagnostic.code, "ANVIL_READER_UNCLOSED_DELIMITER");
        assert_eq!(diagnostic.span.start.line, 1);
        assert_eq!(diagnostic.span.start.column, 1);
    }
}
