use std::fmt;

use serde::Serialize;

use crate::{
    ast::{AstKind, AstLiteral, SpannedAst, lower_source_text},
    diagnostic::{Diagnostic, DiagnosticPhase, DiagnosticSpec},
    source::{SourceLocation, SourceSpan, SourceText},
};

pub type VmDiagnostic = Diagnostic;
pub type VmResult<T> = Result<T, Box<VmDiagnostic>>;

const BYTECODE_VERSION: u16 = 1;
const RESULT_REGISTER: usize = 0;
const DEFAULT_INSTRUCTION_FUEL: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BytecodeProgram {
    pub version: u16,
    pub source_id: String,
    pub register_count: usize,
    pub constants: Vec<Value>,
    pub instructions: Vec<BytecodeInstruction>,
    #[serde(skip)]
    source: SourceText,
}

impl BytecodeProgram {
    pub fn source(&self) -> &SourceText {
        &self.source
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BytecodeInstruction {
    #[serde(flatten)]
    pub instruction: Instruction,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Instruction {
    LoadConstant {
        dst: usize,
        constant: usize,
    },
    MakeVector {
        dst: usize,
        items: Vec<usize>,
    },
    MakeMap {
        dst: usize,
        entries: Vec<MapRegisterEntry>,
    },
    JumpIfFalse {
        condition: usize,
        target: usize,
    },
    Jump {
        target: usize,
    },
    Return {
        src: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MapRegisterEntry {
    pub key: usize,
    pub value: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Value {
    Nil,
    Bool(bool),
    Integer(i64),
    Float64(f64),
    String(String),
    Keyword(String),
    Vector(Vec<Value>),
    Map(Vec<ValueMapEntry>),
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        !matches!(self, Self::Nil | Self::Bool(false))
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nil => f.write_str("nil"),
            Self::Bool(value) => write!(f, "{value}"),
            Self::Integer(value) => write!(f, "{value}"),
            Self::Float64(value) => write!(f, "{value}"),
            Self::String(value) => write!(f, "\"{}\"", escape_string(value)),
            Self::Keyword(value) => write!(f, ":{value}"),
            Self::Vector(items) => write_sequence(f, "[", "]", items),
            Self::Map(entries) => write_map(f, entries),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ValueMapEntry {
    pub key: Value,
    pub value: Value,
}

fn write_sequence(
    f: &mut fmt::Formatter<'_>,
    open: &str,
    close: &str,
    items: &[Value],
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

fn write_map(f: &mut fmt::Formatter<'_>, entries: &[ValueMapEntry]) -> fmt::Result {
    f.write_str("{")?;
    let mut separator = "";
    for entry in entries {
        f.write_str(separator)?;
        write!(f, "{} {}", entry.key, entry.value)?;
        separator = " ";
    }
    f.write_str("}")
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmBudget {
    pub instruction_fuel: Option<usize>,
}

impl VmBudget {
    pub fn with_instruction_fuel(instruction_fuel: usize) -> Self {
        Self {
            instruction_fuel: Some(instruction_fuel),
        }
    }

    pub fn unlimited() -> Self {
        Self {
            instruction_fuel: None,
        }
    }
}

impl Default for VmBudget {
    fn default() -> Self {
        Self {
            instruction_fuel: Some(DEFAULT_INSTRUCTION_FUEL),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VmOutput {
    pub value: Value,
    pub instructions_executed: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vm {
    budget: VmBudget,
}

impl Vm {
    pub fn new() -> Self {
        Self {
            budget: VmBudget::default(),
        }
    }

    pub fn with_budget(budget: VmBudget) -> Self {
        Self { budget }
    }

    pub fn run(&self, program: &BytecodeProgram) -> VmResult<VmOutput> {
        Interpreter::new(program, self.budget).run()
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

pub fn compile_source(source: &str) -> VmResult<BytecodeProgram> {
    compile_source_text(&SourceText::repl(source))
}

pub fn compile_source_text(source: &SourceText) -> VmResult<BytecodeProgram> {
    let ast = lower_source_text(source)?;

    compile_ast(source, &ast)
}

pub fn compile_ast(source: &SourceText, expressions: &[SpannedAst]) -> VmResult<BytecodeProgram> {
    let mut compiler = Compiler::new(source);
    compiler.compile_expressions(expressions, RESULT_REGISTER)?;
    compiler.emit(
        Instruction::Return {
            src: RESULT_REGISTER,
        },
        compiler.last_span,
    );

    Ok(compiler.finish())
}

pub fn run_source(source: &str) -> VmResult<VmOutput> {
    run_source_text(&SourceText::repl(source))
}

pub fn run_source_text(source: &SourceText) -> VmResult<VmOutput> {
    let program = compile_source_text(source)?;

    Vm::new().run(&program)
}

struct Compiler<'source> {
    source: &'source SourceText,
    program: BytecodeProgram,
    next_register: usize,
    last_span: SourceSpan,
}

impl<'source> Compiler<'source> {
    fn new(source: &'source SourceText) -> Self {
        Self {
            source,
            program: BytecodeProgram {
                version: BYTECODE_VERSION,
                source_id: source.id().to_string(),
                register_count: 1,
                constants: Vec::new(),
                instructions: Vec::new(),
                source: source.clone(),
            },
            next_register: 1,
            last_span: SourceSpan::point(SourceLocation::start()),
        }
    }

    fn finish(self) -> BytecodeProgram {
        self.program
    }

    fn compile_expressions(&mut self, expressions: &[SpannedAst], dst: usize) -> VmResult<()> {
        if expressions.is_empty() {
            self.load_constant(dst, Value::Nil, self.last_span);
            return Ok(());
        }

        for expression in expressions {
            self.compile_expression(expression, dst)?;
            self.last_span = expression.span;
        }
        Ok(())
    }

    fn compile_expression(&mut self, expression: &SpannedAst, dst: usize) -> VmResult<()> {
        match &expression.kind {
            AstKind::Literal { value } => {
                self.load_constant(dst, literal_value(value), expression.span);
                Ok(())
            }
            AstKind::Vector { items } => self.compile_vector(items, dst, expression.span),
            AstKind::Map { entries } => self.compile_map(entries, dst, expression.span),
            AstKind::Do { expressions } => self.compile_do(expressions, dst, expression.span),
            AstKind::If {
                condition,
                then_branch,
                else_branch,
            } => self.compile_if(condition, then_branch, else_branch, dst, expression.span),
            AstKind::Symbol { name } => Err(self.compile_error(CompileDiagnosticSpec {
                code: "ANVIL_COMPILE_UNBOUND_SYMBOL",
                message: format!("symbol {name} is not bound in the bootstrap VM"),
                span: expression.span,
                expected: vec!["literal or supported special form".to_string()],
                actual: Some(format!("symbol {name}")),
                suggestion: Some(
                    "Define executable bindings after environments and calls land.".to_string(),
                ),
            })),
            _ => Err(self.compile_error(CompileDiagnosticSpec {
                code: "ANVIL_COMPILE_UNSUPPORTED_FORM",
                message: "form is not executable in the bootstrap VM".to_string(),
                span: expression.span,
                expected: vec![
                    "literal".to_string(),
                    "vector".to_string(),
                    "map".to_string(),
                    "do".to_string(),
                    "if".to_string(),
                ],
                actual: Some(ast_kind_name(&expression.kind).to_string()),
                suggestion: Some(
                    "Use the first executable subset, or wait for the matching runtime contract."
                        .to_string(),
                ),
            })),
        }
    }

    fn compile_vector(
        &mut self,
        items: &[SpannedAst],
        dst: usize,
        span: SourceSpan,
    ) -> VmResult<()> {
        let mut item_registers = Vec::with_capacity(items.len());
        for item in items {
            let item_register = self.allocate_register();
            self.compile_expression(item, item_register)?;
            item_registers.push(item_register);
        }
        self.emit(
            Instruction::MakeVector {
                dst,
                items: item_registers,
            },
            span,
        );
        Ok(())
    }

    fn compile_map(
        &mut self,
        entries: &[crate::ast::AstMapEntry],
        dst: usize,
        span: SourceSpan,
    ) -> VmResult<()> {
        let mut entry_registers = Vec::with_capacity(entries.len());
        for entry in entries {
            let key = self.allocate_register();
            let value = self.allocate_register();
            self.compile_expression(&entry.key, key)?;
            self.compile_expression(&entry.value, value)?;
            entry_registers.push(MapRegisterEntry { key, value });
        }
        self.emit(
            Instruction::MakeMap {
                dst,
                entries: entry_registers,
            },
            span,
        );
        Ok(())
    }

    fn compile_do(
        &mut self,
        expressions: &[SpannedAst],
        dst: usize,
        span: SourceSpan,
    ) -> VmResult<()> {
        if expressions.is_empty() {
            self.load_constant(dst, Value::Nil, span);
            return Ok(());
        }

        self.compile_expressions(expressions, dst)
    }

    fn compile_if(
        &mut self,
        condition: &SpannedAst,
        then_branch: &SpannedAst,
        else_branch: &SpannedAst,
        dst: usize,
        span: SourceSpan,
    ) -> VmResult<()> {
        let condition_register = self.allocate_register();
        self.compile_expression(condition, condition_register)?;
        let false_jump = self.emit(
            Instruction::JumpIfFalse {
                condition: condition_register,
                target: usize::MAX,
            },
            span,
        );

        self.compile_expression(then_branch, dst)?;
        let end_jump = self.emit(Instruction::Jump { target: usize::MAX }, then_branch.span);

        let else_start = self.program.instructions.len();
        self.patch_jump_target(false_jump, else_start);
        self.compile_expression(else_branch, dst)?;

        let end = self.program.instructions.len();
        self.patch_jump_target(end_jump, end);
        Ok(())
    }

    fn load_constant(&mut self, dst: usize, value: Value, span: SourceSpan) {
        let constant = self.program.constants.len();
        self.program.constants.push(value);
        self.emit(Instruction::LoadConstant { dst, constant }, span);
    }

    fn allocate_register(&mut self) -> usize {
        let register = self.next_register;
        self.next_register += 1;
        self.program.register_count = self.program.register_count.max(self.next_register);
        register
    }

    fn emit(&mut self, instruction: Instruction, span: SourceSpan) -> usize {
        let index = self.program.instructions.len();
        self.program
            .instructions
            .push(BytecodeInstruction { instruction, span });
        index
    }

    fn patch_jump_target(&mut self, instruction_index: usize, target: usize) {
        match &mut self.program.instructions[instruction_index].instruction {
            Instruction::JumpIfFalse {
                target: jump_target,
                ..
            }
            | Instruction::Jump {
                target: jump_target,
            } => *jump_target = target,
            _ => unreachable!("only jump instructions are patched"),
        }
    }

    fn compile_error(&self, spec: CompileDiagnosticSpec) -> Box<VmDiagnostic> {
        Diagnostic::new(DiagnosticSpec {
            code: spec.code,
            phase: DiagnosticPhase::Compile,
            source: self.source,
            message: spec.message,
            span: spec.span,
            expected: spec.expected,
            actual: spec.actual,
            suggestion: spec.suggestion,
        })
    }
}

struct CompileDiagnosticSpec {
    code: &'static str,
    message: String,
    span: SourceSpan,
    expected: Vec<String>,
    actual: Option<String>,
    suggestion: Option<String>,
}

fn literal_value(literal: &AstLiteral) -> Value {
    match literal {
        AstLiteral::Nil => Value::Nil,
        AstLiteral::Bool(value) => Value::Bool(*value),
        AstLiteral::Integer(value) => Value::Integer(*value),
        AstLiteral::Float64(value) => Value::Float64(*value),
        AstLiteral::String(value) => Value::String(value.clone()),
        AstLiteral::Keyword(value) => Value::Keyword(value.clone()),
    }
}

fn ast_kind_name(kind: &AstKind) -> &'static str {
    match kind {
        AstKind::Literal { .. } => "literal",
        AstKind::Symbol { .. } => "symbol",
        AstKind::Quote { .. } => "quote",
        AstKind::Define { .. } => "define",
        AstKind::If { .. } => "if",
        AstKind::Do { .. } => "do",
        AstKind::Fn { .. } => "fn",
        AstKind::Require { .. } => "require",
        AstKind::Call { .. } => "call",
        AstKind::Vector { .. } => "vector",
        AstKind::Map { .. } => "map",
    }
}

struct Interpreter<'program> {
    program: &'program BytecodeProgram,
    budget: VmBudget,
    registers: Vec<Value>,
    pc: usize,
    instructions_executed: usize,
}

impl<'program> Interpreter<'program> {
    fn new(program: &'program BytecodeProgram, budget: VmBudget) -> Self {
        Self {
            program,
            budget,
            registers: vec![Value::Nil; program.register_count],
            pc: 0,
            instructions_executed: 0,
        }
    }

    fn run(mut self) -> VmResult<VmOutput> {
        loop {
            let instruction = self.current_instruction()?.clone();
            self.consume_fuel(instruction.span)?;
            self.instructions_executed += 1;

            match &instruction.instruction {
                Instruction::LoadConstant { dst, constant } => {
                    let value = self.constant(*constant, instruction.span)?.clone();
                    self.set_register(*dst, value, instruction.span)?;
                    self.pc += 1;
                }
                Instruction::MakeVector { dst, items } => {
                    let value = self.build_vector(items, instruction.span)?;
                    self.set_register(*dst, value, instruction.span)?;
                    self.pc += 1;
                }
                Instruction::MakeMap { dst, entries } => {
                    let value = self.build_map(entries, instruction.span)?;
                    self.set_register(*dst, value, instruction.span)?;
                    self.pc += 1;
                }
                Instruction::JumpIfFalse { condition, target } => {
                    if self.register(*condition, instruction.span)?.is_truthy() {
                        self.pc += 1;
                    } else {
                        self.jump_to(*target, instruction.span)?;
                    }
                }
                Instruction::Jump { target } => {
                    self.jump_to(*target, instruction.span)?;
                }
                Instruction::Return { src } => {
                    let value = self.register(*src, instruction.span)?.clone();
                    return Ok(VmOutput {
                        value,
                        instructions_executed: self.instructions_executed,
                    });
                }
            }
        }
    }

    fn current_instruction(&self) -> VmResult<&BytecodeInstruction> {
        self.program.instructions.get(self.pc).ok_or_else(|| {
            self.runtime_error(
                "ANVIL_RUNTIME_PC_OUT_OF_BOUNDS",
                "program counter moved outside the bytecode program".to_string(),
                self.program
                    .instructions
                    .last()
                    .map(|instruction| instruction.span)
                    .unwrap_or_else(|| SourceSpan::point(SourceLocation::start())),
                vec!["valid bytecode instruction".to_string()],
                Some(format!("pc {}", self.pc)),
                Some("Report this as a compiler or VM bug.".to_string()),
            )
        })
    }

    fn consume_fuel(&mut self, span: SourceSpan) -> VmResult<()> {
        let Some(remaining) = self.budget.instruction_fuel.as_mut() else {
            return Ok(());
        };
        if *remaining == 0 {
            return Err(self.runtime_error(
                "ANVIL_RUNTIME_FUEL_EXHAUSTED",
                "instruction fuel exhausted".to_string(),
                span,
                vec!["available instruction fuel".to_string()],
                Some("0 instruction(s) remaining".to_string()),
                Some("Increase the runtime budget or simplify the program.".to_string()),
            ));
        }
        *remaining -= 1;
        Ok(())
    }

    fn build_vector(&self, items: &[usize], span: SourceSpan) -> VmResult<Value> {
        let mut values = Vec::with_capacity(items.len());
        for item in items {
            values.push(self.register(*item, span)?.clone());
        }
        Ok(Value::Vector(values))
    }

    fn build_map(&self, entries: &[MapRegisterEntry], span: SourceSpan) -> VmResult<Value> {
        let mut values = Vec::with_capacity(entries.len());
        for entry in entries {
            values.push(ValueMapEntry {
                key: self.register(entry.key, span)?.clone(),
                value: self.register(entry.value, span)?.clone(),
            });
        }
        Ok(Value::Map(values))
    }

    fn jump_to(&mut self, target: usize, span: SourceSpan) -> VmResult<()> {
        if target >= self.program.instructions.len() {
            return Err(self.runtime_error(
                "ANVIL_RUNTIME_BAD_JUMP_TARGET",
                "bytecode jump target is outside the program".to_string(),
                span,
                vec!["instruction index within bytecode program".to_string()],
                Some(format!("target {target}")),
                Some("Report this as a compiler or VM bug.".to_string()),
            ));
        }
        self.pc = target;
        Ok(())
    }

    fn constant(&self, constant: usize, span: SourceSpan) -> VmResult<&Value> {
        self.program.constants.get(constant).ok_or_else(|| {
            self.runtime_error(
                "ANVIL_RUNTIME_CONSTANT_OUT_OF_BOUNDS",
                "bytecode constant index is outside the constant table".to_string(),
                span,
                vec!["valid constant index".to_string()],
                Some(format!("constant {constant}")),
                Some("Report this as a compiler or VM bug.".to_string()),
            )
        })
    }

    fn register(&self, register: usize, span: SourceSpan) -> VmResult<&Value> {
        self.registers.get(register).ok_or_else(|| {
            self.runtime_error(
                "ANVIL_RUNTIME_REGISTER_OUT_OF_BOUNDS",
                "bytecode register index is outside the register file".to_string(),
                span,
                vec!["valid register index".to_string()],
                Some(format!("register {register}")),
                Some("Report this as a compiler or VM bug.".to_string()),
            )
        })
    }

    fn set_register(&mut self, register: usize, value: Value, span: SourceSpan) -> VmResult<()> {
        let Some(slot) = self.registers.get_mut(register) else {
            return Err(self.runtime_error(
                "ANVIL_RUNTIME_REGISTER_OUT_OF_BOUNDS",
                "bytecode register index is outside the register file".to_string(),
                span,
                vec!["valid register index".to_string()],
                Some(format!("register {register}")),
                Some("Report this as a compiler or VM bug.".to_string()),
            ));
        };
        *slot = value;
        Ok(())
    }

    fn runtime_error(
        &self,
        code: &'static str,
        message: String,
        span: SourceSpan,
        expected: Vec<String>,
        actual: Option<String>,
        suggestion: Option<String>,
    ) -> Box<VmDiagnostic> {
        Diagnostic::new(DiagnosticSpec {
            code,
            phase: DiagnosticPhase::Runtime,
            source: self.program.source(),
            message,
            span,
            expected,
            actual,
            suggestion,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_value(source: &str) -> Value {
        run_source(source).expect("VM output").value
    }

    fn bytecode(instructions: Vec<Instruction>, constants: Vec<Value>) -> BytecodeProgram {
        bytecode_with_registers(instructions, constants, 1)
    }

    fn bytecode_with_registers(
        instructions: Vec<Instruction>,
        constants: Vec<Value>,
        register_count: usize,
    ) -> BytecodeProgram {
        let source = SourceText::new("malformed-bytecode", "broken");
        BytecodeProgram {
            version: BYTECODE_VERSION,
            source_id: source.id().to_string(),
            register_count,
            constants,
            instructions: instructions
                .into_iter()
                .map(|instruction| BytecodeInstruction {
                    instruction,
                    span: SourceSpan::point(SourceLocation::start()),
                })
                .collect(),
            source,
        }
    }

    fn runtime_code(program: BytecodeProgram) -> String {
        Vm::with_budget(VmBudget::unlimited())
            .run(&program)
            .expect_err("runtime diagnostic")
            .code
            .to_string()
    }

    #[test]
    fn runs_literals_and_top_level_sequences() {
        assert_eq!(run_value("nil"), Value::Nil);
        assert_eq!(run_value("false"), Value::Bool(false));
        assert_eq!(run_value("42"), Value::Integer(42));
        assert_eq!(run_value("1.5"), Value::Float64(1.5));
        assert_eq!(run_value("\"agent\""), Value::String("agent".into()));
        assert_eq!(run_value("1 2"), Value::Integer(2));
        assert_eq!(run_value(""), Value::Nil);
    }

    #[test]
    fn runs_do_and_if_forms() {
        assert_eq!(run_value("(do 1 2 3)"), Value::Integer(3));
        assert_eq!(
            run_value("(if true :yes :no)"),
            Value::Keyword("yes".into())
        );
        assert_eq!(
            run_value("(if false :yes :no)"),
            Value::Keyword("no".into())
        );
        assert_eq!(run_value("(if nil :yes :no)"), Value::Keyword("no".into()));
        assert_eq!(run_value("(if 0 :yes :no)"), Value::Keyword("yes".into()));
    }

    #[test]
    fn runs_vectors_and_ordered_maps() {
        let value = run_value("(do [1 2] {:ok true :answer 42})");

        assert_eq!(
            value,
            Value::Map(vec![
                ValueMapEntry {
                    key: Value::Keyword("ok".into()),
                    value: Value::Bool(true),
                },
                ValueMapEntry {
                    key: Value::Keyword("answer".into()),
                    value: Value::Integer(42),
                },
            ])
        );
        assert_eq!(value.to_string(), "{:ok true :answer 42}");
    }

    #[test]
    fn displays_all_value_forms_with_escaping() {
        let value = Value::Vector(vec![
            Value::Nil,
            Value::Bool(true),
            Value::Integer(7),
            Value::Float64(2.5),
            Value::String("a\\b\"c\n".into()),
            Value::Keyword("ready".into()),
            Value::Map(vec![ValueMapEntry {
                key: Value::Keyword("nested".into()),
                value: Value::Bool(false),
            }]),
        ]);

        assert_eq!(
            value.to_string(),
            "[nil true 7 2.5 \"a\\\\b\\\"c\\n\" :ready {:nested false}]"
        );
    }

    #[test]
    fn records_source_spans_on_bytecode_instructions() {
        let program = compile_source("(if false 1 2)").expect("bytecode");

        assert!(matches!(
            program.instructions[1].instruction,
            Instruction::JumpIfFalse { .. }
        ));
        assert_eq!(program.instructions[1].span.start.column, 1);
    }

    #[test]
    fn reports_unsupported_forms_as_compile_diagnostics() {
        let diagnostic = compile_source("(define answer 42)").expect_err("compile diagnostic");

        assert_eq!(diagnostic.code, "ANVIL_COMPILE_UNSUPPORTED_FORM");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Compile);
        assert_eq!(diagnostic.primary_span.start.column, 1);
    }

    #[test]
    fn reports_unbound_symbols_as_compile_diagnostics() {
        let diagnostic = compile_source("answer").expect_err("compile diagnostic");

        assert_eq!(diagnostic.code, "ANVIL_COMPILE_UNBOUND_SYMBOL");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Compile);
        assert_eq!(diagnostic.actual.as_deref(), Some("symbol answer"));
    }

    #[test]
    fn reports_fuel_exhaustion_as_runtime_diagnostic() {
        let program = compile_source("42").expect("bytecode");
        let diagnostic = Vm::with_budget(VmBudget::with_instruction_fuel(0))
            .run(&program)
            .expect_err("runtime diagnostic");

        assert_eq!(diagnostic.code, "ANVIL_RUNTIME_FUEL_EXHAUSTED");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Runtime);
        assert_eq!(diagnostic.primary_span.start.column, 1);
    }

    #[test]
    fn unlimited_budget_runs_without_consuming_fuel() {
        let program = compile_source("(do 1 2)").expect("bytecode");
        let output = Vm::with_budget(VmBudget::unlimited())
            .run(&program)
            .expect("VM output");

        assert_eq!(output.value, Value::Integer(2));
        assert!(output.instructions_executed > 0);
    }

    #[test]
    fn malformed_bytecode_reports_pc_out_of_bounds() {
        assert_eq!(
            runtime_code(bytecode(Vec::new(), Vec::new())),
            "ANVIL_RUNTIME_PC_OUT_OF_BOUNDS"
        );
    }

    #[test]
    fn malformed_bytecode_reports_constant_out_of_bounds() {
        assert_eq!(
            runtime_code(bytecode(
                vec![Instruction::LoadConstant {
                    dst: 0,
                    constant: 9,
                }],
                Vec::new(),
            )),
            "ANVIL_RUNTIME_CONSTANT_OUT_OF_BOUNDS"
        );
    }

    #[test]
    fn malformed_bytecode_reports_register_read_out_of_bounds() {
        assert_eq!(
            runtime_code(bytecode(vec![Instruction::Return { src: 9 }], Vec::new(),)),
            "ANVIL_RUNTIME_REGISTER_OUT_OF_BOUNDS"
        );
    }

    #[test]
    fn malformed_bytecode_reports_register_write_out_of_bounds() {
        assert_eq!(
            runtime_code(bytecode(
                vec![Instruction::LoadConstant {
                    dst: 9,
                    constant: 0,
                }],
                vec![Value::Nil],
            )),
            "ANVIL_RUNTIME_REGISTER_OUT_OF_BOUNDS"
        );
    }

    #[test]
    fn malformed_bytecode_reports_bad_jump_targets() {
        assert_eq!(
            runtime_code(bytecode(vec![Instruction::Jump { target: 99 }], Vec::new(),)),
            "ANVIL_RUNTIME_BAD_JUMP_TARGET"
        );
    }

    #[test]
    fn malformed_bytecode_reports_vector_register_errors() {
        assert_eq!(
            runtime_code(bytecode(
                vec![Instruction::MakeVector {
                    dst: 0,
                    items: vec![9],
                }],
                Vec::new(),
            )),
            "ANVIL_RUNTIME_REGISTER_OUT_OF_BOUNDS"
        );
    }

    #[test]
    fn malformed_bytecode_reports_map_register_errors() {
        assert_eq!(
            runtime_code(bytecode_with_registers(
                vec![Instruction::MakeMap {
                    dst: 0,
                    entries: vec![MapRegisterEntry { key: 0, value: 9 }],
                }],
                Vec::new(),
                1,
            )),
            "ANVIL_RUNTIME_REGISTER_OUT_OF_BOUNDS"
        );
    }

    #[test]
    fn malformed_bytecode_reports_jump_condition_register_errors() {
        assert_eq!(
            runtime_code(bytecode(
                vec![Instruction::JumpIfFalse {
                    condition: 9,
                    target: 0,
                }],
                Vec::new(),
            )),
            "ANVIL_RUNTIME_REGISTER_OUT_OF_BOUNDS"
        );
    }
}
