# Acceptance Programs

The first acceptance programs should prove that Anvil is useful for the actual
target domain before implementation taste hardens into language design.

Initial candidates:

- Small functional Lisp programs with tail recursion, closures, locals,
  conditionals, lists, vectors, maps, and structured errors.
- Agent-authored module requiring another module, with deterministic module
  resolution and source-span diagnostics.
- MarkoDB-style fact/rule/query/explain program.
- Hard and soft type membership example using QBBN/VSA evidence without letting
  soft evidence silently satisfy hard type requirements.
- Lightweight `pmap` over a collection with cancellation and bounded scheduler
  resources.
- Actor with mailbox, supervised restart, and REPL inspection.
- Capability denial example for file, network, debug attach, and resource
  handle use.
- Resource-handle example covering open, inspect, denial-before-adapter-call,
  narrowed delegation, explicit close, revoke, and redacted display.
- Value/heap stress example with cyclic language graphs, actor roots, process
  cancellation, resource handle revocation, and debug inspection without raw
  address exposure.
- Staged module replacement: draft, compile, test, approve, activate, rollback.
- WASM profile example with explicit host imports.
- Tiny tensor/resource example once the value/resource model is coherent.

The exact matrix should live here once the syntax, GC, resource-handle, and
capability decisions are more concrete.

## Executable Specs

The first executable acceptance harness is intentionally tiny but real:

- Gherkin feature files live under `specs/`.
- Rust step definitions live in the `anvil-acceptance` crate.
- Agents can run the suite with
  `cargo test -p anvil-acceptance --test acceptance`.
- `reader_repl.feature` covers the first REPL-visible reader behavior,
  Clojure-like delimiters, multiline interactive input, VM-backed evaluation
  across an interactive session, JSON pending events, JSON-serializable
  diagnostics, and source-aware diagnostic rendering.
- `ast_lowering.feature` covers the first syntax layer behavior: lowering
  definition and function forms, serializing AST JSON, and returning
  syntax-phase diagnostics. It also covers minimal `require` lowering,
  resolver-backed import metadata, and module diagnostics at the module name
  span.
- `syntax_objects.feature` covers deterministic syntax-object ids, source ids,
  spans, initial hygiene context serialization, and reader diagnostic
  pass-through.
- `module_resolution.feature` covers deterministic root precedence, draft
  shadowing, ambiguous short-name diagnostics, and missing-module diagnostics.
- `draft_overlays.feature` covers the first in-memory draft overlay shape:
  owner, status, source override, virtual path, and diagnostics.
- `manifest.feature` covers the first `Anvil.toml` parser: package identity,
  library root, default and explicit source/test/eval/example roots, workspace
  members, missing-table diagnostics, and malformed TOML diagnostics.
- `package_snapshot.feature` covers the first manifest-backed package snapshot:
  library module registration, source-root module derivation, and ignoring
  package files outside declared source roots.
- `filesystem_package_loading.feature` covers loading a real package directory
  into the resolver, plus project-phase diagnostics for a missing `Anvil.toml`
  and missing declared library files or source roots.
- `workspace_loading.feature` covers workspace member loading from
  `[workspace].members`, root-package precedence over workspace members, missing
  member manifests, and ambiguous workspace module diagnostics.
- `module_execution.feature` covers the first module-aware session execution:
  require-prefix package modules, transitive requires, loaded module tracking,
  failure isolation, cycle diagnostics, and explicit alias deferral.
- `bytecode_vm.feature` covers the first executable VM behavior: running
  literals, expression sequences, `if`, quote-as-data, vectors, maps, top-level
  bindings, function calls, closures, proper tail calls, lexical `let`
  bindings, compile diagnostics for unsupported forms, and runtime diagnostics
  for fuel exhaustion.
- `vm_session.feature` covers stateful VM-backed evaluation: definitions,
  quoted data, and closures survive across successful evaluations, while failed
  evaluations and fuel exhaustion leave the existing session state intact.
- `resource_handles.feature` covers the first resource-handle substrate:
  typed open, redacted display, use-site missing-capability denials, narrowed
  delegation, rejected widening, and revocation.
- `resource_adapters.feature` covers the first adapter-backed execution
  contract: checked dispatch after authorization, denial before adapter calls,
  structured adapter outcomes, and adapter failure diagnostics.
- `capability_profiles.feature` covers process-profile mediation for resource
  open, adapter execution, delegation, and revocation, including
  `capability_denied` diagnostics and zero adapter calls on denied operations.
- `host_functions.feature` covers the first embedded host-function contract:
  direct calls from VM sessions, calls from required modules, arity/profile
  mediation before callback invocation, authorized host calls, and host
  callback failures mapped into VM runtime diagnostics.

This keeps acceptance behavior cargo-shaped while leaving room for richer
spec linting, coverage mapping, eval artifacts, and agent-readable reports as
the language surface becomes executable.
