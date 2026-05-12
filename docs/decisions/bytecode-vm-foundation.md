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
  nil, booleans, integers, `Float64`, strings, symbols, keywords, lists,
  vectors, and ordered maps, plus function closure values with owned lexical
  captures. This is a bootstrap surface, not the final heap layout. The final
  direction is covered by `docs/decisions/value-heap-gc.md`.
- `false` and `nil` are falsey for branch tests; all other values are truthy.
- The first compiler supports top-level expression sequences, literals,
  quote-as-data, vectors, ordered maps, `do`, `if`, sequential lexical
  `let`/`let*`, top-level `define`, symbol lookup, `fn` closure values,
  user-defined function calls, and bootstrap primitive calls.
- Quote compiles to a constant immutable data value. Quoted symbols become
  symbol values, quoted lists become list values, quoted vectors/maps preserve
  their collection shape, and nested quote sugar is represented as list data
  shaped like `(quote value)`. Quoted lists are never compiled as calls.
- The standalone binding model remains top-level and per-program for fresh
  `Vm::run`/`run_source` calls: `define` writes a VM binding table and later
  expressions in the same program can read it. Stateful REPL/host evaluation
  uses `VmSession`, which carries top-level bindings, binding names, and
  function prototypes across successful evaluations. Function parameters are
  lexical locals on the active call frame and shadow captured locals and
  top-level bindings.
- Function values now carry an owned lexical capture map. Loading a function in
  a non-top-level frame captures the current frame locals, including any
  transitive outer captures already visible to that frame. Calling a closure
  seeds the new frame with those captures and then binds parameters over the top,
  so parameters shadow captured locals. Top-level bindings remain dynamic
  per-program globals rather than copied closure captures.
- Sequential lexical `let`/`let*` bindings use a Clojure-shaped binding vector:
  `(let [name value ...] body...)`. Each binding initializer can see earlier
  bindings from the same vector. The VM emits explicit lexical-scope
  push/bind/pop bytecode so local bindings can shadow captured locals,
  parameters, or top-level bindings without leaking after the body exits.
- Proper tail calls are implemented by compiling tail-position user function
  calls to explicit tail-call bytecode. The interpreter replaces the active
  function frame with the callee frame instead of pushing a new one, while
  preserving the caller's return register. Tail-recursive and mutually
  tail-recursive programs therefore run at constant VM call depth.
- Registered synchronous host functions compile to explicit host-call bytecode
  when a direct call target matches the session's `HostFunctionRegistry`.
  Arity, trust-zone, and capability-profile checks run before Rust host code is
  invoked; denials and callback failures return runtime diagnostics.
- The initial primitive table is limited to checked numeric `+`, `-`, `*`, and
  `=` over `Integer` and `Float64`, with exact integer overflow reported as a
  runtime diagnostic.
- Empty top-level input and empty `do` evaluate to `nil`.
- Unbound symbols now fail at runtime with `ANVIL_RUNTIME_UNBOUND_SYMBOL`,
  preserving the source span of the symbol read.
- Calling a non-function value fails at runtime with
  `ANVIL_RUNTIME_NOT_CALLABLE`; wrong function arity fails with
  `ANVIL_RUNTIME_ARITY`.
- Unsupported executable forms still fail during standalone VM compilation with
  `phase: compile` diagnostics. `require` is executable only through the
  module-aware `ModuleSession` wrapper, which resolves and loads top-level
  require prefixes before handing ordinary forms to `VmSession`.
- Runtime diagnostics use `phase: runtime` and preserve the current instruction
  span. The first runtime budget is instruction fuel.
- `VmOutput` includes `max_call_depth` as an initial execution metric so tests,
  agents, and hosts can distinguish real tail-call frame replacement from
  merely returning the right value.
- `VmSession` commits updated bindings and function/compiler tables only after
  a successful top-level return. Runtime or compile failures keep the prior
  session state alive, including after instruction-fuel exhaustion.

Non-goals for this slice:

- Local `define` semantics, first-class host-function values, async/streaming
  host calls, module namespaces/generations, heap GC, actors, and debugger
  attachment.
- Final scalar numeric representation. The bootstrap integer remains `i64`
  because exact `BigInt`/`Ratio` implementation is covered by the numeric
  semantics decision and will land after the basic VM loop is real.

Open follow-up decisions:

- Concrete persistent collection layout and root-table implementation details
  inside the tracing-GC direction.
- Tail-call interaction with local `define`, debug frame inspection, and module
  generation replacement.
- Module bytecode cache serialization and bytecode versioning policy.
