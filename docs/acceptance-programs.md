# Acceptance Programs

The first acceptance programs should prove that Anvil is useful for the actual
target domain before implementation taste hardens into language design.

Initial candidates:

- Small functional Lisp programs with tail recursion, closures, locals,
  conditionals, lists, vectors, maps, and structured errors.
- Agent-authored module requiring another module, with deterministic module
  resolution and source-span diagnostics.
- MarkoDB-style fact/rule/query/explain program.
- Hard and soft type membership example using QBBN/VSA evidence without letting
  soft evidence silently satisfy hard type requirements.
- Lightweight `pmap` over a collection with cancellation and bounded scheduler
  resources.
- Actor with mailbox, supervised restart, and REPL inspection.
- Capability denial example for file, network, debug attach, and resource
  handle use.
- Staged module replacement: draft, compile, test, approve, activate, rollback.
- WASM profile example with explicit host imports.
- Tiny tensor/resource example once the value/resource model is coherent.

The exact matrix should live here once the syntax, GC, resource-handle, and
capability decisions are more concrete.

## Executable Specs

The first executable acceptance harness is intentionally tiny but real:

- Gherkin feature files live under `specs/`.
- Rust step definitions live in the `anvil-acceptance` crate.
- Agents can run the suite with
  `cargo test -p anvil-acceptance --test acceptance`.
- `reader_repl.feature` covers the first REPL-visible reader behavior,
  Clojure-like delimiters, multiline interactive input, JSON pending events,
  JSON-serializable diagnostics, and source-aware diagnostic rendering.
- `ast_lowering.feature` covers the first syntax layer behavior: lowering
  definition and function forms, serializing AST JSON, and returning
  syntax-phase diagnostics.

This keeps acceptance behavior cargo-shaped while leaving room for richer
spec linting, coverage mapping, eval artifacts, and agent-readable reports as
the language surface becomes executable.
