use std::{collections::BTreeMap, fmt};

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
    pub bindings: Vec<String>,
    pub functions: Vec<FunctionPrototype>,
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
pub struct FunctionPrototype {
    pub params: Vec<String>,
    pub register_count: usize,
    pub instructions: Vec<BytecodeInstruction>,
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
    LoadBinding {
        dst: usize,
        binding: usize,
    },
    DefineBinding {
        binding: usize,
        src: usize,
    },
    LoadFunction {
        dst: usize,
        function: usize,
    },
    CallPrimitive {
        dst: usize,
        primitive: usize,
        args: Vec<usize>,
    },
    CallFunction {
        dst: usize,
        callee: usize,
        args: Vec<usize>,
    },
    TailCallFunction {
        callee: usize,
        args: Vec<usize>,
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
    Function(FunctionValue),
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
            Self::Function(value) => write!(f, "#<fn:{}>", value.function),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ValueMapEntry {
    pub key: Value,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FunctionValue {
    pub function: usize,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub captures: BTreeMap<String, Value>,
}

impl FunctionValue {
    pub fn new(function: usize) -> Self {
        Self {
            function,
            captures: BTreeMap::new(),
        }
    }

    fn with_captures(function: usize, captures: BTreeMap<String, Value>) -> Self {
        Self { function, captures }
    }
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
    pub max_call_depth: usize,
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
        compiler.unit.last_span,
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
    constants: Vec<Value>,
    bindings: Vec<String>,
    functions: Vec<FunctionPrototype>,
    unit: CompileUnit,
}

#[derive(Debug, Clone, PartialEq)]
struct CompileUnit {
    register_count: usize,
    instructions: Vec<BytecodeInstruction>,
    next_register: usize,
    last_span: SourceSpan,
}

impl CompileUnit {
    fn new() -> Self {
        Self {
            register_count: 1,
            instructions: Vec::new(),
            next_register: 1,
            last_span: SourceSpan::point(SourceLocation::start()),
        }
    }
}

impl<'source> Compiler<'source> {
    fn new(source: &'source SourceText) -> Self {
        Self {
            source,
            constants: Vec::new(),
            bindings: Vec::new(),
            functions: Vec::new(),
            unit: CompileUnit::new(),
        }
    }

    fn finish(self) -> BytecodeProgram {
        BytecodeProgram {
            version: BYTECODE_VERSION,
            source_id: self.source.id().to_string(),
            register_count: self.unit.register_count,
            constants: self.constants,
            bindings: self.bindings,
            functions: self.functions,
            instructions: self.unit.instructions,
            source: self.source.clone(),
        }
    }

    fn compile_expressions(&mut self, expressions: &[SpannedAst], dst: usize) -> VmResult<()> {
        if expressions.is_empty() {
            self.load_constant(dst, Value::Nil, self.unit.last_span);
            return Ok(());
        }

        for expression in expressions {
            self.compile_expression(expression, dst)?;
            self.unit.last_span = expression.span;
        }
        Ok(())
    }

    fn compile_tail_expressions(&mut self, expressions: &[SpannedAst], dst: usize) -> VmResult<()> {
        let Some((tail, prefix)) = expressions.split_last() else {
            self.load_constant(dst, Value::Nil, self.unit.last_span);
            return Ok(());
        };

        for expression in prefix {
            self.compile_expression(expression, dst)?;
            self.unit.last_span = expression.span;
        }
        self.compile_tail_expression(tail, dst)?;
        self.unit.last_span = tail.span;
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
            AstKind::Define { name, value } => {
                self.compile_define(name, value, dst, expression.span)
            }
            AstKind::Fn { params, body } => self.compile_fn(params, body, dst, expression.span),
            AstKind::If {
                condition,
                then_branch,
                else_branch,
            } => self.compile_if(condition, then_branch, else_branch, dst, expression.span),
            AstKind::Symbol { name } => {
                let binding = self.intern_binding(name);
                self.emit(Instruction::LoadBinding { dst, binding }, expression.span);
                Ok(())
            }
            AstKind::Call { callee, args } => self.compile_call(callee, args, dst, expression.span),
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
                    "define".to_string(),
                    "fn".to_string(),
                    "symbol".to_string(),
                    "call".to_string(),
                ],
                actual: Some(ast_kind_name(&expression.kind).to_string()),
                suggestion: Some(
                    "Use the first executable subset, or wait for the matching runtime contract."
                        .to_string(),
                ),
            })),
        }
    }

    fn compile_tail_expression(&mut self, expression: &SpannedAst, dst: usize) -> VmResult<()> {
        match &expression.kind {
            AstKind::Do { expressions } => self.compile_tail_do(expressions, dst, expression.span),
            AstKind::If {
                condition,
                then_branch,
                else_branch,
            } => self.compile_tail_if(condition, then_branch, else_branch, dst, expression.span),
            AstKind::Call { callee, args } => {
                self.compile_tail_call(callee, args, dst, expression.span)
            }
            _ => self.compile_expression(expression, dst),
        }
    }

    fn compile_define(
        &mut self,
        name: &str,
        value: &SpannedAst,
        dst: usize,
        span: SourceSpan,
    ) -> VmResult<()> {
        self.compile_expression(value, dst)?;
        let binding = self.intern_binding(name);
        self.emit(Instruction::DefineBinding { binding, src: dst }, span);
        Ok(())
    }

    fn compile_fn(
        &mut self,
        params: &[String],
        body: &[SpannedAst],
        dst: usize,
        span: SourceSpan,
    ) -> VmResult<()> {
        let parent_unit = std::mem::replace(&mut self.unit, CompileUnit::new());
        let prototype = self.compile_function_prototype(params, body);
        self.unit = parent_unit;

        let function = self.push_function(prototype?);
        self.emit(Instruction::LoadFunction { dst, function }, span);
        Ok(())
    }

    fn compile_function_prototype(
        &mut self,
        params: &[String],
        body: &[SpannedAst],
    ) -> VmResult<FunctionPrototype> {
        self.compile_tail_expressions(body, RESULT_REGISTER)?;
        self.emit(
            Instruction::Return {
                src: RESULT_REGISTER,
            },
            self.unit.last_span,
        );

        Ok(FunctionPrototype {
            params: params.to_vec(),
            register_count: self.unit.register_count,
            instructions: self.unit.instructions.clone(),
        })
    }

    fn compile_call(
        &mut self,
        callee: &SpannedAst,
        args: &[SpannedAst],
        dst: usize,
        span: SourceSpan,
    ) -> VmResult<()> {
        if let AstKind::Symbol { name } = &callee.kind
            && is_bootstrap_primitive(name)
        {
            return self.compile_primitive_call(name, args, dst, span);
        }

        self.compile_function_call(callee, args, Some(dst), span)
    }

    fn compile_tail_call(
        &mut self,
        callee: &SpannedAst,
        args: &[SpannedAst],
        dst: usize,
        span: SourceSpan,
    ) -> VmResult<()> {
        if let AstKind::Symbol { name } = &callee.kind
            && is_bootstrap_primitive(name)
        {
            return self.compile_primitive_call(name, args, dst, span);
        }

        self.compile_function_call(callee, args, None, span)
    }

    fn compile_function_call(
        &mut self,
        callee: &SpannedAst,
        args: &[SpannedAst],
        dst: Option<usize>,
        span: SourceSpan,
    ) -> VmResult<()> {
        let callee_register = self.allocate_register();
        self.compile_expression(callee, callee_register)?;
        let mut arg_registers = Vec::with_capacity(args.len());
        for arg in args {
            let arg_register = self.allocate_register();
            self.compile_expression(arg, arg_register)?;
            arg_registers.push(arg_register);
        }

        let instruction = match dst {
            Some(dst) => Instruction::CallFunction {
                dst,
                callee: callee_register,
                args: arg_registers,
            },
            None => Instruction::TailCallFunction {
                callee: callee_register,
                args: arg_registers,
            },
        };

        self.emit(instruction, span);
        Ok(())
    }

    fn compile_primitive_call(
        &mut self,
        name: &str,
        args: &[SpannedAst],
        dst: usize,
        span: SourceSpan,
    ) -> VmResult<()> {
        let mut arg_registers = Vec::with_capacity(args.len());
        for arg in args {
            let arg_register = self.allocate_register();
            self.compile_expression(arg, arg_register)?;
            arg_registers.push(arg_register);
        }
        let primitive = self.intern_binding(name);
        self.emit(
            Instruction::CallPrimitive {
                dst,
                primitive,
                args: arg_registers,
            },
            span,
        );
        Ok(())
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

    fn compile_tail_do(
        &mut self,
        expressions: &[SpannedAst],
        dst: usize,
        span: SourceSpan,
    ) -> VmResult<()> {
        if expressions.is_empty() {
            self.load_constant(dst, Value::Nil, span);
            return Ok(());
        }

        self.compile_tail_expressions(expressions, dst)
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

        let else_start = self.unit.instructions.len();
        self.patch_jump_target(false_jump, else_start);
        self.compile_expression(else_branch, dst)?;

        let end = self.unit.instructions.len();
        self.patch_jump_target(end_jump, end);
        Ok(())
    }

    fn compile_tail_if(
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

        self.compile_tail_expression(then_branch, dst)?;
        let end_jump = self.emit(Instruction::Jump { target: usize::MAX }, then_branch.span);

        let else_start = self.unit.instructions.len();
        self.patch_jump_target(false_jump, else_start);
        self.compile_tail_expression(else_branch, dst)?;

        let end = self.unit.instructions.len();
        self.patch_jump_target(end_jump, end);
        Ok(())
    }

    fn load_constant(&mut self, dst: usize, value: Value, span: SourceSpan) {
        let constant = self.constants.len();
        self.constants.push(value);
        self.emit(Instruction::LoadConstant { dst, constant }, span);
    }

    fn intern_binding(&mut self, name: &str) -> usize {
        if let Some(index) = self.bindings.iter().position(|binding| binding == name) {
            return index;
        }

        let index = self.bindings.len();
        self.bindings.push(name.to_string());
        index
    }

    fn push_function(&mut self, function: FunctionPrototype) -> usize {
        let index = self.functions.len();
        self.functions.push(function);
        index
    }

    fn allocate_register(&mut self) -> usize {
        let register = self.unit.next_register;
        self.unit.next_register += 1;
        self.unit.register_count = self.unit.register_count.max(self.unit.next_register);
        register
    }

    fn emit(&mut self, instruction: Instruction, span: SourceSpan) -> usize {
        let index = self.unit.instructions.len();
        self.unit
            .instructions
            .push(BytecodeInstruction { instruction, span });
        index
    }

    fn patch_jump_target(&mut self, instruction_index: usize, target: usize) {
        match &mut self.unit.instructions[instruction_index].instruction {
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

fn is_bootstrap_primitive(name: &str) -> bool {
    matches!(name, "+" | "-" | "*" | "=")
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameCode {
    TopLevel,
    Function(usize),
}

#[derive(Debug, Clone, PartialEq)]
struct ExecutionFrame {
    code: FrameCode,
    registers: Vec<Value>,
    locals: BTreeMap<String, Value>,
    pc: usize,
    return_register: Option<usize>,
}

impl ExecutionFrame {
    fn top_level(register_count: usize) -> Self {
        Self {
            code: FrameCode::TopLevel,
            registers: vec![Value::Nil; register_count],
            locals: BTreeMap::new(),
            pc: 0,
            return_register: None,
        }
    }

    fn function(
        function: usize,
        register_count: usize,
        locals: BTreeMap<String, Value>,
        return_register: usize,
    ) -> Self {
        Self {
            code: FrameCode::Function(function),
            registers: vec![Value::Nil; register_count],
            locals,
            pc: 0,
            return_register: Some(return_register),
        }
    }
}

struct Interpreter<'program> {
    program: &'program BytecodeProgram,
    budget: VmBudget,
    frames: Vec<ExecutionFrame>,
    bindings: BTreeMap<String, Value>,
    instructions_executed: usize,
    max_call_depth: usize,
}

impl<'program> Interpreter<'program> {
    fn new(program: &'program BytecodeProgram, budget: VmBudget) -> Self {
        Self {
            program,
            budget,
            frames: vec![ExecutionFrame::top_level(program.register_count)],
            bindings: BTreeMap::new(),
            instructions_executed: 0,
            max_call_depth: 1,
        }
    }

    fn run(mut self) -> VmResult<VmOutput> {
        loop {
            let instruction = self.current_instruction()?.clone();
            self.consume_fuel(instruction.span)?;
            self.instructions_executed += 1;

            if let Some(output) = self.execute_instruction(instruction)? {
                return Ok(output);
            }
        }
    }

    fn execute_instruction(
        &mut self,
        instruction: BytecodeInstruction,
    ) -> VmResult<Option<VmOutput>> {
        let span = instruction.span;
        match instruction.instruction {
            Instruction::JumpIfFalse { condition, target } => {
                self.execute_jump_if_false(condition, target, span)?;
                Ok(None)
            }
            Instruction::Jump { target } => {
                self.jump_to(target, span)?;
                Ok(None)
            }
            Instruction::Return { src } => self.return_from_frame(src, span),
            instruction => {
                self.execute_value_instruction(instruction, span)?;
                Ok(None)
            }
        }
    }

    fn execute_value_instruction(
        &mut self,
        instruction: Instruction,
        span: SourceSpan,
    ) -> VmResult<()> {
        match instruction {
            Instruction::LoadConstant { dst, constant } => {
                let value = self.constant(constant, span)?.clone();
                self.set_register(dst, value, span)?;
                self.advance_pc()
            }
            Instruction::MakeVector { dst, items } => {
                let value = self.build_vector(&items, span)?;
                self.set_register(dst, value, span)?;
                self.advance_pc()
            }
            Instruction::MakeMap { dst, entries } => {
                let value = self.build_map(&entries, span)?;
                self.set_register(dst, value, span)?;
                self.advance_pc()
            }
            Instruction::LoadBinding { dst, binding } => {
                let value = self.load_binding(binding, span)?;
                self.set_register(dst, value, span)?;
                self.advance_pc()
            }
            Instruction::DefineBinding { binding, src } => {
                self.define_binding(binding, src, span)?;
                self.advance_pc()
            }
            Instruction::LoadFunction { dst, function } => {
                self.function(function, span)?;
                let captures = self.current_frame()?.locals.clone();
                self.set_register(
                    dst,
                    Value::Function(FunctionValue::with_captures(function, captures)),
                    span,
                )?;
                self.advance_pc()
            }
            Instruction::CallPrimitive {
                dst,
                primitive,
                args,
            } => {
                let primitive_name = self.binding_name(primitive, span)?.to_string();
                let value = self.call_primitive(&primitive_name, &args, span)?;
                self.set_register(dst, value, span)?;
                self.advance_pc()
            }
            Instruction::CallFunction { dst, callee, args } => {
                self.call_function(dst, callee, &args, span)
            }
            Instruction::TailCallFunction { callee, args } => {
                self.tail_call_function(callee, &args, span)
            }
            _ => unreachable!("control instructions are handled by execute_instruction"),
        }
    }

    fn execute_jump_if_false(
        &mut self,
        condition: usize,
        target: usize,
        span: SourceSpan,
    ) -> VmResult<()> {
        if self.register(condition, span)?.is_truthy() {
            return self.advance_pc();
        }

        self.jump_to(target, span)
    }

    fn return_from_frame(&mut self, src: usize, span: SourceSpan) -> VmResult<Option<VmOutput>> {
        let value = self.register(src, span)?.clone();
        let return_register = self.current_frame()?.return_register;
        let Some(dst) = return_register else {
            return Ok(Some(VmOutput {
                value,
                instructions_executed: self.instructions_executed,
                max_call_depth: self.max_call_depth,
            }));
        };

        self.frames.pop();
        self.set_register(dst, value, span)?;
        Ok(None)
    }

    fn current_instruction(&self) -> VmResult<&BytecodeInstruction> {
        let (code, pc) = {
            let frame = self.current_frame()?;
            (frame.code, frame.pc)
        };
        let instructions = self.instructions_for(code)?;
        instructions.get(pc).ok_or_else(|| {
            self.runtime_error(
                "ANVIL_RUNTIME_PC_OUT_OF_BOUNDS",
                "program counter moved outside the bytecode program".to_string(),
                self.last_instruction_span(code),
                vec!["valid bytecode instruction".to_string()],
                Some(format!("pc {pc}")),
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

    fn define_binding(&mut self, binding: usize, src: usize, span: SourceSpan) -> VmResult<()> {
        let name = self.binding_name(binding, span)?.to_string();
        let value = self.register(src, span)?.clone();
        self.bindings.insert(name, value);
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

    fn load_binding(&self, binding: usize, span: SourceSpan) -> VmResult<Value> {
        let name = self.binding_name(binding, span)?;
        if let Some(value) = self.current_frame()?.locals.get(name) {
            return Ok(value.clone());
        }

        self.bindings.get(name).cloned().ok_or_else(|| {
            self.runtime_error(
                "ANVIL_RUNTIME_UNBOUND_SYMBOL",
                format!("symbol {name} is not bound"),
                span,
                vec!["local parameter or defined top-level binding".to_string()],
                Some(format!("symbol {name}")),
                Some("Define the symbol before reading it in this VM program.".to_string()),
            )
        })
    }

    fn call_function(
        &mut self,
        dst: usize,
        callee: usize,
        args: &[usize],
        span: SourceSpan,
    ) -> VmResult<()> {
        let callee = self.register(callee, span)?.clone();
        let frame = self.build_function_frame(callee, args, dst, span)?;

        self.advance_pc()?;
        self.frames.push(frame);
        self.record_call_depth();
        Ok(())
    }

    fn tail_call_function(
        &mut self,
        callee: usize,
        args: &[usize],
        span: SourceSpan,
    ) -> VmResult<()> {
        let return_register = self
            .current_frame()?
            .return_register
            .ok_or_else(|| self.tail_call_outside_function(span))?;
        let callee = self.register(callee, span)?.clone();
        let frame = self.build_function_frame(callee, args, return_register, span)?;

        *self.current_frame_mut()? = frame;
        Ok(())
    }

    fn build_function_frame(
        &self,
        callee: Value,
        args: &[usize],
        return_register: usize,
        span: SourceSpan,
    ) -> VmResult<ExecutionFrame> {
        let Value::Function(function) = callee else {
            return Err(self.not_callable(callee, span));
        };

        let arg_values = self.register_values(args, span)?;
        let (register_count, params) = self.function_signature(function.function, span)?;
        self.ensure_arity(params.len(), arg_values.len(), span)?;
        let mut locals = function.captures;
        locals.extend(params.into_iter().zip(arg_values));

        Ok(ExecutionFrame::function(
            function.function,
            register_count,
            locals,
            return_register,
        ))
    }

    fn function_signature(
        &self,
        function: usize,
        span: SourceSpan,
    ) -> VmResult<(usize, Vec<String>)> {
        let prototype = self.function(function, span)?;
        Ok((prototype.register_count, prototype.params.clone()))
    }

    fn ensure_arity(&self, expected: usize, actual: usize, span: SourceSpan) -> VmResult<()> {
        if expected == actual {
            return Ok(());
        }

        Err(self.runtime_error(
            "ANVIL_RUNTIME_ARITY",
            format!("function expects {expected} argument(s)"),
            span,
            vec![format!("{expected} argument(s)")],
            Some(format!("{actual} argument(s)")),
            Some("Call the function with the parameter count declared by fn.".to_string()),
        ))
    }

    fn not_callable(&self, value: Value, span: SourceSpan) -> Box<VmDiagnostic> {
        self.runtime_error(
            "ANVIL_RUNTIME_NOT_CALLABLE",
            "value is not callable".to_string(),
            span,
            vec!["function value".to_string()],
            Some(value_type_name(&value).to_string()),
            Some(
                "Call a function value, or check that the callee symbol is bound to one."
                    .to_string(),
            ),
        )
    }

    fn tail_call_outside_function(&self, span: SourceSpan) -> Box<VmDiagnostic> {
        self.runtime_error(
            "ANVIL_RUNTIME_TAIL_CALL_OUTSIDE_FUNCTION",
            "tail call bytecode can only run from a function frame".to_string(),
            span,
            vec!["active function frame".to_string()],
            Some("top-level frame".to_string()),
            Some("Report this as a compiler or bytecode construction bug.".to_string()),
        )
    }

    fn call_primitive(&self, primitive: &str, args: &[usize], span: SourceSpan) -> VmResult<Value> {
        let args = self.register_values(args, span)?;
        match primitive {
            "+" => self.primitive_add(&args, span),
            "-" => self.primitive_subtract(&args, span),
            "*" => self.primitive_multiply(&args, span),
            "=" => self.primitive_numeric_equals(&args, span),
            _ => Err(self.runtime_error(
                "ANVIL_RUNTIME_UNKNOWN_PRIMITIVE",
                format!("unknown bootstrap primitive {primitive}"),
                span,
                vec!["known bootstrap primitive".to_string()],
                Some(primitive.to_string()),
                Some("Report this as a compiler or bytecode construction bug.".to_string()),
            )),
        }
    }

    fn register_values(&self, args: &[usize], span: SourceSpan) -> VmResult<Vec<Value>> {
        args.iter()
            .map(|arg| self.register(*arg, span).cloned())
            .collect()
    }

    fn primitive_add(&self, args: &[Value], span: SourceSpan) -> VmResult<Value> {
        let mut sum = RuntimeNumber::Integer(0);
        for (index, arg) in args.iter().enumerate() {
            sum = sum.add(self.expect_number(arg, index, "+", span)?, span, self)?;
        }
        Ok(sum.into_value())
    }

    fn primitive_subtract(&self, args: &[Value], span: SourceSpan) -> VmResult<Value> {
        let Some((first, rest)) = args.split_first() else {
            return Err(self.runtime_error(
                "ANVIL_RUNTIME_ARITY",
                "- expects at least one operand".to_string(),
                span,
                vec!["one or more numeric operands".to_string()],
                Some("0 operand(s)".to_string()),
                Some("Pass a number to negate, or a left-hand value and subtrahends.".to_string()),
            ));
        };

        let mut difference = self.expect_number(first, 0, "-", span)?;
        if rest.is_empty() {
            return difference.negate(span, self).map(RuntimeNumber::into_value);
        }

        for (index, arg) in rest.iter().enumerate() {
            difference =
                difference.subtract(self.expect_number(arg, index + 1, "-", span)?, span, self)?;
        }
        Ok(difference.into_value())
    }

    fn primitive_multiply(&self, args: &[Value], span: SourceSpan) -> VmResult<Value> {
        let mut product = RuntimeNumber::Integer(1);
        for (index, arg) in args.iter().enumerate() {
            product = product.multiply(self.expect_number(arg, index, "*", span)?, span, self)?;
        }
        Ok(product.into_value())
    }

    fn primitive_numeric_equals(&self, args: &[Value], span: SourceSpan) -> VmResult<Value> {
        let numbers = args
            .iter()
            .enumerate()
            .map(|(index, arg)| self.expect_number(arg, index, "=", span))
            .collect::<VmResult<Vec<_>>>()?;
        let Some((first, rest)) = numbers.split_first() else {
            return Ok(Value::Bool(true));
        };

        Ok(Value::Bool(
            rest.iter().all(|number| first.numeric_eq(*number)),
        ))
    }

    fn expect_number(
        &self,
        value: &Value,
        index: usize,
        primitive: &str,
        span: SourceSpan,
    ) -> VmResult<RuntimeNumber> {
        RuntimeNumber::try_from_value(value).ok_or_else(|| {
            self.runtime_error(
                "ANVIL_RUNTIME_TYPE_ERROR",
                format!("{primitive} expects numeric operands"),
                span,
                vec!["integer or Float64 operand".to_string()],
                Some(format!(
                    "argument {} was {}",
                    index + 1,
                    value_type_name(value)
                )),
                Some("Convert the value explicitly before using numeric primitives.".to_string()),
            )
        })
    }

    fn jump_to(&mut self, target: usize, span: SourceSpan) -> VmResult<()> {
        let instruction_count = self.instructions_for(self.current_frame()?.code)?.len();
        if target >= instruction_count {
            return Err(self.runtime_error(
                "ANVIL_RUNTIME_BAD_JUMP_TARGET",
                "bytecode jump target is outside the program".to_string(),
                span,
                vec!["instruction index within bytecode program".to_string()],
                Some(format!("target {target}")),
                Some("Report this as a compiler or VM bug.".to_string()),
            ));
        }
        self.current_frame_mut()?.pc = target;
        Ok(())
    }

    fn advance_pc(&mut self) -> VmResult<()> {
        self.current_frame_mut()?.pc += 1;
        Ok(())
    }

    fn record_call_depth(&mut self) {
        self.max_call_depth = self.max_call_depth.max(self.frames.len());
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

    fn function(&self, function: usize, span: SourceSpan) -> VmResult<&FunctionPrototype> {
        self.program.functions.get(function).ok_or_else(|| {
            self.runtime_error(
                "ANVIL_RUNTIME_FUNCTION_OUT_OF_BOUNDS",
                "bytecode function index is outside the function table".to_string(),
                span,
                vec!["valid function index".to_string()],
                Some(format!("function {function}")),
                Some("Report this as a compiler or bytecode construction bug.".to_string()),
            )
        })
    }

    fn binding_name(&self, binding: usize, span: SourceSpan) -> VmResult<&str> {
        self.program
            .bindings
            .get(binding)
            .map(String::as_str)
            .ok_or_else(|| {
                self.runtime_error(
                    "ANVIL_RUNTIME_BINDING_OUT_OF_BOUNDS",
                    "bytecode binding index is outside the binding table".to_string(),
                    span,
                    vec!["valid binding index".to_string()],
                    Some(format!("binding {binding}")),
                    Some("Report this as a compiler or VM bug.".to_string()),
                )
            })
    }

    fn register(&self, register: usize, span: SourceSpan) -> VmResult<&Value> {
        self.current_frame()?
            .registers
            .get(register)
            .ok_or_else(|| {
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
        let register_count = self.current_frame()?.registers.len();
        if register >= register_count {
            return Err(self.runtime_error(
                "ANVIL_RUNTIME_REGISTER_OUT_OF_BOUNDS",
                "bytecode register index is outside the register file".to_string(),
                span,
                vec!["valid register index".to_string()],
                Some(format!("register {register}")),
                Some("Report this as a compiler or VM bug.".to_string()),
            ));
        };

        self.current_frame_mut()?.registers[register] = value;
        Ok(())
    }

    fn current_frame(&self) -> VmResult<&ExecutionFrame> {
        self.frames.last().ok_or_else(|| self.empty_frame_error())
    }

    fn current_frame_mut(&mut self) -> VmResult<&mut ExecutionFrame> {
        if self.frames.is_empty() {
            return Err(self.empty_frame_error());
        }

        Ok(self
            .frames
            .last_mut()
            .expect("frame stack was checked before mutable access"))
    }

    fn instructions_for(&self, code: FrameCode) -> VmResult<&[BytecodeInstruction]> {
        match code {
            FrameCode::TopLevel => Ok(&self.program.instructions),
            FrameCode::Function(function) => Ok(self
                .function(function, SourceSpan::point(SourceLocation::start()))?
                .instructions
                .as_slice()),
        }
    }

    fn last_instruction_span(&self, code: FrameCode) -> SourceSpan {
        self.instructions_for(code)
            .ok()
            .and_then(|instructions| instructions.last())
            .map(|instruction| instruction.span)
            .unwrap_or_else(|| SourceSpan::point(SourceLocation::start()))
    }

    fn empty_frame_error(&self) -> Box<VmDiagnostic> {
        self.runtime_error(
            "ANVIL_RUNTIME_EMPTY_FRAME_STACK",
            "VM frame stack is empty".to_string(),
            SourceSpan::point(SourceLocation::start()),
            vec!["active execution frame".to_string()],
            Some("empty frame stack".to_string()),
            Some("Report this as a VM bug.".to_string()),
        )
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

#[derive(Debug, Clone, Copy, PartialEq)]
enum RuntimeNumber {
    Integer(i64),
    Float64(f64),
}

impl RuntimeNumber {
    fn try_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Integer(value) => Some(Self::Integer(*value)),
            Value::Float64(value) => Some(Self::Float64(*value)),
            _ => None,
        }
    }

    fn into_value(self) -> Value {
        match self {
            Self::Integer(value) => Value::Integer(value),
            Self::Float64(value) => Value::Float64(value),
        }
    }

    fn add(self, rhs: Self, span: SourceSpan, interpreter: &Interpreter<'_>) -> VmResult<Self> {
        self.integer_op(rhs, span, interpreter, |lhs, rhs| lhs.checked_add(rhs))
            .map(|value| value.unwrap_or_else(|| Self::Float64(self.as_f64() + rhs.as_f64())))
    }

    fn subtract(
        self,
        rhs: Self,
        span: SourceSpan,
        interpreter: &Interpreter<'_>,
    ) -> VmResult<Self> {
        self.integer_op(rhs, span, interpreter, |lhs, rhs| lhs.checked_sub(rhs))
            .map(|value| value.unwrap_or_else(|| Self::Float64(self.as_f64() - rhs.as_f64())))
    }

    fn multiply(
        self,
        rhs: Self,
        span: SourceSpan,
        interpreter: &Interpreter<'_>,
    ) -> VmResult<Self> {
        self.integer_op(rhs, span, interpreter, |lhs, rhs| lhs.checked_mul(rhs))
            .map(|value| value.unwrap_or_else(|| Self::Float64(self.as_f64() * rhs.as_f64())))
    }

    fn negate(self, span: SourceSpan, interpreter: &Interpreter<'_>) -> VmResult<Self> {
        match self {
            Self::Integer(value) => value
                .checked_neg()
                .map(Self::Integer)
                .ok_or_else(|| numeric_overflow(interpreter, "integer negation overflowed", span)),
            Self::Float64(value) => Ok(Self::Float64(-value)),
        }
    }

    fn numeric_eq(self, rhs: Self) -> bool {
        match (self, rhs) {
            (Self::Integer(lhs), Self::Integer(rhs)) => lhs == rhs,
            _ => self.as_f64() == rhs.as_f64(),
        }
    }

    fn integer_op(
        self,
        rhs: Self,
        span: SourceSpan,
        interpreter: &Interpreter<'_>,
        operation: impl FnOnce(i64, i64) -> Option<i64>,
    ) -> VmResult<Option<Self>> {
        match (self, rhs) {
            (Self::Integer(lhs), Self::Integer(rhs)) => operation(lhs, rhs)
                .map(Self::Integer)
                .map(Some)
                .ok_or_else(|| {
                    numeric_overflow(interpreter, "integer arithmetic overflowed", span)
                }),
            _ => Ok(None),
        }
    }

    fn as_f64(self) -> f64 {
        match self {
            Self::Integer(value) => value as f64,
            Self::Float64(value) => value,
        }
    }
}

fn numeric_overflow(
    interpreter: &Interpreter<'_>,
    message: &'static str,
    span: SourceSpan,
) -> Box<VmDiagnostic> {
    interpreter.runtime_error(
        "ANVIL_RUNTIME_NUMERIC_OVERFLOW",
        message.to_string(),
        span,
        vec!["exact integer arithmetic without overflow".to_string()],
        Some("overflow".to_string()),
        Some("Use an explicit wider numeric representation when BigInt support lands.".to_string()),
    )
}

fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Nil => "nil",
        Value::Bool(_) => "bool",
        Value::Integer(_) => "integer",
        Value::Float64(_) => "float64",
        Value::String(_) => "string",
        Value::Keyword(_) => "keyword",
        Value::Vector(_) => "vector",
        Value::Map(_) => "map",
        Value::Function(_) => "function",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_value(source: &str) -> Value {
        run_source(source).expect("VM output").value
    }

    fn bytecode(instructions: Vec<Instruction>, constants: Vec<Value>) -> BytecodeProgram {
        bytecode_with_registers_and_bindings(instructions, constants, Vec::new(), 1)
    }

    fn bytecode_with_registers(
        instructions: Vec<Instruction>,
        constants: Vec<Value>,
        register_count: usize,
    ) -> BytecodeProgram {
        bytecode_with_registers_and_bindings(instructions, constants, Vec::new(), register_count)
    }

    fn bytecode_with_bindings(
        instructions: Vec<Instruction>,
        bindings: Vec<String>,
    ) -> BytecodeProgram {
        bytecode_with_registers_and_bindings(instructions, Vec::new(), bindings, 1)
    }

    fn bytecode_with_registers_and_bindings(
        instructions: Vec<Instruction>,
        constants: Vec<Value>,
        bindings: Vec<String>,
        register_count: usize,
    ) -> BytecodeProgram {
        bytecode_with_all(
            instructions,
            constants,
            bindings,
            Vec::new(),
            register_count,
        )
    }

    fn bytecode_with_functions(
        instructions: Vec<Instruction>,
        functions: Vec<FunctionPrototype>,
        register_count: usize,
    ) -> BytecodeProgram {
        bytecode_with_all(
            instructions,
            Vec::new(),
            Vec::new(),
            functions,
            register_count,
        )
    }

    fn bytecode_with_all(
        instructions: Vec<Instruction>,
        constants: Vec<Value>,
        bindings: Vec<String>,
        functions: Vec<FunctionPrototype>,
        register_count: usize,
    ) -> BytecodeProgram {
        let source = SourceText::new("malformed-bytecode", "broken");
        BytecodeProgram {
            version: BYTECODE_VERSION,
            source_id: source.id().to_string(),
            register_count,
            constants,
            bindings,
            functions,
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
    fn runs_definitions_symbol_lookup_and_bootstrap_primitives() {
        assert_eq!(
            run_value(
                r#"
                (define answer (+ 40 2))
                answer
                "#,
            ),
            Value::Integer(42)
        );
        assert_eq!(run_value("(do (define x 7) (* x 6))"), Value::Integer(42));
        assert_eq!(run_value("(+ 1 2.5)"), Value::Float64(3.5));
        assert_eq!(run_value("(- 10 3 2)"), Value::Integer(5));
        assert_eq!(run_value("(- 3)"), Value::Integer(-3));
        assert_eq!(run_value("(- 3.5)"), Value::Float64(-3.5));
        assert_eq!(run_value("(= 1 1)"), Value::Bool(true));
        assert_eq!(run_value("(= 1 1.0)"), Value::Bool(true));
        assert_eq!(run_value("(= 0 0.0 -0.0)"), Value::Bool(true));
    }

    #[test]
    fn runs_named_function_values_with_call_frames() {
        assert_eq!(
            run_value(
                r#"
                (define add (fn [x y] (+ x y)))
                (add 40 2)
                "#,
            ),
            Value::Integer(42)
        );
    }

    #[test]
    fn runs_direct_function_literals_and_multiexpression_bodies() {
        assert_eq!(run_value("((fn [x] (* x x)) 6)"), Value::Integer(36));
        assert_eq!(
            run_value("((fn [x] (+ x 1) (* x 2)) 21)"),
            Value::Integer(42)
        );
    }

    #[test]
    fn function_parameters_shadow_top_level_bindings() {
        assert_eq!(
            run_value("(define x 100) ((fn [x] (+ x 1)) 41)"),
            Value::Integer(42)
        );
    }

    #[test]
    fn function_bodies_can_read_top_level_bindings() {
        assert_eq!(
            run_value("(define y 2) ((fn [x] (+ x y)) 40)"),
            Value::Integer(42)
        );
    }

    #[test]
    fn function_calls_return_composite_values() {
        assert_eq!(
            run_value("((fn [x] [x (+ x 1)]) 1)"),
            Value::Vector(vec![Value::Integer(1), Value::Integer(2)])
        );
    }

    #[test]
    fn returned_closures_capture_lexical_locals() {
        assert_eq!(
            run_value(
                r#"
                (define make-adder (fn [x] (fn [y] (+ x y))))
                (define add40 (make-adder 40))
                (add40 2)
                "#,
            ),
            Value::Integer(42)
        );
    }

    #[test]
    fn nested_closures_capture_transitive_lexical_locals() {
        assert_eq!(
            run_value(
                r#"
                (define make-nested
                  (fn [x]
                    (fn [y]
                      (fn [z] (+ x (+ y z))))))
                (define add3 ((make-nested 1) 2))
                (add3 39)
                "#,
            ),
            Value::Integer(42)
        );
    }

    #[test]
    fn closure_parameters_shadow_captured_locals() {
        assert_eq!(
            run_value("((fn [x] ((fn [x] (+ x 1)) 41)) 100)"),
            Value::Integer(42)
        );
    }

    #[test]
    fn closures_prefer_captured_locals_over_later_top_level_bindings() {
        assert_eq!(
            run_value(
                r#"
                (define make (fn [x] (fn [] x)))
                (define f (make 42))
                (define x 7)
                (f)
                "#,
            ),
            Value::Integer(42)
        );
    }

    #[test]
    fn tail_recursive_calls_replace_the_current_frame() {
        let program = compile_source(
            r#"
            (define loop
              (fn [n acc]
                (if (= n 0)
                  acc
                  (loop (- n 1) (+ acc 1)))))
            (loop 1000 0)
            "#,
        )
        .expect("bytecode");
        let output = Vm::with_budget(VmBudget::with_instruction_fuel(50_000))
            .run(&program)
            .expect("VM output");

        assert_eq!(output.value, Value::Integer(1000));
        assert_eq!(output.max_call_depth, 2);
        assert!(output.instructions_executed > 1000);
    }

    #[test]
    fn mutual_tail_recursion_reuses_the_active_function_frame() {
        let output = run_source(
            r#"
            (define even (fn [n] (if (= n 0) true (odd (- n 1)))))
            (define odd (fn [n] (if (= n 0) false (even (- n 1)))))
            (even 999)
            "#,
        )
        .expect("VM output");

        assert_eq!(output.value, Value::Bool(false));
        assert_eq!(output.max_call_depth, 2);
    }

    #[test]
    fn tail_calls_can_target_captured_closure_locals() {
        let output = run_source(
            r#"
            (define make-forwarder (fn [f] (fn [x] (f x))))
            (define id (fn [x] x))
            ((make-forwarder id) 42)
            "#,
        )
        .expect("VM output");

        assert_eq!(output.value, Value::Integer(42));
        assert_eq!(output.max_call_depth, 2);
    }

    #[test]
    fn empty_tail_do_returns_nil() {
        let output = run_source(
            r#"
            (define nothing (fn [] (do)))
            (nothing)
            "#,
        )
        .expect("VM output");

        assert_eq!(output.value, Value::Nil);
        assert_eq!(output.max_call_depth, 2);
    }

    #[test]
    fn tail_do_preserves_prefix_work_and_replaces_on_final_call() {
        let output = run_source(
            r#"
            (define id (fn [x] x))
            (define final (fn [x] (do (+ x 1) (id x))))
            (final 42)
            "#,
        )
        .expect("VM output");

        assert_eq!(output.value, Value::Integer(42));
        assert_eq!(output.max_call_depth, 2);
    }

    #[test]
    fn tail_if_true_branch_replaces_the_current_frame() {
        let output = run_source(
            r#"
            (define id (fn [x] x))
            (define choose (fn [flag] (if flag (id 42) 0)))
            (choose true)
            "#,
        )
        .expect("VM output");

        assert_eq!(output.value, Value::Integer(42));
        assert_eq!(output.max_call_depth, 2);
    }

    #[test]
    fn tail_calls_direct_function_literals() {
        let output = run_source(
            r#"
            (define outer (fn [] ((fn [] 42))))
            (outer)
            "#,
        )
        .expect("VM output");

        assert_eq!(output.value, Value::Integer(42));
        assert_eq!(output.max_call_depth, 2);
    }

    #[test]
    fn non_tail_calls_still_push_frames() {
        let output = run_source(
            r#"
            (define id (fn [x] x))
            (define wrap (fn [x] (+ (id x) 1)))
            (wrap 41)
            "#,
        )
        .expect("VM output");

        assert_eq!(output.value, Value::Integer(42));
        assert_eq!(output.max_call_depth, 3);
    }

    #[test]
    fn compiles_tail_positions_to_tail_call_bytecode() {
        let program = compile_source(
            r#"
            (define f (fn [x] (do (+ x 1) (if x ((fn [] 1)) ((fn [] 2))))))
            (f true)
            "#,
        )
        .expect("bytecode");

        let tail_calls = program
            .functions
            .iter()
            .flat_map(|function| &function.instructions)
            .filter(|instruction| {
                matches!(
                    instruction.instruction,
                    Instruction::TailCallFunction { .. }
                )
            })
            .count();

        assert_eq!(tail_calls, 2);
    }

    #[test]
    fn displays_all_value_forms_with_escaping() {
        let value = Value::Vector(vec![
            Value::Nil,
            Value::Bool(true),
            Value::Integer(7),
            Value::Float64(2.5),
            Value::String("a\\b\"c\n\r\t".into()),
            Value::Keyword("ready".into()),
            Value::Map(vec![ValueMapEntry {
                key: Value::Keyword("nested".into()),
                value: Value::Bool(false),
            }]),
            Value::Function(FunctionValue::new(7)),
        ]);

        assert_eq!(
            value.to_string(),
            "[nil true 7 2.5 \"a\\\\b\\\"c\\n\\r\\t\" :ready {:nested false} #<fn:7>]"
        );

        let mut captures = BTreeMap::new();
        captures.insert("x".to_string(), Value::Integer(42));
        assert_eq!(
            Value::Function(FunctionValue::with_captures(3, captures)).to_string(),
            "#<fn:3>"
        );
    }

    #[test]
    fn default_vm_runs_bytecode_programs() {
        let program = compile_source("42").expect("bytecode");
        let output = Vm::default().run(&program).expect("VM output");

        assert_eq!(output.value, Value::Integer(42));
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
        let diagnostic =
            compile_source("(require planner.search)").expect_err("compile diagnostic");

        assert_eq!(diagnostic.code, "ANVIL_COMPILE_UNSUPPORTED_FORM");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Compile);
        assert_eq!(diagnostic.primary_span.start.column, 1);
    }

    #[test]
    fn reports_unbound_symbols_as_runtime_diagnostics() {
        let diagnostic = run_source("answer").expect_err("runtime diagnostic");

        assert_eq!(diagnostic.code, "ANVIL_RUNTIME_UNBOUND_SYMBOL");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Runtime);
        assert_eq!(diagnostic.actual.as_deref(), Some("symbol answer"));
    }

    #[test]
    fn reports_function_call_diagnostics() {
        let not_callable = run_source("(42)").expect_err("runtime diagnostic");
        assert_eq!(not_callable.code, "ANVIL_RUNTIME_NOT_CALLABLE");
        assert_eq!(not_callable.phase, DiagnosticPhase::Runtime);
        assert_eq!(not_callable.actual.as_deref(), Some("integer"));

        let arity = run_source("((fn [x] x))").expect_err("runtime diagnostic");
        assert_eq!(arity.code, "ANVIL_RUNTIME_ARITY");
        assert_eq!(arity.phase, DiagnosticPhase::Runtime);
        assert_eq!(arity.actual.as_deref(), Some("0 argument(s)"));

        let unbound_callee = run_source("(unknown 1)").expect_err("runtime diagnostic");
        assert_eq!(unbound_callee.code, "ANVIL_RUNTIME_UNBOUND_SYMBOL");
        assert_eq!(unbound_callee.phase, DiagnosticPhase::Runtime);
    }

    #[test]
    fn reports_non_callable_value_types() {
        for (source, actual) in [
            ("(nil)", "nil"),
            ("(false)", "bool"),
            ("(1.5)", "float64"),
            ("(\"agent\")", "string"),
            ("(:ready)", "keyword"),
            ("([1])", "vector"),
            ("({:ready true})", "map"),
        ] {
            let diagnostic = run_source(source).expect_err("runtime diagnostic");
            assert_eq!(diagnostic.code, "ANVIL_RUNTIME_NOT_CALLABLE");
            assert_eq!(diagnostic.actual.as_deref(), Some(actual));
        }
    }

    #[test]
    fn reports_bootstrap_primitive_type_and_arity_errors() {
        let type_diagnostic = run_source("(+ 1 \"agent\")").expect_err("runtime diagnostic");
        assert_eq!(type_diagnostic.code, "ANVIL_RUNTIME_TYPE_ERROR");
        assert_eq!(type_diagnostic.phase, DiagnosticPhase::Runtime);
        assert_eq!(
            type_diagnostic.actual.as_deref(),
            Some("argument 2 was string")
        );

        let arity_diagnostic = run_source("(-)").expect_err("runtime diagnostic");
        assert_eq!(arity_diagnostic.code, "ANVIL_RUNTIME_ARITY");
        assert_eq!(arity_diagnostic.phase, DiagnosticPhase::Runtime);
    }

    #[test]
    fn runs_empty_do_and_zero_argument_numeric_equality() {
        assert_eq!(run_value("(do)"), Value::Nil);
        assert_eq!(run_value("(=)"), Value::Bool(true));
    }

    #[test]
    fn reports_exact_integer_overflow() {
        let diagnostic = run_source("(+ 9223372036854775807 1)").expect_err("runtime diagnostic");

        assert_eq!(diagnostic.code, "ANVIL_RUNTIME_NUMERIC_OVERFLOW");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Runtime);
    }

    #[test]
    fn reports_exact_integer_negation_overflow() {
        let diagnostic = run_source("(- -9223372036854775808)").expect_err("runtime diagnostic");

        assert_eq!(diagnostic.code, "ANVIL_RUNTIME_NUMERIC_OVERFLOW");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Runtime);
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
        assert_eq!(output.max_call_depth, 1);
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

    #[test]
    fn malformed_bytecode_reports_binding_out_of_bounds() {
        assert_eq!(
            runtime_code(bytecode(
                vec![Instruction::LoadBinding { dst: 0, binding: 9 }],
                Vec::new(),
            )),
            "ANVIL_RUNTIME_BINDING_OUT_OF_BOUNDS"
        );
    }

    #[test]
    fn malformed_bytecode_reports_function_out_of_bounds() {
        assert_eq!(
            runtime_code(bytecode_with_functions(
                vec![Instruction::LoadFunction {
                    dst: 0,
                    function: 9,
                }],
                Vec::new(),
                1,
            )),
            "ANVIL_RUNTIME_FUNCTION_OUT_OF_BOUNDS"
        );
    }

    #[test]
    fn malformed_bytecode_reports_function_value_out_of_bounds() {
        assert_eq!(
            runtime_code(bytecode_with_registers(
                vec![
                    Instruction::LoadConstant {
                        dst: 0,
                        constant: 0,
                    },
                    Instruction::CallFunction {
                        dst: 0,
                        callee: 0,
                        args: Vec::new(),
                    },
                ],
                vec![Value::Function(FunctionValue::new(9))],
                1,
            )),
            "ANVIL_RUNTIME_FUNCTION_OUT_OF_BOUNDS"
        );
    }

    #[test]
    fn malformed_bytecode_reports_tail_call_outside_function() {
        assert_eq!(
            runtime_code(bytecode(
                vec![Instruction::TailCallFunction {
                    callee: 0,
                    args: Vec::new(),
                }],
                Vec::new(),
            )),
            "ANVIL_RUNTIME_TAIL_CALL_OUTSIDE_FUNCTION"
        );
    }

    #[test]
    fn tail_call_diagnostics_match_function_calls() {
        let not_callable = run_source("((fn [] (42)))").expect_err("runtime diagnostic");
        assert_eq!(not_callable.code, "ANVIL_RUNTIME_NOT_CALLABLE");
        assert_eq!(not_callable.actual.as_deref(), Some("integer"));

        let arity = run_source("((fn [f] (f)) (fn [x] x))").expect_err("runtime diagnostic");
        assert_eq!(arity.code, "ANVIL_RUNTIME_ARITY");
        assert_eq!(arity.actual.as_deref(), Some("0 argument(s)"));
    }

    #[test]
    fn malformed_bytecode_reports_unknown_primitives() {
        assert_eq!(
            runtime_code(bytecode_with_bindings(
                vec![Instruction::CallPrimitive {
                    dst: 0,
                    primitive: 0,
                    args: Vec::new(),
                }],
                vec!["future".to_string()],
            )),
            "ANVIL_RUNTIME_UNKNOWN_PRIMITIVE"
        );
    }
}
