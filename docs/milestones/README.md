# Milestones

## M0: Charter To Skeleton

Exit criteria:

- Implementation repo exists and builds.
- Obsidian planning links to this repo.
- First acceptance matrix is drafted.
- Syntax, GC, resource-handle, capability, diagnostics, host API, package, and
  numeric decision dives are scheduled.
- Initial executable Cucumber acceptance harness exists.

## M1: Reader-Backed REPL, Datum Reader, Errors

Exit criteria:

- CLI REPL exists and is honest about being read-only.
- Reader/parser with spans.
- Datum model and pretty printer.
- Structured error output suitable for agents.
- Cucumber specs for reader-visible REPL behavior.
- Round-trip tests for the first syntax slice.

## M2: AST, Modules, And Macro Skeleton

Exit criteria:

- Core AST model.
- Syntax objects for macro expansion.
- Deterministic module resolver.
- Draft overlay representation.
- Module diagnostics with spans.

## M3: Bytecode VM Foundation

Exit criteria:

- Register-based bytecode interpreter.
- Proper tail calls without Rust stack growth.
- Closures, locals, calls, branches, and basic immutable values.
- Source-span runtime errors.
- Basic fuel/budget accounting.

## M4: Host API And Capabilities

Exit criteria:

- Rust hosts can register functions, modules, and resource handles.
- Capability checks are precise and inspectable.
- A module can run under multiple profiles with different authority.
