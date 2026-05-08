# Agent Guide

Anvil is in early architecture and requirements work. It is a long-term Rust
language/runtime project for a Chez-quality Lisp-family system aimed at agent
programming, declarative reasoning, secure live runtimes, and later ML-native
compute.

Keep work small, documented, and aligned with the Obsidian planning project:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime`

## Source Of Truth

Use this order when deciding what should happen:

1. Current user request.
2. Repo-local implementation docs under `docs/`.
3. Obsidian planning notes in the Anvil project.
4. Existing code and tests.

If a decision changes, update the relevant Obsidian note and the repo-local
implementation-facing doc when it constrains code. Do not leave a decision only
in chat if it should guide future agents.

## Working Assumptions

Preserve these unless the planning docs are updated first:

- Rust implementation, bytecode VM first.
- Chez-quality Lisp/Scheme-family language, but not initially Scheme-compatible.
- Capability-aware runtime kernel with process/task isolation.
- REPL/debugger as a first-class agent interface.
- Lightweight VM tasks, actors, atoms, PubSub, hooks, and watchers.
- Immutable language values by default; explicit mutation through controlled
  cells, actors, resource handles, transient builders, tables, or Rust-backed
  internals.
- Ordinary language heap values use tracing GC; host resources, tensors,
  devices, files, secrets, actors, runtime tables, and debug ports stay behind
  supervisor-owned opaque handles.
- WASM is a sandbox and portability target, not the whole security model.
- MightyGrad remains a separate tensor/backend project that Anvil can target
  later through a backend adapter.
- Exact scalar numerics, rational and complex support, strict tensor dtypes,
  explicit approximate equality, and checked probability constructors.

## Implementation Discipline

- Do not add large implementation surfaces before the relevant decision note is
  written or linked from `docs/decisions/README.md`.
- Prefer small vertical slices that connect docs, code, tests, and acceptance
  specs.
- Keep Anvil cargo-shaped. Add crates only when they clarify ownership or avoid
  mixing concerns.
- Keep MightyGrad integration behind a future backend adapter. Do not fold
  MightyGrad-specific implementation into the language kernel.
- Treat security, capability checks, structured diagnostics, and agent-readable
  behavior as core runtime contracts, not optional wrappers.
- Avoid compatibility promises that the charter has not made.

## Gherkin And Acceptance Specs

Executable acceptance specs live in `specs/*.feature` and are run through the
`anvil-acceptance` crate:

```bash
cargo test -p anvil-acceptance --test acceptance
```

Agents should formalize stable requirements into Gherkin scenarios so design
work becomes executable over time.

Rules:

- When adding or changing externally visible behavior, add or update a Gherkin
  scenario first, then implement the Rust step definitions and code needed to
  pass it.
- When drafting a decision that is not executable yet, include proposed
  acceptance scenarios in the decision doc or acceptance matrix. Move them into
  `specs/` once there is a real executable surface or a deliberate harness for
  that contract.
- Do not add failing aspirational scenarios to the active Cucumber suite unless
  the runner is explicitly configured to exclude or mark them.
- Keep scenario language concrete and agent-readable: name the observable input,
  action, response, diagnostics, spans, capability denials, effects, or audit
  events.
- Prefer one behavior per scenario. Use multiple small scenarios instead of one
  broad narrative scenario.
- Every important decision family should eventually have coverage: reader
  syntax, module resolution, diagnostics, capabilities, process sandboxing,
  REPL envelopes, staged replacement, numeric semantics, concurrency, MarkoDB
  forms, and tensor dtype/shape behavior.
- If a feature file describes behavior that is also documented in Obsidian,
  link or name the canonical decision in comments or scenario titles where
  useful.

## Quality Gates

Quality gates are part of the design. Treat failures as design feedback:
inspect the report, add focused tests, then refactor or split code before
changing baselines.

Current tiers:

```bash
make check-fast
make check-push
make check-deep
```

Installed hooks use `.githooks`: pre-commit runs `scripts/quality/fast.sh`;
pre-push runs `scripts/quality/push.sh`. Set `ANVIL_DEEP_ON_PUSH=1` to run
mutation testing on push.

`check-push` generates tarpaulin coverage and runs CRAP with threshold 30. The
repo currently has no approved CRAP baseline. If CRAP fails, add meaningful
coverage or refactor the function before considering any baseline.

`check-deep` runs `cargo-mutants`. Mutation misses are test-quality feedback;
add assertions that kill the mutant, or simplify/exclude only when the code is
demonstrably not worth mutating and the exception is documented.

As the repo grows, preserve the planned gate shape: architecture checks,
dependency checks, duplication checks, performance baselines, risk reports, and
coupling reports should become deterministic scripts rather than loose advice.

Baseline increases require explicit human approval.

## Definition Of Done

A change is not done until:

- The implementation matches the current decision docs or updates them.
- User-facing or agent-facing behavior has tests.
- Stable externally visible behavior has a Gherkin scenario, or the relevant
  decision doc explains why the scenario is still pending.
- The acceptance suite passes when touched.
- The relevant quality gates pass, or any skipped gate is reported with the
  reason.
- Docs and examples are updated when the change affects how agents should use
  the project.
