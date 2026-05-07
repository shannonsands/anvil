# REPL-First Implementation Sequence

Canonical planning note:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/repl-first-implementation-sequence.md`

Implementation-facing decision:

- Start implementation with the agent-facing REPL loop, but make the first REPL
  reader-backed rather than evaluator-backed.
- The first REPL accepts source text, runs the lexer and datum reader, prints
  parsed datums, and reports structured diagnostics with spans.
- Interactive REPL sessions keep reading when the current input is incomplete,
  while batch `read` keeps reporting incomplete input as a structured reader
  diagnostic.
- In JSON mode, interactive REPL sessions emit explicit `pending` events for
  incomplete input so agents can distinguish continuation from a stalled
  runtime.
- The VM remains the reference execution path, but it should come after the
  language has a concrete reader, spans, diagnostics, acceptance specs, and
  agent-visible feedback loop.
- Parser/reader diagnostics should be designed as the first instance of the
  broader agent protocol: concise by default, structured enough to drive tests
  and future REPL facets.
- Acceptance specs should describe REPL-visible behavior from the beginning.

Concrete order:

1. Reader-backed REPL: source input, datum output, multiline input collection,
   JSON pending/read/error events, structured reader errors.
2. Lexer with spans: parentheses, brackets, braces, strings, comments, symbols,
   keywords, numeric atoms, quote sugar, and source locations.
3. Datum reader: lists, vectors, maps, strings, booleans, nil, symbols,
   keywords, integers, floats, and quote forms.
4. Pretty printer and round-trip tests for the first syntax slice.
5. Core AST lowering for a tiny language subset.
6. Bytecode IR and source maps.
7. Minimal VM over the core subset.
8. Modules, require, bytecode cache, and draft overlays.
9. Macro expansion and syntax objects.
10. Capabilities, host API, and resource handles.
11. Runtime attach, debugger, actors, and live process inspection.

Non-goal for the first REPL: evaluating code. Early output should be honest
about being read-only until AST lowering and bytecode execution exist.
