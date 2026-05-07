# Diagnostics And Agent Protocol

The canonical planning note is:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/diagnostics-and-agent-protocol.md`

Implementation-facing decision:

- All agent-facing runtime surfaces should share a small response envelope.
- Default responses should be concise: status, kind, id, summary, primary value
  when small, important notices, facet references, and next actions.
- Detailed data should be opt-in through verbosity levels or facet drill-down:
  diagnostics, spans, expansion trace, type trace, capability denials, effects,
  audit events, frames, tasks, artifacts, and suggestions.
- Different operations can return different facets, but the envelope should be
  stable across REPL, host API, tests, evals, debugger, and module workflows.
- Large traces and expansion output should be returned by id, not dumped into
  every response.

Open implementation dependency: reader/parser spans and result/facet lifetime
need concrete data structures before this becomes Rust API.
