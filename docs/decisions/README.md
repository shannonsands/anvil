# Design Decisions

This directory records implementation-facing decisions. The longer working
notes live in the Obsidian project; this repo should only carry decisions that
matter to code structure.

## Locked For Now

- New Lisp in the Scheme family; Chez is the quality bar, not the compatibility
  contract.
- Bytecode VM is the reference execution path.
- No full continuations in the first core; proper tail calls are required.
- Rational and complex numbers are first-class numeric types.
- VM-scheduled lightweight tasks, actors, atoms, PubSub, hooks, watchers, and
  supervisors are central runtime features.
- REPL/debugger/attach protocol is a core surface for agents.
- Capability-aware runtime kernel mediates modules, tasks, actors, resources,
  debug access, and host calls.
- WASM is a containment and embedding substrate, backed by Anvil capabilities
  and resource handles.
- Runtime-internal process sandboxing is the core security model: WASM-style
  imports and handles for Anvil processes, without relying on Docker,
  Firecracker, or OS containers.
- Agent-facing diagnostics use a concise default response envelope with
  structured facets. The first concrete reader diagnostic includes source id,
  severity, phase, primary span, labels, expected/actual values, suggestions,
  and code-frame rendering; syntax diagnostics now reuse the same envelope.
- Embedded-first host API: Rust hosts register functions, resources, modules,
  async calls, streams, actors/services, profiles, capabilities, budgets, event
  topics, and devices; TypeScript and other environments use the public runtime
  facade rather than internals.
- Cargo-shaped packages: `Anvil.toml`, `Anvil.lock`, predictable `src/`,
  `tests/`, `evals/`, `examples/`, `docs/`, `fixtures/`, and generated
  `.anvil/` state for caches, drafts, traces, facets, and artifacts.
- Numeric semantics: exact scalar core, real `Ratio` and complex numbers,
  `Float64` decimal default, IEEE floats, checked `Prob`, strict tensor dtypes,
  explicit dtype conversions, explicit approximate equality, and first-class
  vector/tensor shape operations.
- Repo and quality gates: Anvil lives in its own repo, MightyGrad remains
  separate, Obsidian drives early requirements, repo-local docs grow with
  implementation, and Snapdragon-style fast/push/deep quality gates are part of
  the engineering model. The repo now has `.githooks`, `scripts/quality/`, and
  `make check-fast`, `make check-push`, and `make check-deep`; coverage is
  enforced at 80% with 90% as the target for critical runtime crates, and CRAP
  currently has no approved baseline.
- Executable acceptance specs use Gherkin under `specs/` and a Rust-native
  Cucumber harness in the `anvil-acceptance` crate.
- Implementation starts REPL-first: the first REPL is reader-backed and
  diagnostic-focused, with VM execution added after the reader, spans, datums,
  and acceptance specs are stable.
- Initial reader grammar: Lisp reader with `()`, `[]`, `{}`, strings, comments,
  quote sugar, keywords, nil, booleans, integers, floats, symbols, ordered maps,
  spans, and structured diagnostics.
- Initial core AST lowering covers literals, symbols, quote, `define`, `if`,
  `do`, `fn`/`lambda`, `require`, calls, vectors, and maps, preserving spans
  and emitting syntax-phase diagnostics.
- Initial syntax objects wrap reader datums with deterministic ids, source ids,
  spans, and empty hygiene context fields for future scopes and marks.
- Initial module resolution is deterministic and explicit across package,
  draft, workspace, locked dependency, vendored dependency, standard-library,
  and host roots, with module-phase diagnostics for missing or ambiguous names.
- Resolver-backed `require` lowering attaches module diagnostics to the module
  name span and records resolved import metadata when resolution succeeds.
- Initial draft overlays are in-memory miniature worktrees with owner, status,
  source overrides, virtual draft paths, diagnostics, and resolver shadow
  metadata.
- Initial `Anvil.toml` parsing reads package identity, library root,
  source/test/eval/example roots, and workspace members, with manifest-phase
  diagnostics for malformed TOML and missing required tables.
- Initial package snapshots derive package module sources from a parsed
  manifest plus known in-memory package files, registering the library root and
  `.anv` files under declared source roots.
- Initial filesystem package loading reads `Anvil.toml`, walks declared source
  roots deterministically, loads `.anv` files into a package snapshot, and
  reports project-phase diagnostics for missing manifests and missing declared
  library files or source roots.
