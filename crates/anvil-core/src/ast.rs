use std::{collections::HashSet, fmt};

use serde::Serialize;

use crate::{
    diagnostic::{Diagnostic, DiagnosticPhase, DiagnosticSpec},
    module::{ModuleResolution, ModuleResolver},
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
    Let {
        form: LetForm,
        bindings: Vec<LetBinding>,
        body: Vec<SpannedAst>,
    },
    Fn {
        params: Vec<String>,
        body: Vec<SpannedAst>,
    },
    Require {
        imports: Vec<RequireImport>,
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
            Self::Let {
                form,
                bindings,
                body,
            } => write_let(f, *form, bindings, body),
            Self::Fn { params, body } => write_fn(f, params, body),
            Self::Require { imports } => write_sequence(f, "require", imports),
            Self::Call { callee, args } => write_call(f, callee, args),
            Self::Vector { items } => write_enclosed_items(f, "[", "]", items),
            Self::Map { entries } => write_map(f, entries),
        }
    }
}

fn write_let(
    f: &mut fmt::Formatter<'_>,
    form: LetForm,
    bindings: &[LetBinding],
    body: &[SpannedAst],
) -> fmt::Result {
    write!(f, "({form} [")?;
    let mut separator = "";
    for binding in bindings {
        f.write_str(separator)?;
        write!(f, "{} {}", binding.name, binding.value)?;
        separator = " ";
    }
    f.write_str("]")?;
    for expression in body {
        write!(f, " {expression}")?;
    }
    f.write_str(")")
}

fn write_fn(f: &mut fmt::Formatter<'_>, params: &[String], body: &[SpannedAst]) -> fmt::Result {
    f.write_str("(fn ")?;
    write_enclosed_items(f, "[", "]", params)?;
    for expression in body {
        write!(f, " {expression}")?;
    }
    f.write_str(")")
}

fn write_call(f: &mut fmt::Formatter<'_>, callee: &SpannedAst, args: &[SpannedAst]) -> fmt::Result {
    write!(f, "({callee}")?;
    for arg in args {
        write!(f, " {arg}")?;
    }
    f.write_str(")")
}

fn write_sequence(
    f: &mut fmt::Formatter<'_>,
    form_name: &str,
    expressions: &[impl fmt::Display],
) -> fmt::Result {
    write!(f, "({form_name}")?;
    for expression in expressions {
        write!(f, " {expression}")?;
    }
    f.write_str(")")
}

fn write_enclosed_items(
    f: &mut fmt::Formatter<'_>,
    open: &str,
    close: &str,
    items: &[impl fmt::Display],
) -> fmt::Result {
    f.write_str(open)?;
    let mut separator = "";
    for item in items {
        f.write_str(separator)?;
        write!(f, "{item}")?;
        separator = " ";
    }
    f.write_str(close)
}

fn write_map(f: &mut fmt::Formatter<'_>, entries: &[AstMapEntry]) -> fmt::Result {
    f.write_str("{")?;
    let mut separator = "";
    for entry in entries {
        f.write_str(separator)?;
        write!(f, "{} {}", entry.key, entry.value)?;
        separator = " ";
    }
    f.write_str("}")
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LetForm {
    Let,
    LetStar,
}

impl LetForm {
    fn source_name(self) -> &'static str {
        match self {
            Self::Let => "let",
            Self::LetStar => "let*",
        }
    }
}

impl fmt::Display for LetForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.source_name())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LetBinding {
    pub name: String,
    pub value: SpannedAst,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RequireImport {
    pub module: String,
    pub alias: Option<String>,
    pub resolution: Option<ModuleResolution>,
    pub span: SourceSpan,
}

impl fmt::Display for RequireImport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.alias {
            Some(alias) => write!(f, "[{} :as {alias}]", self.module),
            None => f.write_str(&self.module),
        }
    }
}

pub fn lower_source(source: &str) -> AstResult<Vec<SpannedAst>> {
    lower_source_text(&SourceText::repl(source))
}

pub fn lower_source_text(source: &SourceText) -> AstResult<Vec<SpannedAst>> {
    let datums = read_source_text(source)?;
    lower_datums(source, &datums)
}

pub fn lower_source_with_resolver(
    source: &str,
    resolver: &ModuleResolver,
) -> AstResult<Vec<SpannedAst>> {
    lower_source_text_with_resolver(&SourceText::repl(source), resolver)
}

pub fn lower_source_text_with_resolver(
    source: &SourceText,
    resolver: &ModuleResolver,
) -> AstResult<Vec<SpannedAst>> {
    let datums = read_source_text(source)?;
    lower_datums_with_resolver(source, &datums, resolver)
}

