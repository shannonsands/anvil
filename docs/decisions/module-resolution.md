# Module Resolution

Canonical planning notes:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/module-system-and-hot-reload.md`

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/package-workspace-artifact-format.md`

Implementation-facing decision:

- Module resolution is deterministic, explicit, and manifest-oriented.
- The first resolver core is intentionally explicit and manifest-oriented.
  It can be populated from in-memory package snapshots, filesystem packages,
  workspaces, and draft overlays. Lockfile handling and package registry
  integration are later package-system slices.
- Resolution uses the locked precedence order:
  1. current package
  2. draft overlays
  3. workspace members
  4. locked dependencies
  5. vendored dependencies
  6. standard library
  7. host modules
- Fully qualified module names resolve by exact match. If multiple candidates
  exist at the same precedence level, resolution fails with
  `ANVIL_MODULE_AMBIGUOUS`.
- Short names are not guessed. If a short name matches multiple known module
  suffixes, resolution fails with an ambiguity diagnostic and candidate list.
- Missing and invalid modules use structured `phase: module` diagnostics.
- Resolver-backed AST lowering can attach module diagnostics to the source span
  of the module name inside a `require` form.
- Draft overlays can shadow workspace/dependency/standard/host modules, but not
  the current package in this first precedence model.
- When a draft wins resolution, `ModuleResolution.shadowed` records the
  lower-precedence source it shadows, when one exists.

Current executable surface:

- Core API: `ModuleResolver`, `ModuleRootKind`, `ModuleSource`,
  `ModuleResolution`, `ModuleCandidate`, and `ModuleSession`.
- `ModuleSession` wraps `VmSession` with a resolver plus source store. It loads
  top-level require prefixes before evaluating the remaining forms, executes
  each resolved module at most once per session, supports transitive requires,
  detects require cycles, and leaves prior session state intact when a required
  module fails.
- CLI `run --package DIR` and `repl --package DIR` load a filesystem workspace
  snapshot and evaluate through a module-aware session.
- Gherkin: `specs/module_resolution.feature`, `specs/module_execution.feature`,
  and require-resolution scenarios in `specs/ast_lowering.feature`.

Open implementation dependencies:

- Read `Anvil.lock`.
- Index dependencies, standard-library roots, and host module registrations.
- Add namespace export/import semantics for aliases, refer, rename, and private
  bindings.
- Add draft overlay ownership, capability checks, dynamic require authority,
  and activation workflow.
