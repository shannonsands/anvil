# Bytecode VM Foundation

Canonical planning note:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/bytecode-vm-foundation.md`

Implementation-facing decision:

- The first executable VM slice is register-based bytecode, not a temporary
  stack evaluator.
- Each bytecode instruction carries a source span so compile and runtime
  diagnostics can point back to the originating form.
- Execution is an explicit program-counter loop over a register file. Language
  execution must not use Rust recursion for runtime control flow. Function calls
  now run through explicit VM call frames.
- The bootstrap value representation is an owned immutable `Value` enum for
  nil, booleans, integers, `Float64`, strings, keywords, vectors, and ordered
  maps, plus lightweight function references. This is a bootstrap surface, not
  the final heap layout. The final direction is covered by
  `docs/decisions/value-heap-gc.md`.
- `false` and `nil` are falsey for branch tests; all other values are truthy.
- The first compiler supports top-level expression sequences, literals,
  vectors, ordered maps, `do`, `if`, top-level `define`, symbol lookup, `fn`
  values, user-defined function calls, and bootstrap primitive calls.
- The initial binding model is intentionally top-level and per-program:
  `define` writes a VM binding table, later expressions in the same program can
  read it, and a fresh `run_source` call starts with no user bindings. Function
  parameters are lexical locals on the active call frame and shadow top-level
  bindings.
- The initial primitive table is limited to checked numeric `+`, `-`, `*`, and
  `=` over `Integer` and `Float64`, with exact integer overflow reported as a
  runtime diagnostic.
- Empty top-level input and empty `do` evaluate to `nil`.
- Unbound symbols now fail at runtime with `ANVIL_RUNTIME_UNBOUND_SYMBOL`,
  preserving the source span of the symbol read.
- Calling a non-function value fails at runtime with
  `ANVIL_RUNTIME_NOT_CALLABLE`; wrong function arity fails with
  `ANVIL_RUNTIME_ARITY`.
- Unsupported executable forms such as `require` and quote still fail during
  compilation with `phase: compile` diagnostics.
- Runtime diagnostics use `phase: runtime` and preserve the current instruction
  span. The first runtime budget is instruction fuel.

Non-goals for this slice:

- Captured closure environments, local `define` semantics, proper tail calls,
  host calls, modules at execution time, resource handles, heap GC, actors, and
  debugger attachment.
- Final scalar numeric representation. The bootstrap integer remains `i64`
  because exact `BigInt`/`Ratio` implementation is covered by the numeric
  semantics decision and will land after the basic VM loop is real.

Open follow-up decisions:

- Concrete persistent collection layout and root-table implementation details
  inside the tracing-GC direction.
- Captured closure representation and tail-call opcode details.
- Module bytecode cache serialization and bytecode versioning policy.