pub fn lower_datums(source: &SourceText, datums: &[SpannedDatum]) -> AstResult<Vec<SpannedAst>> {
    lower_datums_in_context(&LoweringContext::new(source), datums)
}

pub fn lower_datums_with_resolver(
    source: &SourceText,
    datums: &[SpannedDatum],
    resolver: &ModuleResolver,
) -> AstResult<Vec<SpannedAst>> {
    lower_datums_in_context(&LoweringContext::with_resolver(source, resolver), datums)
}

fn lower_datums_in_context(
    context: &LoweringContext<'_>,
    datums: &[SpannedDatum],
) -> AstResult<Vec<SpannedAst>> {
    datums
        .iter()
        .map(|datum| lower_datum(context, datum))
        .collect()
}

pub fn format_ast(expressions: &[SpannedAst]) -> String {
    expressions
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Clone, Copy)]
struct LoweringContext<'source> {
    source: &'source SourceText,
    module_resolver: Option<&'source ModuleResolver>,
}

impl<'source> LoweringContext<'source> {
    fn new(source: &'source SourceText) -> Self {
        Self {
            source,
            module_resolver: None,
        }
    }

    fn with_resolver(source: &'source SourceText, resolver: &'source ModuleResolver) -> Self {
        Self {
            source,
            module_resolver: Some(resolver),
        }
    }
}

