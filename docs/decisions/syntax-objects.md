# Syntax Objects

Canonical planning note:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/syntax-objects.md`

Implementation-facing decision:

- Syntax objects are the span-preserving bridge between reader datums, macro
  expansion, modules, and later AST lowering.
- A syntax object currently contains a deterministic id, source id, original
  spanned datum, top-level span, and syntax context.
- Initial syntax ids are source-local and deterministic: `repl:1`, `repl:2`,
  and so on.
- The initial syntax context is intentionally empty, with `scopes` and `marks`
  fields reserved for hygiene and expansion metadata.
- Reader diagnostics pass through the syntax-object layer unchanged. Syntax
  object wrapping should not hide reader errors or convert them into a new
  diagnostic family.
- This slice does not implement hygienic macro expansion, module scopes,
  compiler macros, reader macros, or source-map chains. It creates the data
  shape those later features can enrich.

Current executable surface:

- Core API: `syntax_from_source`, `syntax_from_source_text`,
  `syntax_from_datums`, and `format_syntax_objects`.
- CLI: `anvil-cli syntax [--json] [SOURCE]`.
- Gherkin: `specs/syntax_objects.feature`.
