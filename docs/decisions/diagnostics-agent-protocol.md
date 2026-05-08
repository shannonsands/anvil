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
- Reader, syntax, and module diagnostics now provide the first concrete Rust
  API shape for this protocol: source id, severity, phase, primary span, labels,
  expected/actual values, suggestions, and a source code frame.

Current diagnostic fields:

- `code`: stable machine-readable reason code.
- `severity`: currently `error`.
- `phase`: currently `reader`, `syntax`, `module`, `manifest`, `project`,
  `compile`, or `runtime`.
- `message`: concise human summary.
- `source_id`: source identity such as `repl`, `stdin`, or a future module id.
- `primary_span` and `span`: source span for compatibility and explicit
  primary-span access.
- `labels`: span labels for richer diagnostics and future related spans.
- `expected` and `actual`: compact repair context.
- `suggestion` and `suggestions`: text-compatible and structured repair hints.
- `code_frame`: one-line source frame for human output and agent repair context.

Open implementation dependency: later macro, type, capability, runtime, and
host diagnostics should reuse this shape rather than introducing parallel error
formats.
