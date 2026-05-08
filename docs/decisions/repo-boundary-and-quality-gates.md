# Repo Boundary And Quality Gates

The canonical planning note is:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/repo-boundary-and-quality-gates.md`

Implementation-facing decision:

- Anvil implementation lives in `/Users/shannon/Workspace/artivus/anvil`.
- MightyGrad remains separate and reusable for now.
- Obsidian remains the early requirements and architecture workspace.
- The Anvil repo grows local `docs/` as decisions become implementation-facing.
- Anvil should inherit the Snapdragon quality-gate shape: fast, push, and deep
  deterministic gates.
- Gate failures are design feedback. Inspect reports, add focused tests, and
  refactor before changing baselines.
- Baseline increases require explicit human approval.
- The repo currently carries no approved CRAP baseline. Coverage or structure
  should improve before any over-threshold function is accepted.
- The project target is 90% line coverage for critical runtime crates. The
  current enforced push gate is 80% and should ratchet upward only after
  meaningful tests or simplifying refactors land.
- Quality gates should eventually cover formatting, linting, tests, coverage,
  CRAP, mutation, Gherkin/spec linting, architecture boundaries, dependencies,
  performance, duplication, docs, examples, and acceptance evals.
- Executable Gherkin specs live under `specs/` and are currently run by the
  Rust-native `anvil-acceptance` Cucumber harness.

Implemented concrete gates:

- `make check-fast`: formatting, Clippy with warnings denied, Gherkin lint,
  Cucumber acceptance specs, workspace tests, and `git diff --check`.
- `make check-push`: `check-fast`, tarpaulin coverage with a default
  `ANVIL_COVERAGE_FAIL_UNDER=80`, and coverage-backed CRAP with
  `ANVIL_CRAP_THRESHOLD=30`.
- `make check-deep`: `check-push` plus `cargo-mutants`.
- `make install-hooks`: configures `.githooks`; pre-commit runs `check-fast`
  and pre-push runs `check-push`.

Deep mutation can be enabled on push with `ANVIL_DEEP_ON_PUSH=1`. It is not the
default pre-push hook because full mutation is intentionally expensive.

Remaining implementation dependencies: richer architecture/dependency/
duplication/performance gates, benchmark baselines, and risk/coupling reports
once VM/runtime surfaces exist.
