# Package, Workspace, And Artifact Format

The canonical planning note is:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/package-workspace-artifact-format.md`

Implementation-facing decision:

- Anvil packages should be Cargo-shaped.
- Use a root manifest, currently drafted as `Anvil.toml`.
- Use a lockfile, currently drafted as `Anvil.lock`, for deterministic
  dependency and artifact resolution.
- Use predictable roots: `src/`, `tests/`, `evals/`, `examples/`, `docs/`,
  `fixtures/`, and generated `.anvil/` state.
- Module resolution is manifest-driven and deterministic: package, draft
  overlays, workspace members, locked dependencies, vendored dependencies,
  standard library, then host modules.
- Bytecode, expanded AST, typed IR, diagnostics, traces, facets, draft
  candidates, and build artifacts live under `.anvil/` and are never the source
  of truth.
- Package, module, test/eval, artifact, and activation capability manifests are
  explicit and reviewable.
- Tests and evals assert structured responses, diagnostics, spans, effects, and
  denials, not just printed output.

Current implementation slice: an in-memory deterministic module resolver models
the resolution order and module diagnostics, the first `Anvil.toml` parser reads
package identity, library root, source/test/eval/example roots, and workspace
members from TOML text with manifest-phase diagnostics, and `PackageSnapshot`
bridges a parsed manifest plus known package files into package-root module
sources. The filesystem loader now reads `Anvil.toml`, walks declared source
roots deterministically, reads `.anv` files into a package snapshot, and reports
project-phase diagnostics for missing manifests or missing declared source
roots or library files. Lockfile handling, registry/dependency indexing,
workspace members, bins, capabilities, budgets, and package metadata remain
later package-system work.

Open implementation dependency: full manifest schema and value serialization
should be chosen with the reader syntax, diagnostics protocol, capability
model, and lockfile design.
