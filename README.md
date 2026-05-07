# Anvil

Anvil is the implementation home for a long-term Rust language/runtime project:
a Chez-quality Lisp-family system for agent programming, declarative reasoning,
secure live runtimes, and eventual ML-native compute.

This repository is intentionally minimal right now. The active work is Phase 0:
charter, requirements, and design decisions. The local planning workspace lives
at:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime`

## Current Shape

- Rust workspace with a tiny smoke-testable core crate and CLI crate.
- Bytecode VM first, not native JIT first.
- Agent REPL, debugger, and runtime attach are core product surfaces.
- Capability-aware runtime kernel, process/task isolation, and auditable
  resource handles.
- Clojure-inspired concurrency: lightweight tasks, actors, atoms, parallel
  collection operations, PubSub, hooks, watchers, and supervisors.
- Gradual, ontology-aware type direction with hard and soft membership.
- MarkoDB/QBBN/VSA standard-library slice as the first serious acceptance
  target.
- Tensor/ML integration as a staged resource/backend layer rather than an early
  language-kernel dependency.

## Repo Boundaries

Anvil owns the language, VM, module system, REPL/debugger, host API, security
model, type surface, and standard-library contracts.

MightyGrad remains a separate reusable tensor/backend project. Anvil may target
it later through a backend adapter, but MightyGrad should still be usable on its
own, including from Rust hosts and eventually other language bindings.

## Quality Gates

Anvil should use Snapdragon-style quality gates as the codebase grows: fast,
push, and deep tiers covering formatting, linting, tests, coverage, CRAP,
mutation, specs, architecture, dependencies, performance, duplication, risk,
and coupling.

The current repo is still a scaffold. The available checks are:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p anvil-acceptance --test acceptance
cargo test --workspace --all-features
```

## Quick Start

```bash
cargo test
cargo test -p anvil-acceptance --test acceptance
cargo run -p anvil-cli
```