- Initial workspace loading expands deterministic member patterns such as
  `packages/*`, loads member package snapshots, registers member modules as
  `workspace` roots, preserves root-package precedence, and surfaces missing
  member manifests through project-phase diagnostics.
- Initial bytecode VM foundation is register-based, source-mapped, and
  fuel-accounted. It executes top-level expression sequences, literals,
  vectors, ordered maps, `do`, `if`, sequential lexical `let`/`let*`, top-level
  `define`, symbol lookup, and checked bootstrap numeric primitive calls. It
  now also supports `fn` values, explicit VM call frames, lexical parameter
  locals, named function calls, direct function literal calls, returned
  closures with owned lexical captures, and proper tail calls through tail-call
  bytecode plus active-frame replacement. `VmOutput` records max call depth so
  agents and tests can inspect stack behavior, while unsupported executable
  forms produce compile-phase diagnostics and unbound symbols/non-callable
  values produce runtime diagnostics.
- Ordinary language values use tracing GC as the primary memory model. The
  first real collector should be precise, stop-the-world, non-moving, and
  safe-point based, with opaque value references, immutable default values,
  explicit mutable abstractions, and supervisor-owned resource handles outside
  the ordinary heap.
- Resource handles are supervisor-issued, typed, revocable capabilities over
  host/runtime resources. They are used through per-process/session handle
  tables, checked at every operation boundary, delegated only through explicit
  narrowing, and serialized only as scoped opaque tokens.
- Initial resource-handle Rust types exist in `anvil-core`: resource registry,
  handle table, operation schema, open/delegate requests, use-site
  authorization, denial diagnostics, audit events, and resource acceptance
  specs.
- Initial resource adapter execution contract exists in `anvil-core`: adapter
  trait, operation request/payload/outcome, execution modes, effect records,
  checked dispatch, and adapter-backed acceptance specs.
- Initial capability-profile integration exists in `anvil-core`: runtime
  profiles gate resource open, operation use, adapter execution, delegation,
  and revocation, with `capability_denied` diagnostics and
  `capability_profiles.feature` acceptance coverage for principal, trust-zone,
  generic, and domain-specific capability behavior.
- MightyGrad remains an independent backend project. Anvil integrates through a
  backend adapter when the compute IR is ready.

## Next Decision Dives

- First acceptance programs and eval matrix.
- Reader and syntax details beyond the initial datum reader: namespaces, exact
  numeric literal sugar, metadata, tagged literals, and reader macros.
- Concrete persistent collection layouts, root-table implementation details,
  GC tuning, and later generational/incremental/compacting collector strategy
  beyond the initial tracing-GC contract.
- Async, streaming, blocking, actor-backed, and device-backed runners behind
  the initial resource adapter execution contract.
- Capability profile composition, persistent policy storage, approval flows,
  and audit sinks beyond the first resource operation checks.
- Concrete runtime syntax for `defactor`, supervisors, atoms, channels,
  task groups, PubSub, hooks, watchers, event streams, and reactive forms.
- Debugger and attach semantics: breakpoints, frame inspection, debug eval,
  rewind/fork, scheduler replay, effect barriers, and debug authority.
- ETS-like runtime table semantics, persistence modes, watch/subscribe
  behavior, ownership, and access control.
- Module execution semantics beyond resolution: dynamic require, draft
  activation, module generations, bytecode cache invalidation, staged
  replacement, rollback, and versioning.
- Macro system contract: CL-style compiler macros, reader macros, hygiene
  policy, expansion traces, macro capabilities, typed lowering, and declarative
  IR expansion.
- Compute and device resource model: CPU/WebGPU/Candle/MightyGrad backend
  ladder, tensor buffers, placement, compute IR, shader/backend diagnostics,
  and CPU/GPU equivalence tests.
- Type syntax details: generics, hard type patterns, soft membership patterns,
  dimension-tracked vectors, shape variables, effect/capability types, and
  resource-handle types.
- WASM import manifest and host ABI: profile syntax, value ABI, native runtime
  choice, component-model boundary, browser/server profiles, and debug metadata
  exposure.
- MarkoDB standard-library slice boundaries: which declarative forms are core
  forms versus standard-library macros with compiler hooks, and the first
  mandatory eval domains.
- Learned policy and training artifact contract: environment/simulation IR,
  training job resources, policy metadata, artifact references, and capability
  boundaries.
- Independent MightyGrad completion milestone and adapter contract so Anvil can
  target it without folding tensor-kernel ownership into the language runtime.
