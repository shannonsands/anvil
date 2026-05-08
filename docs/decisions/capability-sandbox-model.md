# Capability And Sandbox Model

The canonical planning note is:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/capability-and-sandbox-model.md`

Implementation-facing decision:

- Sandbox Anvil language processes inside the runtime itself.
- Treat the model like WASM imports and opaque handles applied to VM processes:
  no ambient host authority, explicit resource imports, bounded execution,
  precise denials, revocation, and audit.
- Do not make Docker, Firecracker, OS jails, or other external sandboxes part of
  the core design. They may be deployment layers later, but the runtime must be
  coherent without them.
- Every process has principal, trust zone, profile, concrete capabilities,
  imports, handle table, budgets, module generations, private stack/frames,
  mailbox, and audit stream.
- Effective authority is the intersection of runtime policy, principal/group,
  trust zone, profile, module manifest, resource policy, delegation, approvals,
  budgets, and revocation state.
- Native Rust host adapters are trusted runtime code. Keep them small, typed,
  capability-aware, and audited.

Open implementation dependency: the value/heap/GC direction is drafted in
`docs/decisions/value-heap-gc.md`; resource handles and checked adapter
dispatch are now drafted in `docs/decisions/resource-handles.md` and
`docs/decisions/resource-adapter-execution.md`. The next dependency is concrete
capability-profile integration for open/use/delegate/revoke checks.
