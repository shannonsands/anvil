# Draft Overlays

Canonical planning notes:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/module-system-and-hot-reload.md`

Implementation-facing decision:

- Draft overlays are Anvil's miniature worktree model for agent-authored module
  edits.
- The first implementation is in-memory. It does not yet write files, parse
  manifests, compile, test, approve, activate, migrate state, or roll back.
- A draft overlay records an id, owner/principal placeholder, status, and
  draft modules.
- A draft module records the logical module name, source override text, virtual
  draft path, optional base module metadata, and diagnostics.
- Draft overlays can be added to the deterministic module resolver as draft
  roots.
- When a draft source wins resolution, the returned `ModuleResolution` includes
  the lower-precedence source it shadows, when one exists.

Initial status values:

- `editing`
- `ready_for_test`
- `tested`
- `approved`

Current executable surface:

- Core API: `DraftOverlay`, `DraftModule`, and `DraftStatus`.
- Resolver API: `ModuleResolver::add_draft_overlay` and
  `ModuleResolver::with_draft_overlay`.
- Gherkin: `specs/draft_overlays.feature` and the draft-shadowing scenario in
  `specs/module_resolution.feature`.

Open implementation dependencies:

- Filesystem-backed draft storage under `.anvil/drafts/`.
- Draft ownership, trust-zone, and capability checks.
- Compile/test/approval/activation lifecycle.
- Compatibility checks and activation policy.
- Runtime process drain/restart/migrate behavior.
