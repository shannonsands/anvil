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
- Quality gates should eventually cover formatting, linting, tests, coverage,
  CRAP, mutation, Gherkin/spec linting, architecture boundaries, dependencies,
  performance, duplication, docs, examples, and acceptance evals.
- Executable Gherkin specs live under `specs/` and are currently run by the
  Rust-native `anvil-acceptance` Cucumber harness.

Initial concrete gates while the repo is still small:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test -p anvil-acceptance --test acceptance`
- `cargo test --workspace --all-features`

Open implementation dependency: add `.quality/`, `scripts/quality/`, richer
Gherkin/spec linting, coverage/CRAP, mutation, and benchmark baselines once
parser/runtime code exists.
