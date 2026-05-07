# Core AST Lowering

Canonical planning note:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/core-ast-lowering.md`

Implementation-facing decision:

- Core AST lowering is the first M2 implementation slice after the
  reader-backed REPL.
- The AST is source-aware: every lowered expression preserves the span of the
  datum it came from.
- Syntax diagnostics reuse the shared agent diagnostic envelope, with
  `phase: syntax`, source id, primary span, labels, expected/actual values,
  suggestions, and code-frame rendering.
- The initial AST subset is deliberately small: literals, symbols, quote,
  `define`, `if`, `do`, `fn`/`lambda`, calls, vectors, and maps.
- Lists are forms or calls. Empty lists are rejected for now with a structured
  syntax diagnostic rather than silently becoming `nil`.
- Function parameters use vector syntax, for example `(fn [x y] (+ x y))`.
  Parameter names must be unique symbols.
- This AST is not yet the macro expansion representation. Syntax objects and
  macro expansion remain later M2 work.

Current executable surface:

- Core API: `lower_source`, `lower_source_text`, `lower_datums`, and
  `format_ast`.
- CLI: `anvil-cli ast [--json] [SOURCE]`.
- Gherkin: `specs/ast_lowering.feature`.
