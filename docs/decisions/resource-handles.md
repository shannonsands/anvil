# Resource Handles

Canonical planning note:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/resource-handles.md`

Implementation-facing decision:

- Resource handles are supervisor-issued, unforgeable, typed references to
  authority-bearing resources. They are not raw Rust pointers, ordinary
  heap-owned resources, or ambient globals.
- A handle value is only usable through a process/session handle table owned by
  the runtime supervisor. Copying a handle value does not expand authority.
- Every resource operation checks current authority at use time: handle
  validity, resource type, operation capability, trust zone, lifetime,
  delegation policy, revocation state, budgets, and resource policy.
- Passing a handle across tasks, actors, sessions, WASM instances, or host
  facade transports requires supervisor-mediated delegation. Delegation can
  narrow authority but must never widen it.
- Explicit `close` and supervisor `revoke` are the deterministic lifetime
  operations. GC cleanup and host finalizers are fallback cleanup only, not
  ordinary language semantics.
- Handle use must produce structured diagnostics and audit events, including
  precise denial reasons and safe suggestions for agents.

## Implementation Status

The first Rust substrate now exists in
`crates/anvil-core/src/resource.rs`.

Implemented in this slice:

- `ResourceRegistry` and `ResourceEntry` for registered resource metadata.
- `HandleTable` and `HandleEntry` for process/session-scoped handles.
- `ResourceOperationSchema`, policy, lifetime, delegation, display, and
  revocation enums.
- `ResourceOpenRequest` and `ResourceDelegationRequest`.
- `ResourceOperationAuthorization` for allowed use-site checks.
- `ResourceError`, `ResourceDenial`, `ResourceDenialReason`, and
  `ResourceAuditEvent`.
- `DiagnosticPhase::Resource` for structured denial diagnostics.
- Unit tests and `specs/resource_handles.feature` covering typed open,
  redacted display, missing-capability denial, narrowed delegation, rejected
  widening, and revocation.

Not implemented yet:

- Real host/resource adapters.
- Async/blocking/streaming resource execution.
- Capability-profile integration beyond handle grants.
- Transport, TypeScript, or WASM facade token stores.
- Persistent audit sinks.

## Core Model

The runtime has two related tables:

- The resource registry maps a supervisor-owned `resource-id` to the real host,
  runtime, tensor, debug, table, actor, stream, secret, model, or device
  resource.
- A process/session handle table maps an opaque `handle-id` to a narrowed grant
  over one resource.

An Anvil program sees only handle values. Hosts and transports see only public
handle descriptors or scoped opaque tokens. Only the supervisor and trusted
resource adapters can reach the real Rust object or backend resource.

This is the resource-level version of the VM sandbox: language code receives a
capability-bearing reference to a resource, not a pointer or ambient name.

## Required Metadata

Resource registry entries should include:

- `resource-id`: stable supervisor-owned identity.
- `type-id`: resource kind, such as `markodb.collection`, `file.root`,
  `runtime.table`, `actor`, `debug.port`, `tensor.buffer`, `device.webgpu`,
  `secret`, `model.client`, or `stream`.
- `owner`: principal, runtime, package, host, or supervisor that owns the real
  resource.
- `trust-zone`: zone where the resource is valid.
- `adapter`: trusted Rust/backend implementation that owns operations.
- `operation-schema`: supported operations, inputs, outputs, effects, and
  capability requirements.
- `resource-policy`: grant, delegation, revocation, audit, redaction, and
  lifetime policy.
- `budget-policy`: host-call, memory, mailbox, stream, tensor/device, wall
  time, or fairness budgets.
- `debug-policy`: what can be inspected and how values are redacted.

Handle table entries should include:

- `handle-id`: opaque per-process/session token.
- `resource-id`: target registry entry.
- `type-id`: expected resource kind.
- `holder`: principal, process, task, actor, session, or WASM instance that can
  use the handle.
- `grants`: narrowed operations/capabilities available through this handle.
- `trust-zone`: zone where the handle is usable.
- `lifetime`: lexical, call, process, actor, session, runtime, lease, stream,
  draft, test-run, or artifact-bound lifetime.
- `revocation-state`: active, closing, closed, expired, revoked, or poisoned.
- `delegation-policy`: whether it can be passed and how grants must narrow.
- `audit-policy`: allow/deny logging, redaction, approval, and effect policy.
- `display-policy`: what agents see in REPL and structured responses.

## Operation Contract

The first resource operation families are:

| Operation | Purpose |
| --- | --- |
| `import` / `open` | Resolve a named or manifest-declared resource and issue a handle. |
| `read` | Read data or metadata. |
| `write` | Mutate a resource or emit a write intent. |
| `call` | Invoke a resource-specific method. |
| `stream` | Start or consume a stream-backed resource operation. |
| `inspect` | Return safe metadata for REPL/debug/agent tooling. |
| `delegate` | Issue a narrowed handle for another holder. |
| `close` | Release this handle deterministically. |
| `revoke` | Supervisor invalidates one handle, a resource, or a grant family. |

Every operation follows the same order:

1. Resolve the current process/session and handle table.
2. Verify the handle exists and is active.
3. Verify the requested operation is supported by the resource type.
4. Check process profile, handle grants, resource policy, trust zone, owner,
   delegation state, budgets, and revocation state.
5. Emit an audit event for deny, approval-required, or effectful allow.
6. Call the trusted adapter only after checks pass.
7. Convert adapter output, denial, cancellation, panic, timeout, or backend
   error into the standard response envelope.

Host code must not be called before a denial is known to be allowed. Partial
effects before capability checks are a bug.

## Delegation

Delegation is explicit and supervisor-mediated.

Allowed examples:

- A parent process delegates a read-only MarkoDB collection handle to a child
  test process.
- A REPL session delegates a scratch table handle to an actor it spawned.
- A stream handle is narrowed to read-only consumption by a TypeScript facade
  client.
- A device handle delegates a tensor buffer handle with a smaller memory budget.

Denied examples:

- Sending a raw handle value to another actor and treating that as authority.
- Delegating a handle into another trust zone without policy support.
- Turning `secret/use` into `secret/read`.
- Turning `resource/read` into `resource/write`.
- Persisting a live handle in a package, bytecode cache, or model artifact.

Delegation produces a new handle entry. It must copy only safe metadata and
must narrow grants, budgets, lifetime, and inspection rights.

## Lifetime And Revocation

Handle lifetime is independent from ordinary language value lifetime.

- Explicit `close` releases the holder's handle.
- Supervisor `revoke` invalidates handles or resources immediately at the next
  check and may cancel in-flight operations according to resource policy.
- Lease expiry turns future use into an expired-handle denial.
- Process crash or cancellation releases process-owned handles according to
  supervisor policy.
- In-flight operations root their handles until completion or cancellation.
- GC may retire unreachable handle values and notify the registry, but Anvil
  programs must not depend on GC timing for resource release.

Revoked handles should remain inspectable enough to explain failure when the
holder has `resource/inspect` or relevant debug authority.

## Resource Classes

The same handle contract applies across resource families:

- Rust host objects and native services.
- MarkoDB collections, QBBN/VSA stores, planners, and explanation resources.
- Files, directories, virtual filesystems, scratch roots, and write intents.
- Network clients, allowlisted endpoints, model clients, and external tools.
- Runtime tables, topics, hooks, watchers, channels, and streams.
- Actors, processes, mailboxes, supervisors, and task groups.
- Debug ports, trace streams, snapshots, breakpoints, and frame views.
- Secrets and credentials, especially `secret/use` handles that do not reveal
  raw secret material.
- Devices, tensor buffers, compute queues, kernels, training jobs, policy
  artifacts, and future MightyGrad/Candle/WebGPU resources.

Resource-specific modules can expose nicer APIs, but they must lower to this
same use-site check and audit model.

## Serialization And Display

Default handle display should be useful to agents without leaking authority:

```scheme
#<resource markodb.collection markodb:papers caps=[read qbbn/ask] zone=project.markodb>
```

Structured output should include safe metadata:

- Handle kind and resource type.
- Stable handle id or redacted token according to display policy.
- Trust zone.
- Allowed operation names, possibly clipped.
- Lifetime/expiry summary.
- Revocation state.
- Facet links for richer inspection when authorized.

Serialization rules:

- Runtime-local structured responses may include scoped handle references.
- Transport adapters may serialize handles only as opaque tokens scoped to a
  session, principal, transport, or WASM instance.
- Package files, bytecode caches, eval artifacts, model artifacts, and logs
  should store resource requirements or redacted descriptors, not live handles.
- Debug/audit exports must redact secrets and resource-specific protected
  metadata by default.

## WASM And Host Facades

Across WASM and TypeScript boundaries, a handle is an opaque token. The token
has no authority without a matching supervisor-side handle table entry.

- WASM guests receive instance-scoped handle ids.
- TypeScript wrappers receive facade-scoped handle tokens and metadata.
- Hosts validate every handle use through the same operation contract.
- No raw Rust pointer, native file descriptor, tensor pointer, GPU buffer
  pointer, or host object reference crosses the boundary.

## Diagnostics

Resource denials use structured diagnostics rather than vague exceptions.

Required denial reasons include:

- handle missing
- handle expired
- handle revoked
- handle closed
- wrong resource type
- wrong trust zone
- missing capability
- delegation denied
- serialization denied
- budget exhausted
- approval required
- resource unavailable
- adapter failure

Diagnostics should include operation, process/session, principal, trust zone,
resource type, expected/actual type or capability where safe, audit event id,
source span where available, and suggestions such as "request a read handle",
"run in a draft test process", or "delegate a narrowed handle".

## Acceptance Plan

Future tests should cover:

- Opening an allowed resource issues a typed handle with redacted display.
- A missing capability denial occurs before adapter invocation.
- A wrong-type handle produces a structured diagnostic.
- Copying a handle inside one process does not duplicate or widen grants.
- Sending a handle to another actor requires explicit delegation.
- Delegation can narrow `read/write` to `read` and cannot widen authority.
- Revocation invalidates future calls and reports a stable denial reason.
- Explicit close releases the handle deterministically.
- Secret `use` handles allow an operation without revealing the secret value.
- WASM/TypeScript facade tokens cannot be used outside their scoped session.
- Debug inspection shows safe handle metadata under authority and redacts it
  without authority.
