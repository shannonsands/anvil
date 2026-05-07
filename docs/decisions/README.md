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
- Agent-facing diagnostics use a concise default response envelope with opt-in
  drill-down facets for spans, expansion traces, effects, denials, frames,
  audit, artifacts, and suggestions.
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
  the engineering model.
- Executable acceptance specs use Gherkin under `specs/` and a Rust-native
  Cucumber harness in the `anvil-acceptance` crate.
- Implementation starts REPL-first: the first REPL is reader-backed and
  diagnostic-focused, with VM execution added after the reader, spans, datums,
  and acceptance specs are stable.
- Initial reader grammar: Lisp reader with `()`, `[]`, `{}`, strings, comments,
  quote sugar, keywords, nil, booleans, integers, floats, symbols, ordered maps,
  spans, and structured diagnostics.
- MightyGrad remains an independent backend project. Anvil integrates through a
  backend adapter when the compute IR is ready.

## Next Decision Dives

- First acceptance programs and eval matrix.
- Reader and syntax details beyond the initial datum reader: namespaces, exact
  numeric literal sugar, metadata, tagged literals, and reader macros.
- Value representation, heap layout, and tracing GC strategy.
- Resource-handle contract for Rust, MarkoDB, tensor, file, network, process,
  runtime table, actor, debug, and secret resources.
