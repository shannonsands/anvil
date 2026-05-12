# Milestones

## M0: Charter To Skeleton

Exit criteria:

- Implementation repo exists and builds.
- Obsidian planning links to this repo.
- First acceptance matrix is drafted.
- Syntax, GC, resource-handle, capability, diagnostics, host API, package, and
  numeric decision dives are scheduled or drafted.
- Initial executable Cucumber acceptance harness exists.

## M1: Reader-Backed REPL, Datum Reader, Errors

Exit criteria:

- CLI REPL exists and is honest about being read-only.
- Reader/parser with spans.
- Datum model and pretty printer.
- Structured error output suitable for agents.
- Cucumber specs for reader-visible REPL behavior.
- Round-trip tests for the first syntax slice.

## M2: AST, Modules, And Macro Skeleton

Exit criteria:

- Core AST model.
- Syntax objects for macro expansion.
- Deterministic module resolver.
- Draft overlay representation.
- Module diagnostics with spans.

Current slice:

- Core AST lowering exists for literals, symbols, quote, `define`, `if`, `do`,
  `fn`/`lambda`, `require`, calls, vectors, and maps.
- Syntax objects exist as a span-preserving wrapper around reader datums, with
  deterministic ids and initial hygiene context fields.
- Syntax diagnostics reuse the shared diagnostic envelope with
  `phase: syntax`.
- Deterministic module resolver core exists for package, draft, workspace,
  dependency, standard-library, and host roots, with module-phase diagnostics.
- Resolver-backed `require` lowering records resolved import metadata and
  reports module diagnostics at the module name source span.
- Draft overlay representation exists as an in-memory miniature worktree model,
  including resolver shadow metadata.
- Minimal `Anvil.toml` parsing exists for package identity, library root,
  source/test/eval/example roots, and workspace members, with manifest-phase
  diagnostics for malformed TOML and missing required tables.
- Package snapshots can build a package-root module resolver from a parsed
  manifest plus known in-memory `.anv` files under declared source roots.
- Filesystem package loading reads `Anvil.toml`, walks declared source roots,
  loads `.anv` files into a package snapshot, and emits project-phase
  diagnostics for missing manifests and missing declared library files or source
  roots.
- Workspace loading expands member patterns such as `packages/*`, loads member
  package snapshots, registers member modules as `workspace` roots, preserves
  root-package precedence, and reports missing member manifests.

## M3: Bytecode VM Foundation

Exit criteria:

- Register-based bytecode interpreter.
- Proper tail calls without Rust stack growth.
- Closures, locals, calls, branches, and basic immutable values.
- Source-span runtime errors.
- Basic fuel/budget accounting.

Current slice:

- Bootstrap register bytecode runs top-level expression sequences, literals,
  quote-as-data, symbols and lists as data, vectors, ordered maps, `do`, `if`,
  top-level `define`, symbol lookup, checked bootstrap numeric primitive calls,
  function values, direct and named function calls, returned closures, nested
  closures, and proper tail calls.
- Lexical parameter locals, owned lexical closure captures, and active-frame
  replacement for tail calls are implemented. Sequential lexical `let`/`let*`
  bindings now use explicit scope push/bind/pop bytecode and restore shadowed
  locals when the lexical body exits.
- `VmSession` now gives the REPL and future host surfaces stateful evaluation:
  successful interactions persist top-level bindings, binding names, and
  function prototypes; failed interactions do not corrupt prior state.
- The CLI `repl` command is VM-backed for complete interactive forms, while
  `read` remains a reader/diagnostic command.
- `ModuleSession` now gives package/workspace-aware sessions an executable
  require prefix: modules resolve through the deterministic resolver, load once
  per session, support transitive requires, detect cycles, and keep prior state
  intact when a required module fails.
- CLI `run --package DIR` and `repl --package DIR` use module-aware sessions.
- Unsupported executable forms outside that session wrapper should fail with
  compile-phase diagnostics until their runtime contracts are implemented.
- The final value direction is now locked as tracing-GC-managed immutable
  language values with explicit mutable abstractions and supervisor-owned
  resource handles outside the ordinary heap.

## M4: Host API And Capabilities

Exit criteria:

- Rust hosts can register functions, modules, and resource handles.
- Capability checks are precise and inspectable.
- A module can run under multiple profiles with different authority.

Current implementation slice:

- Resource handles are locked as supervisor-issued, typed, revocable
  capabilities over host/runtime resources.
- Handle use is checked at every operation boundary through a
  process/session-scoped handle table.
- Delegation creates a narrowed handle and never widens authority.
- Live handles are not persisted into packages, bytecode caches, eval artifacts,
  model artifacts, or logs.
- `anvil-core` now has initial resource registry, handle table, operation
  schema, authorization, denial, and audit event structs.
- `resource_handles.feature` covers the first executable resource contract.
- `anvil-core` now has the first resource adapter trait and checked dispatch
  trampoline, including operation payloads, outcomes, execution modes, effect
  records, and adapter failure mapping.
- `resource_adapters.feature` covers authorized adapter execution, denial
  before adapter calls, and adapter failure diagnostics.
- `anvil-core` now has `CapabilityProfile` and profile-aware resource checks
  for open, use, adapter execution, delegation, and revocation.
- `capability_profiles.feature` covers `capability_denied` diagnostics,
  missing-capability reporting, revocation, and denial before adapter calls.
- `anvil-core` now has an initial synchronous `HostFunctionRegistry`,
  `HostFunctionSpec`, and host-call bytecode path. VM and module sessions can
  register Rust callbacks by explicit name, with arity checks,
  capability-profile checks, structured host-failure diagnostics, and denial
  before callback invocation.
- `host_functions.feature` covers direct VM calls, required-module calls,
  capability denial before invocation, authorized calls, and host callback
  failure diagnostics.
- `anvil-core` now has the first canonical eval response envelope:
  `EvalResponse`/`ResponseEnvelope` with protocol, status, kind, summary, safe
  structured values, diagnostics, metadata, effects/facets hooks, and opt-in
  debug facets. `VmSession`, `ModuleSession`, standalone VM response helpers,
  CLI `run --json`, and VM-backed `repl --json` can emit it.
- `response_envelope.feature` covers concise success envelopes, runtime
  diagnostic envelopes, VM metadata, safe value serialization, and debug facet
  opt-in.
- `HostFunctionSpec` now carries optional typed signature metadata for
  embedding wrappers and future transport bindings.
- `anvil-core` now has `EmbeddedRuntime`, the first Rust host facade over
  module-aware eval, host functions, resource registration, handle opening,
  capability profile registration/activation, default VM budget, and
  inspectable `EmbeddedRuntimeSnapshot` metadata.
- `embedding_contract.feature` covers eval envelopes, facade inspection, host
  function registration, active-profile host mediation, and resource opens
  under the active profile.

Remaining M4 work:

- Async, streaming, blocking, and actor-backed host runners.
- Result ids, response/facet retention, and facet lookup.
- TypeScript/WASM transport bindings over the Rust facade.
- Profile composition, policy persistence, approval flows, and audit sinks.
- Full host-call/resource audit events in response facets.
