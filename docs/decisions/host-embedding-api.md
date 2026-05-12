# Host Embedding API

The canonical planning note is:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/host-embedding-api.md`

Implementation-facing decision:

- Anvil is embedded-first.
- Rust is the authoritative host API.
- Rust hosts can register functions, resources, modules, async calls, streams,
  actors/services, profiles, capabilities, budgets, event topics, and device
  backends.
- A stable runtime facade should sit on top of the Rust API so TypeScript and
  other environments can drive the runtime without accessing internals.
- The facade should expose eval, call, draft compile/test/activation request,
  inspect, attach/debug, subscribe/stream, resource calls, actor messages,
  facet lookup, capability/profile inspection, and cancellation.
- Resource registration and facade-visible handles follow
  `docs/decisions/resource-handles.md`: handles are typed, opaque,
  supervisor-issued, use-site checked, narrowed on delegation, and never raw
  Rust object access.
- Resource adapter execution follows
  `docs/decisions/resource-adapter-execution.md`: runtime checks happen before
  adapter calls, and adapter outcomes/failures return through structured
  resource envelopes.
- The first executable host-function slice is synchronous and explicit:
  `HostFunctionRegistry` stores registered Rust callbacks by name,
  `HostFunctionSpec` declares arity plus optional required capability and trust
  zone, and `VmSession`/`ModuleSession` expose registration methods.
- Direct calls to registered host-function names compile to a dedicated
  bytecode instruction. Arity, trust-zone, and capability checks happen before
  invoking Rust host code. Denials and host callback failures become structured
  VM runtime diagnostics rather than host-language transport errors.
- Host functions are not ordinary first-class Anvil values yet. The current
  contract is a direct-call import surface for embedded runtimes; first-class
  handles, async calls, streams, actor-backed services, and typed signatures are
  later host-facade work.
- The facade should not expose GC objects, scheduler internals, raw frames, raw
  Rust pointers, or unmediated host resources.
- Ordinary language failures, denials, approvals, timeouts, and budget
  exhaustion return the structured Anvil response envelope. Transport failures
  and runtime corruption are host-language errors.

Open implementation dependency: value serialization, typed host signatures,
async/streaming host runners, and the response/facet retention model need
concrete Rust types before transport adapters can be built.
