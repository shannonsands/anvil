use std::{collections::HashSet, fmt};

use serde::Serialize;

use crate::{
    diagnostic::{Diagnostic, DiagnosticPhase, DiagnosticSpec},
    reader::{Datum, SpannedDatum, read_source_text},
    source::{SourceSpan, SourceText},
};

pub type AstDiagnostic = Diagnostic;
pub type AstResult<T> = Result<T, Box<AstDiagnostic>>;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpannedAst {
    #[serde(flatten)]
    pub kind: AstKind,
    pub span: SourceSpan,
}

impl fmt::Display for SpannedAst {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AstKind {
    Literal {
        value: AstLiteral,
    },
    Symbol {
        name: String,
    },
    Quote {
        datum: Box<SpannedDatum>,
    },
    Define {
        name: String,
        value: Box<SpannedAst>,
    },
    If {
        condition: Box<SpannedAst>,
        then_branch: Box<SpannedAst>,
        else_branch: Box<SpannedAst>,
    },
    Do {
        expressions: Vec<SpannedAst>,
    },
    Fn {
        params: Vec<String>,
        body: Vec<SpannedAst>,
    },
    Call {
        callee: Box<SpannedAst>,
        args: Vec<SpannedAst>,
    },
    Vector {
        items: Vec<SpannedAst>,
    },
    Map {
        entries: Vec<AstMapEntry>,
    },
}

impl fmt::Display for AstKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal { value } => value.fmt(f),
            Self::Symbol { name } => f.write_str(name),
            Self::Quote { datum } => write!(f, "'{datum}"),
            Self::Define { name, value } => write!(f, "(define {name} {value})"),
            Self::If {
                condition,
                then_branch,
                else_branch,
            } => write!(f, "(if {condition} {then_branch} {else_branch})"),
            Self::Do { expressions } => write_sequence(f, "do", expressions),
            Self::Fn { params, body } => {
                write!(f, "(fn [")?;
                for (index, param) in params.iter().enumerate() {
                    if index > 0 {
                        f.write_str(" ")?;
                    }
                    f.write_str(param)?;
                }
                f.write_str("]")?;
                for expression in body {
                    write!(f, " {expression}")?;
                }
                f.write_str(")")
            }
            Self::Call { callee, args } => {
                write!(f, "({callee}")?;
                for arg in args {
                    write!(f, " {arg}")?;
                }
                f.write_str(")")
            }
            Self::Vector { items } => {
                f.write_str("[")?;
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        f.write_str(" ")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str("]")
            }
            Self::Map { entries } => {
                f.write_str("{")?;
                for (index, entry) in entries.iter().enumerate() {
                    if index > 0 {
                        f.write_str(" ")?;
                    }
                    write!(f, "{} {}", entry.key, entry.value)?;
                }
                f.write_str("}")
            }
        }
    }
}