fn lower_datum(context: &LoweringContext<'_>, datum: &SpannedDatum) -> AstResult<SpannedAst> {
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
        Datum::List(items) => lower_list(context, datum.span, items)?,
        Datum::Vector(items) => AstKind::Vector {
            items: lower_datums_in_context(context, items)?,
        },
        Datum::Map(pairs) => AstKind::Map {
            entries: pairs
                .iter()
                .map(|(key, value)| {
                    Ok(AstMapEntry {
                        key: lower_datum(context, key)?,
                        value: lower_datum(context, value)?,
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

fn lower_list(
    context: &LoweringContext<'_>,
    span: SourceSpan,
    items: &[SpannedDatum],
) -> AstResult<AstKind> {
    let Some(first) = items.first() else {
        return Err(syntax_error(SyntaxDiagnosticSpec {
            source: context.source,
            code: "ANVIL_SYNTAX_EMPTY_LIST",
            message: "empty list cannot be lowered as an expression".to_string(),
            span,
            expected: vec!["callee or special form".to_string()],
            actual: Some("empty list".to_string()),
            suggestion: Some("Use nil for absence, or add a callee before operands.".to_string()),
        }));
    };

    match &first.datum {
        Datum::Symbol(name) if name == "define" => lower_define(context, items),
        Datum::Symbol(name) if name == "if" => lower_if(context, items),
        Datum::Symbol(name) if name == "do" => lower_do(context, items),
        Datum::Symbol(name) if name == "let" => lower_let(context, items, LetForm::Let),
        Datum::Symbol(name) if name == "let*" => lower_let(context, items, LetForm::LetStar),
        Datum::Symbol(name) if matches!(name.as_str(), "fn" | "lambda") => lower_fn(context, items),
        Datum::Symbol(name) if name == "require" => lower_require(context, items),
        _ => lower_call(context, items),
    }
}

fn lower_define(context: &LoweringContext<'_>, items: &[SpannedDatum]) -> AstResult<AstKind> {
    require_exact_operands(context.source, "define", items, 2, "name and value")?;
    let name = expect_symbol(context.source, &items[1], "definition name")?;
    let value = lower_datum(context, &items[2])?;

    Ok(AstKind::Define {
        name,
        value: Box::new(value),
    })
}

fn lower_if(context: &LoweringContext<'_>, items: &[SpannedDatum]) -> AstResult<AstKind> {
    require_exact_operands(
        context.source,
        "if",
        items,
        3,
        "condition, then branch, and else branch",
    )?;

    Ok(AstKind::If {
        condition: Box::new(lower_datum(context, &items[1])?),
        then_branch: Box::new(lower_datum(context, &items[2])?),
        else_branch: Box::new(lower_datum(context, &items[3])?),
    })
}

fn lower_do(context: &LoweringContext<'_>, items: &[SpannedDatum]) -> AstResult<AstKind> {
    Ok(AstKind::Do {
        expressions: lower_datums_in_context(context, &items[1..])?,
    })
}

fn lower_let(
    context: &LoweringContext<'_>,
    items: &[SpannedDatum],
    form: LetForm,
) -> AstResult<AstKind> {
    require_at_least_operands(
        context.source,
        form.source_name(),
        items,
        2,
        "binding vector and body",
    )?;
    let bindings = expect_let_bindings(context, &items[1])?;
    let body = lower_datums_in_context(context, &items[2..])?;

    Ok(AstKind::Let {
        form,
        bindings,
        body,
    })
}

fn lower_fn(context: &LoweringContext<'_>, items: &[SpannedDatum]) -> AstResult<AstKind> {
    require_at_least_operands(context.source, "fn", items, 2, "parameter vector and body")?;
    let params = expect_param_vector(context.source, &items[1])?;
    let body = lower_datums_in_context(context, &items[2..])?;

    Ok(AstKind::Fn { params, body })
}

fn lower_require(context: &LoweringContext<'_>, items: &[SpannedDatum]) -> AstResult<AstKind> {
    require_at_least_operands(context.source, "require", items, 1, "module import")?;

    Ok(AstKind::Require {
        imports: items[1..]
            .iter()
            .map(|item| lower_require_import(context, item))
            .collect::<AstResult<_>>()?,
    })
}

fn lower_require_import(
    context: &LoweringContext<'_>,
    datum: &SpannedDatum,
) -> AstResult<RequireImport> {
    let (module, alias, span) = match &datum.datum {
        Datum::Symbol(module) => (module.clone(), None, datum.span),
        Datum::Vector(items) => lower_require_vector(context.source, datum.span, items)?,
        other => {
            return Err(syntax_error(SyntaxDiagnosticSpec {
                source: context.source,
                code: "ANVIL_SYNTAX_REQUIRE_IMPORT",
                message: "require import must be a module symbol or import vector".to_string(),
                span: datum.span,
                expected: vec![
                    "module symbol".to_string(),
                    "[module :as alias]".to_string(),
                ],
                actual: Some(datum_description(other)),
                suggestion: Some("Use a bare module symbol or [module :as alias].".to_string()),
            }));
        }
    };
    let resolution = match context.module_resolver {
        Some(resolver) => Some(resolver.resolve_in_source(&module, context.source, span)?),
        None => None,
    };

    Ok(RequireImport {
        module,
        alias,
        resolution,
        span,
    })
}

fn lower_require_vector(
    source: &SourceText,
    span: SourceSpan,
    items: &[SpannedDatum],
) -> AstResult<(String, Option<String>, SourceSpan)> {
    if items.len() != 3 {
        return Err(syntax_error(SyntaxDiagnosticSpec {
            source,
            code: "ANVIL_SYNTAX_REQUIRE_IMPORT",
            message: "require import vector expects module, :as, and alias".to_string(),
            span,
            expected: vec!["[module :as alias]".to_string()],
            actual: Some(format!("{} form(s)", items.len())),
            suggestion: Some("Use [module :as alias].".to_string()),
        }));
    }

    let module = expect_symbol(source, &items[0], "require module")?;
    let Datum::Keyword(marker) = &items[1].datum else {
        return Err(syntax_error(SyntaxDiagnosticSpec {
            source,
            code: "ANVIL_SYNTAX_REQUIRE_IMPORT",
            message: "require import vector expects :as before alias".to_string(),
            span: items[1].span,
            expected: vec![":as".to_string()],
            actual: Some(datum_description(&items[1].datum)),
            suggestion: Some("Use [module :as alias].".to_string()),
        }));
    };
    if marker != "as" {
        return Err(syntax_error(SyntaxDiagnosticSpec {
            source,
            code: "ANVIL_SYNTAX_REQUIRE_IMPORT",
            message: "require import vector expects :as before alias".to_string(),
            span: items[1].span,
            expected: vec![":as".to_string()],
            actual: Some(format!(":{marker}")),
            suggestion: Some("Use [module :as alias].".to_string()),
        }));
    }
    let alias = expect_symbol(source, &items[2], "require alias")?;

    Ok((module, Some(alias), items[0].span))
}

fn lower_call(context: &LoweringContext<'_>, items: &[SpannedDatum]) -> AstResult<AstKind> {
    let callee = lower_datum(context, &items[0])?;
    let args = lower_datums_in_context(context, &items[1..])?;

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

fn expect_let_bindings(
    context: &LoweringContext<'_>,
    datum: &SpannedDatum,
) -> AstResult<Vec<LetBinding>> {
    let Datum::Vector(items) = &datum.datum else {
        return Err(syntax_error(SyntaxDiagnosticSpec {
            source: context.source,
            code: "ANVIL_SYNTAX_EXPECTED_VECTOR",
            message: "let expects bindings in a vector".to_string(),
            span: datum.span,
            expected: vec!["binding vector".to_string()],
            actual: Some(datum_description(&datum.datum)),
            suggestion: Some("Wrap lexical bindings in square brackets.".to_string()),
        }));
    };
    if items.len() % 2 != 0 {
        return Err(syntax_error(SyntaxDiagnosticSpec {
            source: context.source,
            code: "ANVIL_SYNTAX_BINDING_VECTOR",
            message: "let binding vector must contain name/value pairs".to_string(),
            span: datum.span,
            expected: vec!["even number of binding forms".to_string()],
            actual: Some(format!("{} form(s)", items.len())),
            suggestion: Some("Use [name value] pairs in the binding vector.".to_string()),
        }));
    }

    let mut seen = HashSet::new();
    let mut bindings = Vec::with_capacity(items.len() / 2);
    for pair in items.chunks_exact(2) {
        let name = expect_symbol(context.source, &pair[0], "lexical binding name")?;
        if !seen.insert(name.clone()) {
            return Err(syntax_error(SyntaxDiagnosticSpec {
                source: context.source,
                code: "ANVIL_SYNTAX_DUPLICATE_BINDING",
                message: format!("duplicate lexical binding {name}"),
                span: pair[0].span,
                expected: vec!["unique lexical binding names".to_string()],
                actual: Some(name),
                suggestion: Some("Rename one of the duplicate lexical bindings.".to_string()),
            }));
        }
        bindings.push(LetBinding {
            name,
            value: lower_datum(context, &pair[1])?,
            span: pair[0].span,
        });
    }

    Ok(bindings)
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
    fn lowers_if_form() {
        let ast = lower_source("(if ready? :yes :no)").unwrap();

        assert_eq!(format_ast(&ast), "(if ready? :yes :no)");
        assert!(matches!(ast[0].kind, AstKind::If { .. }));
    }

    #[test]
    fn lowers_let_forms() {
        let ast = lower_source("(let [x 1 y (+ x 1)] y)").unwrap();

        assert_eq!(format_ast(&ast), "(let [x 1 y (+ x 1)] y)");
        assert!(matches!(ast[0].kind, AstKind::Let { .. }));

        let ast = lower_source("(let* [x 1] x)").unwrap();

        assert_eq!(format_ast(&ast), "(let* [x 1] x)");
    }

    #[test]
    fn rejects_bad_if_arity() {
        let diagnostic = lower_source("(if true 1)").unwrap_err();

        assert_eq!(diagnostic.code, "ANVIL_SYNTAX_ARITY");
        assert_eq!(diagnostic.primary_span.start.column, 2);
    }

    #[test]
    fn formats_composite_ast_forms() {
        let ast = lower_source(
            r#"
            (do
              "line\n"
              '[1 2]
              {:answer 42}
              [true nil])
            "#,
        )
        .unwrap();

        assert_eq!(
            format_ast(&ast),
            r#"(do "line\n" '[1 2] {:answer 42} [true nil])"#
        );
    }

    #[test]
    fn lowers_require_form() {
        let ast = lower_source("(require [planner.search :as search])").unwrap();

        assert_eq!(format_ast(&ast), "(require [planner.search :as search])");
        assert!(matches!(ast[0].kind, AstKind::Require { .. }));
    }

    #[test]
    fn resolves_require_imports_when_resolver_is_provided() {
        let resolver = ModuleResolver::new().with_module(
            crate::ModuleRootKind::Package,
            "planner-tools",
            "planner.search",
            "src/planner/search.anv",
        );
        let ast = lower_source_with_resolver("(require planner.search)", &resolver).unwrap();
        let AstKind::Require { imports } = &ast[0].kind else {
            panic!("expected require AST");
        };

        assert_eq!(
            imports[0]
                .resolution
                .as_ref()
                .map(|resolution| resolution.root_name.as_str()),
            Some("planner-tools")
        );
    }

    #[test]
    fn reports_module_diagnostics_at_require_span() {
        let resolver = ModuleResolver::new();
        let diagnostic = lower_source_with_resolver("(require missing.module)", &resolver)
            .expect_err("missing module diagnostic");

        assert_eq!(diagnostic.code, "ANVIL_MODULE_NOT_FOUND");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Module);
        assert_eq!(diagnostic.primary_span.start.column, 10);
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

    #[test]
    fn rejects_bad_let_binding_vectors() {
        let diagnostic = lower_source("(let [x] x)").unwrap_err();

        assert_eq!(diagnostic.code, "ANVIL_SYNTAX_BINDING_VECTOR");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Syntax);

        let diagnostic = lower_source("(let [x 1 x 2] x)").unwrap_err();

        assert_eq!(diagnostic.code, "ANVIL_SYNTAX_DUPLICATE_BINDING");
        assert_eq!(diagnostic.primary_span.start.column, 11);
    }
}
