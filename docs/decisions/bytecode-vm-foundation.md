# Bytecode VM Foundation

Canonical planning note:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/bytecode-vm-foundation.md`

Implementation-facing decision:

- The first executable VM slice is register-based bytecode, not a temporary
  stack evaluator.
- Each bytecode instruction carries a source span so compile and runtime
  diagnostics can point back to the originating form.
- Execution is an explicit program-counter loop over a register file. Language
  execution must not use Rust recursion for runtime control flow.
- The bootstrap value representation is an owned immutable `Value` enum for
  nil, booleans, integers, `Float64`, strings, keywords, vectors, and ordered
  maps. This is a bootstrap surface, not the final heap layout. The final
  direction is covered by `docs/decisions/value-heap-gc.md`.
- `false` and `nil` are falsey for branch tests; all other values are truthy.
- The first compiler supports top-level expression sequences, literals,
  vectors, ordered maps, `do`, `if`, top-level `define`, symbol lookup, and
  bootstrap primitive calls.
- The initial binding model is intentionally top-level and per-program:
  `define` writes a VM binding table, later expressions in the same program can
  read it, and a fresh `run_source` call starts with no user bindings.
- The initial primitive table is limited to checked numeric `+`, `-`, `*`, and
  `=` over `Integer` and `Float64`, with exact integer overflow reported as a
  runtime diagnostic.
- Empty top-level input and empty `do` evaluate to `nil`.
- Unbound symbols now fail at runtime with `ANVIL_RUNTIME_UNBOUND_SYMBOL`,
  preserving the source span of the symbol read.
- Unsupported executable forms such as `fn`, non-primitive calls, `require`,
  and quote still fail during compilation with `phase: compile` diagnostics.
- Runtime diagnostics use `phase: runtime` and preserve the current instruction
  span. The first runtime budget is instruction fuel.

Non-goals for this slice:

- Closures, lexical locals, user-defined function calls, proper tail calls,
  host calls, modules at execution time, resource handles, heap GC, actors, and
  debugger attachment.
- Final scalar numeric representation. The bootstrap integer remains `i64`
  because exact `BigInt`/`Ratio` implementation is covered by the numeric
  semantics decision and will land after the basic VM loop is real.

Open follow-up decisions:

- Concrete persistent collection layout and root-table implementation details
  inside the tracing-GC direction.
- Closure/call-frame representation and tail-call opcode details.
- Module bytecode cache serialization and bytecode versioning policy.
