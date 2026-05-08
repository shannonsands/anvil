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
  maps. This is a bootstrap surface, not the final heap layout.
- `false` and `nil` are falsey for branch tests; all other values are truthy.
- The first compiler supports top-level expression sequences, literals,
  vectors, ordered maps, `do`, and `if`.
- Empty top-level input and empty `do` evaluate to `nil`.
- Unsupported executable forms such as unresolved symbols, `define`, `fn`,
  calls, `require`, and quote currently fail during compilation with
  `phase: compile` diagnostics.
- Runtime diagnostics use `phase: runtime` and preserve the current instruction
  span. The first runtime budget is instruction fuel.

Non-goals for this slice:

- Closures, locals, globals, calls, proper tail calls, host calls, modules at
  execution time, resource handles, heap GC, actors, and debugger attachment.
- Final scalar numeric representation. The bootstrap integer remains `i64`
  because exact `BigInt`/`Ratio` implementation is covered by the numeric
  semantics decision and will land after the basic VM loop is real.

Open follow-up decisions:

- Full value representation, heap layout, tracing GC, persistent collection
  layout, and host/resource handle rooting.
- Closure/call-frame representation and tail-call opcode details.
- Module bytecode cache serialization and bytecode versioning policy.
