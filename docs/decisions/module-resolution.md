# Module Resolution

Canonical planning notes:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/module-system-and-hot-reload.md`

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/package-workspace-artifact-format.md`

Implementation-facing decision:

- Module resolution is deterministic, explicit, and manifest-oriented.
- The first resolver core is intentionally in-memory. Manifest parsing,
  filesystem walking, lockfile handling, and package registry integration are
  later package-system slices.
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
- Draft overlays can shadow workspace/dependency/standard/host modules, but not
  the current package in this first precedence model.
- When a draft wins resolution, `ModuleResolution.shadowed` records the
  lower-precedence source it shadows, when one exists.

Current executable surface:

- Core API: `ModuleResolver`, `ModuleRootKind`, `ModuleSource`,
  `ModuleResolution`, and `ModuleCandidate`.
- Gherkin: `specs/module_resolution.feature`.

Open implementation dependencies:

- Read `Anvil.toml` and `Anvil.lock`.
- Index `src/`, workspace members, dependencies, standard-library roots, and
  host module registrations.
- Attach module source spans from future `require` syntax to diagnostics.
- Add draft overlay ownership, capability checks, and activation workflow.
