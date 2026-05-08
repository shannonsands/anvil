# Value, Heap, And GC

Canonical planning note:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/value-heap-gc.md`

Implementation-facing decision:

- Ordinary Anvil language values are heap-managed and immutable by default.
- The primary memory-management model is precise tracing GC, not reference
  counting. The runtime is expected to contain long-lived graphs with cycles:
  closures, modules, actors, watchers, hooks, runtime tables, debug handles, and
  host-call state.
- The first implementation should use a simple stop-the-world tracing
  collector with explicit safe points. It should be precise, non-moving, and
  handle-addressed before attempting generational, incremental, concurrent, or
  compacting collection.
- Non-moving collection is an implementation starting point, not a permanent
  performance promise. Stable object ids and opaque handles are more important
  than early heap cleverness because they simplify debugger attachment, source
  mapping, host embedding, WASM boundaries, resource rooting, and VM tests.
- The public Rust value API must use opaque `ValueRef`/handle-like references
  rather than exposing raw object addresses. This keeps room for later moving
  or compacting collectors.

## Value Families

The concrete in-memory representation can evolve, but the semantic families
are locked:

- Immediate values: `nil`, booleans, compact exact integers where possible, and
  small interned ids where useful.
- Heap scalar values: `BigInt`, `Ratio`, complex numbers when they cannot be
  represented inline, strings, symbols, keywords, and other interned or
  structured scalar values.
- Heap structural values: lists, vectors, maps, sets, records, closures,
  syntax objects, bytecode constants that are true language values, errors,
  diagnostics, and persistent collection nodes.
- Controlled mutable values: atoms, actor state cells, runtime table handles,
  transient builders, debugger-authorized frame slots, and Rust-backed
  internals. These are explicit stateful abstractions, not ordinary mutable
  heap objects.
- Opaque handles: host resources, tensors, device buffers, files, network
  clients, actors, runtime tables, debug ports, secrets, model clients, and
  other authority-bearing objects.

The bootstrap VM's owned `Value` enum is only a temporary execution surface.
Final implementation should introduce a heap/handle layer behind the VM
register file before closures, long-lived modules, actors, and resource handles
become substantial.

## Root Sets

The collector must treat these as first-class roots:

- VM registers, operand temporaries, constants, and call frames.
- Closures and captured environments.
- Module globals, imports, exports, loaded module generations, and bytecode
  caches that reference language values.
- Scheduler task stacks, parked futures, pending host-call trampolines, actor
  state, and actor mailboxes.
- Atoms, channels, PubSub subscriptions, hooks, watchers, runtime tables, and
  event streams.
- REPL sessions, debugger handles, breakpoints, snapshots, rewind/fork state,
  and structured response facets.
- Host embedding call boundaries, including arguments, return values, streams,
  async callbacks, and cancellation state.
- Resource handle tables and supervisor-owned registries that contain language
  metadata or callback values.

GC must run only at explicit safe points: allocation, function calls, backward
jumps, host-call entry/exit, task yield, actor receive, debugger pause, and
other scheduler-controlled boundaries.

## Runtime Isolation

The runtime supervisor owns the heap policy, resource registry, scheduler, and
process supervision tree. Anvil processes/tasks own root sets and authority,
not raw memory.

- A process crash drops that process's roots and revokes or closes owned
  handles according to supervisor policy.
- Immutable shared values can remain alive when another process, module, table,
  or debugger session still roots them.
- Mutable identity crossing a process boundary must go through an explicit
  handle, actor, atom, channel, table, or resource capability.
- Ordinary values should never let one process overwrite another process's
  stack, frames, mailbox, or heap-owned state.

This is the VM-level analogue of an OS kernel memory boundary: language code
receives values and handles, not permission to scribble through another
execution unit's memory.

## Resource Handles

Host resources are outside the ordinary language heap. The heap may hold opaque
handle values, but the supervisor/resource registry owns the real resource and
its authority. The detailed contract is `docs/decisions/resource-handles.md`.

- Handles are unforgeable, typed, capability-checked, and revocable.
- Explicit close/revoke is the semantic release path.
- Finalizers are allowed only as cleanup fallbacks for host resources. Anvil
  code must not depend on finalizer timing for ordinary language behavior.
- Tensor/device buffers, GPU queues, WebGPU resources, files, network clients,
  processes, secrets, actors, tables, and debug ports use the same basic handle
  pattern.
- GC must be able to trace from resource tables into any language callbacks,
  metadata, subscriptions, or owner values they retain.

## Persistent Collections

Persistent data structures are the default semantic contract for Anvil
collections.

- Lists, vectors, maps, and sets are immutable by default.
- Structural sharing is allowed and expected.
- The first implementation may use simple immutable heap nodes or owned vectors
  while behavior is small.
- Mature implementations should move toward collection-specific layouts such as
  HAMT-style maps/sets and RRB-like vectors if profiling justifies it.
- Transient builders are lexical, unshareable, and must not escape into actors,
  tasks, resource tables, debug snapshots, or host calls unless frozen.

## WASM And Embedding

The heap design must work when the Anvil runtime is compiled to WASM.

- Do not expose raw Rust pointers in language values or host ABI surfaces.
- Prefer stable indices, handles, or object ids at embedding boundaries.
- Do not rely on OS virtual memory tricks, signal handlers, page protection, or
  native thread assumptions for correctness.
- Host APIs in Rust, TypeScript, and future wrappers should see values through
  public runtime facades, not GC internals.

## Diagnostics And Budgets

Allocation is part of the runtime budget model.

- Heap allocation should be charged to the current task/process/session budget.
- Memory-budget exhaustion returns structured runtime diagnostics with source
  spans where available.
- GC events should be visible in runtime metrics and debug traces when requested
  by a diagnostic facet or profiling mode.
- Backend-specific tensor/device memory pressure must be reported through
  resource diagnostics, not hidden inside ordinary heap failures.

## Non-Goals

- No full Scheme finalizer or weak-reference compatibility promise.
- No reliance on reference counting as the primary collector.
- No initial promise of incremental, concurrent, compacting, or generational
  collection.
- No language-visible object addresses.
- No raw Rust objects stored directly in ordinary language heap objects.

## Acceptance Plan

Future implementation tests should cover:

- Cyclic language graphs become collectible once all roots are dropped.
- Process crash or cancellation drops process roots without corrupting other
  processes.
- Immutable values can be shared across actors/tasks without copying mutable
  authority.
- Resource handles remain capability-checked and revocable even when handle
  values are copied.
- Explicit close/revoke releases a resource deterministically; finalizer cleanup
  is only a fallback.
- Debugger-authorized inspection can identify stable object ids without
  exposing raw addresses.
- Host callbacks and async calls preserve roots across suspension and return.
- WASM host ABI tests prove values cross the boundary through handles/ids, not
  raw pointers.
