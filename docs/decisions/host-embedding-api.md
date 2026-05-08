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
- The facade should not expose GC objects, scheduler internals, raw frames, raw
  Rust pointers, or unmediated host resources.
- Ordinary language failures, denials, approvals, timeouts, and budget
  exhaustion return the structured Anvil response envelope. Transport failures
  and runtime corruption are host-language errors.

Open implementation dependency: value serialization and the response/facet
retention model need concrete Rust types before transport adapters can be built.
