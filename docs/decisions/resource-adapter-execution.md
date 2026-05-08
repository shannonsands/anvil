# Resource Adapter Execution

Canonical planning note:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/resource-adapter-execution.md`

Implementation-facing decision:

- Resource adapters are trusted host/runtime implementations behind resource
  handles. They are never called until the runtime has authorized the handle,
  operation, resource type, trust zone, current revocation state, and narrowed
  grants.
- `ResourceRegistry::check_operation` remains the pure use-site authority
  check. `ResourceRegistry::execute_operation` is the first adapter trampoline:
  it checks authority, checks adapter type/operation compatibility, builds a
  safe request, and then calls the adapter.
- Adapters receive `ResourceAdapterRequest` with authorization metadata, a safe
  handle descriptor, payload values, and an execution mode. They do not receive
  ambient authority or raw resource pointers through this contract.
- Adapters return `ResourceAdapterOutcome` with status, value, continuation
  token, and effect records, or `ResourceAdapterFailure`, which the runtime
  maps into `ResourceError` with resource-phase diagnostics.
- The first execution modes are `pure`, `effectful`, `blocking`, `async`,
  `streaming`, `actor_backed`, and `device_backed`. Only synchronous in-process
  execution is implemented now; the enum is the stable shape for later async,
  stream, actor, and device schedulers.
- Denials must happen before adapter execution. Adapter-side errors are
  failures after authorization, not evidence that authority checks can be
  skipped.

## Implementation Status

Implemented in this slice:

- `ResourceAdapter` trait with adapter id, resource type id, supported
  operations, execution mode, and execution method.
- `ResourceOperationRequest`, `ResourceOperationPayload`, and
  `ResourceOperationOutcome`.
- `ResourceAdapterRequest`, `ResourceAdapterOutcome`, `ResourceAdapterStatus`,
  `ResourceExecutionMode`, `ResourceEffectRecord`, and
  `ResourceAdapterFailure`.
- `ResourceRegistry::execute_operation` for checked adapter dispatch.
- Unit tests covering successful execution, missing-capability denial before
  adapter calls, wrong adapter type before execution, and adapter failure
  mapping.
- `specs/resource_adapters.feature` covering the executable agent-facing
  contract.

Not implemented yet:

- Async runtime integration, cancellation handles, and wakeups.
- Streaming registries and stream continuation ownership.
- Blocking adapter isolation or scheduler handoff.
- Actor-backed and device-backed adapter runners.
- Capability-profile integration beyond existing handle grants.
- Persistent audit sinks and transport/WASM token stores.

## Adapter Call Order

Every adapter-backed operation follows this order:

1. Resolve the handle from the current handle table.
2. Verify the handle is active.
3. Resolve the resource entry.
4. Verify resource type and trust zone still match the handle.
5. Verify the resource operation schema exists.
6. Verify the handle has the required grant.
7. Verify the adapter type id and supported operation match the authorization.
8. Build `ResourceAdapterRequest`.
9. Call the adapter.
10. Return structured outcome or map adapter failure into a resource diagnostic.

The adapter must not be invoked if steps 1-7 fail.

## AX Notes

For agents, the important property is predictability:

- A denied operation says exactly what was missing and reports that the adapter
  was not called.
- A host/backend failure is visible as an adapter failure with the same
  resource diagnostic envelope.
- The effect list gives REPL/debug surfaces a compact default summary while
  leaving room for drill-down traces later.