fn write_sequence(
    f: &mut fmt::Formatter<'_>,
    form_name: &str,
    expressions: &[SpannedAst],
) -> fmt::Result {
    write!(f, "({form_name}")?;
    for expression in expressions {
        write!(f, " {expression}")?;
    }
    f.write_str(")")
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AstLiteral {
    Nil,
    Bool(bool),
    Integer(i64),
    Float64(f64),
    String(String),
    Keyword(String),
}

impl fmt::Display for AstLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nil => f.write_str("nil"),
            Self::Bool(value) => write!(f, "{value}"),
            Self::Integer(value) => write!(f, "{value}"),
            Self::Float64(value) => write!(f, "{value}"),
            Self::String(value) => write!(f, "\"{}\"", escape_string(value)),
            Self::Keyword(value) => write!(f, ":{value}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AstMapEntry {
    pub key: SpannedAst,
    pub value: SpannedAst,
}

pub fn lower_source(source: &str) -> AstResult<Vec<SpannedAst>> {
    lower_source_text(&SourceText::repl(source))
}

pub fn lower_source_text(source: &SourceText) -> AstResult<Vec<SpannedAst>> {
    let datums = read_source_text(source)?;
    lower_datums(source, &datums)
}

pub fn lower_datums(source: &SourceText, datums: &[SpannedDatum]) -> AstResult<Vec<SpannedAst>> {
    datums
        .iter()
        .map(|datum| lower_datum(source, datum))
        .collect()
}

pub fn format_ast(expressions: &[SpannedAst]) -> String {
    expressions
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn lower_datum(source: &SourceText, datum: &SpannedDatum) -> AstResult<SpannedAst> {
    let kind = match &datum.datum {
        Datum::Nil => AstKind::Literal {
            value: AstLiteral::Nil,
        },
        Datum::Bool(value) => AstKind::Literal {
            value: AstLiteral::Bool(*value),
        },
        Datum::Integer(value) => AstKind::Literal {
            value: AstLiteral::Integer(*value),
        },
        Datum::Float64(value) => AstKind::Literal {
            value: AstLiteral::Float64(*value),
        },
        Datum::String(value) => AstKind::Literal {
            value: AstLiteral::String(value.clone()),
        },
        Datum::Keyword(value) => AstKind::Literal {
            value: AstLiteral::Keyword(value.clone()),
        },
        Datum::Symbol(name) => AstKind::Symbol { name: name.clone() },
        Datum::Quote(quoted) => AstKind::Quote {
            datum: Box::new((**quoted).clone()),
        },
        Datum::List(items) => lower_list(source, datum.span, items)?,
        Datum::Vector(items) => AstKind::Vector {
            items: lower_datums(source, items)?,
        },
        Datum::Map(pairs) => AstKind::Map {
            entries: pairs
                .iter()
                .map(|(key, value)| {
                    Ok(AstMapEntry {
                        key: lower_datum(source, key)?,
                        value: lower_datum(source, value)?,
                    })
                })
                .collect::<AstResult<_>>()?,
        },
    };

    Ok(SpannedAst {
        kind,
        span: datum.span,
    })
}

fn lower_list(source: &SourceText, span: SourceSpan, items: &[SpannedDatum]) -> AstResult<AstKind> {
    let Some(first) = items.first() else {
        return Err(syntax_error(SyntaxDiagnosticSpec {
            source,
            code: "ANVIL_SYNTAX_EMPTY_LIST",
            message: "empty list cannot be lowered as an expression".to_string(),
            span,
            expected: vec!["callee or special form".to_string()],
            actual: Some("empty list".to_string()),
            suggestion: Some("Use nil for absence, or add a callee before operands.".to_string()),
        }));
    };

    match &first.datum {
        Datum::Symbol(name) if name == "define" => lower_define(source, items),
        Datum::Symbol(name) if name == "if" => lower_if(source, items),
        Datum::Symbol(name) if name == "do" => lower_do(source, items),
        Datum::Symbol(name) if matches!(name.as_str(), "fn" | "lambda") => lower_fn(source, items),
        _ => lower_call(source, items),
    }
}

fn lower_define(source: &SourceText, items: &[SpannedDatum]) -> AstResult<AstKind> {
    require_exact_operands(source, "define", items, 2, "name and value")?;
    let name = expect_symbol(source, &items[1], "definition name")?;
    let value = lower_datum(source, &items[2])?;

    Ok(AstKind::Define {
        name,
        value: Box::new(value),
    })
}

fn lower_if(source: &SourceText, items: &[SpannedDatum]) -> AstResult<AstKind> {
    require_exact_operands(
        source,
        "if",
        items,
        3,
        "condition, then branch, and else branch",
    )?;

    Ok(AstKind::If {
        condition: Box::new(lower_datum(source, &items[1])?),
        then_branch: Box::new(lower_datum(source, &items[2])?),
        else_branch: Box::new(lower_datum(source, &items[3])?),
    })
}

fn lower_do(source: &SourceText, items: &[SpannedDatum]) -> AstResult<AstKind> {
    Ok(AstKind::Do {
        expressions: lower_datums(source, &items[1..])?,
    })
}

fn lower_fn(source: &SourceText, items: &[SpannedDatum]) -> AstResult<AstKind> {
    require_at_least_operands(source, "fn", items, 2, "parameter vector and body")?;
    let params = expect_param_vector(source, &items[1])?;
    let body = lower_datums(source, &items[2..])?;

    Ok(AstKind::Fn { params, body })
}

fn lower_call(source: &SourceText, items: &[SpannedDatum]) -> AstResult<AstKind> {
    let callee = lower_datum(source, &items[0])?;
    let args = lower_datums(source, &items[1..])?;

    Ok(AstKind::Call {
        callee: Box::new(callee),
        args,
    })
}

fn require_exact_operands(
    source: &SourceText,
    form_name: &str,
    items: &[SpannedDatum],
    expected: usize,
    expected_description: &str,
) -> AstResult<()> {
    let actual = items.len().saturating_sub(1);
    if actual == expected {
        return Ok(());
    }

    Err(syntax_error(SyntaxDiagnosticSpec {
        source,
        code: "ANVIL_SYNTAX_ARITY",
        message: format!("{form_name} expects {expected_description}"),
        span: items[0].span,
        expected: vec![format!("{expected} operand(s): {expected_description}")],
        actual: Some(format!("{actual} operand(s)")),
        suggestion: Some(format!(
            "Rewrite the {form_name} form with the required operands."
        )),
    }))
}

fn require_at_least_operands(
    source: &SourceText,
    form_name: &str,
    items: &[SpannedDatum],
    expected: usize,
    expected_description: &str,
) -> AstResult<()> {
    let actual = items.len().saturating_sub(1);
    if actual >= expected {
        return Ok(());
    }

    Err(syntax_error(SyntaxDiagnosticSpec {
        source,
        code: "ANVIL_SYNTAX_ARITY",
        message: format!("{form_name} expects at least {expected_description}"),
        span: items[0].span,
        expected: vec![format!(
            "at least {expected} operand(s): {expected_description}"
        )],
        actual: Some(format!("{actual} operand(s)")),
        suggestion: Some(format!("Add the missing {form_name} operands.")),
    }))
}

fn expect_symbol(
    source: &SourceText,
    datum: &SpannedDatum,
    expected_description: &str,
) -> AstResult<String> {
    match &datum.datum {
        Datum::Symbol(name) => Ok(name.clone()),
        other => Err(syntax_error(SyntaxDiagnosticSpec {
            source,
            code: "ANVIL_SYNTAX_EXPECTED_SYMBOL",
            message: format!("expected {expected_description} to be a symbol"),
            span: datum.span,
            expected: vec!["symbol".to_string()],
            actual: Some(datum_description(other)),
            suggestion: Some("Use a bare symbol name here.".to_string()),
        })),
    }
}

fn expect_param_vector(source: &SourceText, datum: &SpannedDatum) -> AstResult<Vec<String>> {
    let Datum::Vector(params) = &datum.datum else {
        return Err(syntax_error(SyntaxDiagnosticSpec {
            source,
            code: "ANVIL_SYNTAX_EXPECTED_VECTOR",
            message: "fn expects its parameters in a vector".to_string(),
            span: datum.span,
            expected: vec!["parameter vector".to_string()],
            actual: Some(datum_description(&datum.datum)),
            suggestion: Some("Wrap parameters in square brackets.".to_string()),
        }));
    };

    let mut seen = HashSet::new();
    let mut names = Vec::new();
    for param in params {
        let name = expect_symbol(source, param, "function parameter")?;
        if !seen.insert(name.clone()) {
            return Err(syntax_error(SyntaxDiagnosticSpec {
                source,
                code: "ANVIL_SYNTAX_DUPLICATE_BINDING",
                message: format!("duplicate function parameter {name}"),
                span: param.span,
                expected: vec!["unique parameter names".to_string()],
                actual: Some(name),
                suggestion: Some("Rename one of the duplicate parameters.".to_string()),
            }));
        }
        names.push(name);
    }

    Ok(names)
}

struct SyntaxDiagnosticSpec<'source> {
    source: &'source SourceText,
    code: &'static str,
    message: String,
    span: SourceSpan,
    expected: Vec<String>,
    actual: Option<String>,
    suggestion: Option<String>,
}

fn syntax_error(spec: SyntaxDiagnosticSpec<'_>) -> Box<AstDiagnostic> {
    Diagnostic::new(DiagnosticSpec {
        code: spec.code,
        phase: DiagnosticPhase::Syntax,
        source: spec.source,
        message: spec.message,
        span: spec.span,
        expected: spec.expected,
        actual: spec.actual,
        suggestion: spec.suggestion,
    })
}

fn datum_description(datum: &Datum) -> String {
    match datum {
        Datum::Nil => "nil".to_string(),
        Datum::Bool(_) => "boolean literal".to_string(),
        Datum::Integer(_) => "integer literal".to_string(),
        Datum::Float64(_) => "float literal".to_string(),
        Datum::String(_) => "string literal".to_string(),
        Datum::Symbol(_) => "symbol".to_string(),
        Datum::Keyword(_) => "keyword literal".to_string(),
        Datum::List(_) => "list".to_string(),
        Datum::Vector(_) => "vector".to_string(),
        Datum::Map(_) => "map".to_string(),
        Datum::Quote(_) => "quoted datum".to_string(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowers_define_form() {
        let ast = lower_source("(define answer (+ 40 2))").unwrap();

        assert_eq!(format_ast(&ast), "(define answer (+ 40 2))");
        assert!(matches!(ast[0].kind, AstKind::Define { .. }));
    }

    #[test]
    fn lowers_fn_form() {
        let ast = lower_source("(fn [x y] (+ x y))").unwrap();

        assert_eq!(format_ast(&ast), "(fn [x y] (+ x y))");
        assert!(matches!(ast[0].kind, AstKind::Fn { .. }));
    }

    #[test]
    fn rejects_bad_define_name() {
        let diagnostic = lower_source("(define 42 true)").unwrap_err();

        assert_eq!(diagnostic.code, "ANVIL_SYNTAX_EXPECTED_SYMBOL");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Syntax);
        assert_eq!(diagnostic.primary_span.start.column, 9);
    }

    #[test]
    fn rejects_duplicate_params() {
        let diagnostic = lower_source("(fn [x x] x)").unwrap_err();

        assert_eq!(diagnostic.code, "ANVIL_SYNTAX_DUPLICATE_BINDING");
        assert_eq!(diagnostic.primary_span.start.column, 8);
    }
}
