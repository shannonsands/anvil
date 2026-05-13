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
- `HostFunctionSpec` now carries optional typed signature metadata through
  `HostFunctionSignature`, `HostParameterSpec`, `HostResultSpec`, and
  `HostValueType`. Runtime arity checks remain authoritative in the first
  slice; signatures are inspectable contract metadata for agents, docs, and
  future transport bindings.
- Direct calls to registered host-function names compile to a dedicated
  bytecode instruction. Arity, trust-zone, and capability checks happen before
  invoking Rust host code. Denials and host callback failures become structured
  VM runtime diagnostics rather than host-language transport errors.
- `EmbeddedRuntime` is the first Rust runtime facade. It wraps a
  `ModuleSession`, owns a resource registry and handle table, stores registered
  capability profiles in a `CapabilityPolicy`, composes profile fragments,
  activates a profile onto the VM/module session, evaluates source through
  `EvalResponse`, opens resources under the active profile when one exists, and
  emits `EmbeddedRuntimeSnapshot` metadata with `protocol:
  anvil.embedding.v1`.
- `EmbeddedRuntimeSnapshot` includes the first facade-visible runtime audit log:
  profile composition, profile activation, host-authority denials during eval,
  and resource-open allow/deny events. This is in-memory and inspectable; durable
  audit sinks and response-facet retention remain later work.
- Host functions are not ordinary first-class Anvil values yet. The current
  contract is a direct-call import surface for embedded runtimes; first-class
  function/resource values, async calls, streams, actor-backed services, and
  transport adapters are later host-facade work.
- The facade should not expose GC objects, scheduler internals, raw frames, raw
  Rust pointers, or unmediated host resources.
- Ordinary language failures, denials, approvals, timeouts, and budget
  exhaustion return the structured Anvil response envelope. Transport failures
  and runtime corruption are host-language errors.
- The first concrete envelope is `EvalResponse` from `anvil-core::response`.
  It is now used by VM/module session response helpers and CLI `run --json` /
  VM-backed `repl --json`. Host function denials and callback failures flow
  through this envelope as VM runtime diagnostics when callers use the response
  helpers.

## Current Core Boundary

Embedding can move ahead without waiting for the rest of the language kernel.
The core pieces that still need dedicated implementation are:

- Language-level sandbox/process objects: private stacks, mailboxes, roots,
  task state, cancellation, and supervisor ownership.
- Capability policy beyond in-memory same-principal composition: roles/groups,
  manifest policy, approvals, durable audit sinks, revocation propagation, and
  persistent policy storage.
- Runtime concurrency forms and scheduler: lightweight tasks, actors, atoms,
  PubSub, hooks, watchers, process supervision, and bounded `pmap`.
- GC/root integration: tracing heap, process roots, actor roots, handle-table
  roots, debug-safe inspection, and cycle-heavy value tests.
- Debug/attach protocol: breakpoints, frame inspection, stack-safe debug eval,
  effect barriers, rewind/fork/replay, and authority checks for every debug
  operation.
- Module generation and staged replacement: dynamic require, aliases/refer,
  exports, bytecode cache invalidation, draft activation, rollback, and live
  state migration.

Open implementation dependency: async/streaming host runners, actor-backed
services, result ids, response/facet retention, durable audit sinks, and
TypeScript/WASM transport adapters need concrete Rust types on top of
`EmbeddedRuntime` before bindings can be built.
