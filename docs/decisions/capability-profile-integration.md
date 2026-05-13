# Capability Profile Integration

Canonical planning note:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/capability-profile-integration.md`

Implementation-facing decision:

- Capability profiles are runtime process authority descriptors. They contain a
  profile id, principal, allowed trust zones, granted capabilities, and explicit
  denied capabilities.
- Resource handles still carry narrowed grants. Profiles do not replace handle
  grants; effective authority requires both the handle and the active profile
  to allow the operation.
- Profile-aware resource operations now exist for opening handles, checking
  operations, executing adapters, delegating handles, and revoking handles.
- Opening a handle under a profile requires the profile principal to match the
  requested holder, the resource trust zone to be allowed, `resource/open`, and
  profile authority for every requested grant.
- Resource operation checks first validate the handle and resource, then check
  the profile before any adapter can run.
- Generic operation capabilities use `resource/open`, `resource/read`,
  `resource/write`, `resource/call`, `resource/stream`, `resource/inspect`,
  `resource/delegate`, `resource/close`, and `resource/revoke`. Resource
  schemas may still use domain-specific capabilities such as `qbbn/ask`; the
  profile checker accepts either the domain capability or the corresponding
  generic operation capability where appropriate.
- Capability-profile denials use `capability_denied` with structured resource
  diagnostics and the missing capability populated. Trust-zone failures remain
  `wrong_trust_zone`.
- Revocation is supervisor-shaped but can be profile-mediated when the profile
  has `resource/revoke` in the handle's trust zone.
- Initial synchronous host functions can declare an optional required
  capability and trust zone. When either is present, `VmSession`/`ModuleSession`
  require an active `CapabilityProfile` and deny before Rust callback invocation
  if the profile lacks the trust zone or capability.
- `CapabilityPolicy` is the first in-memory policy container. It stores
  registered profile fragments and can compose a new profile from existing
  fragments when all components share the same principal. Composition unions
  trust zones and capabilities, and explicit denied capabilities still override
  grants.
- `EmbeddedRuntime` owns a `CapabilityPolicy` and an inspectable runtime audit
  log. Profile composition, profile activation, host-authority denials during
  eval, and resource-open allow/deny decisions are visible in
  `EmbeddedRuntimeSnapshot`.

## Implementation Status

Implemented in this slice:

- `CapabilityProfile` in `crates/anvil-core/src/capability.rs`.
- Profile presets for read-only and agent-development resource work.
- `ResourceEffect::capability_name` for stable operation capability mapping.
- `ResourceDenialReason::CapabilityDenied`.
- `ResourceRegistry::{open_handle_with_profile, check_operation_with_profile,
  execute_operation_with_profile, delegate_handle_with_profile,
  revoke_handle_with_profile}`.
- Unit tests for profile open, profile denial before adapter execution,
  delegation denial, and revocation.
- `specs/capability_profiles.feature` covering the executable
  agent-facing profile contract.
- Host-call profile checks in `crates/anvil-core/src/vm.rs`, covered by
  `specs/host_functions.feature`.
- `CapabilityPolicy` profile composition in
  `crates/anvil-core/src/capability.rs`.
- Embedded profile composition and authority audit events in
  `crates/anvil-core/src/embedding.rs`, covered by
  `specs/embedding_contract.feature`.

Not implemented yet:

- Manifest-driven profile composition across packages, modules, roles, and
  resource policies.
- Budget, approval, and durable audit-sink integration beyond the current
  in-memory facade audit log.
- Full host-call/resource operation audit facets on `EvalResponse`, beyond the
  current embedded-runtime snapshot log.
- TypeScript, WASM, or transport facade profile registration.
- Persistent profile storage or policy editing.

## AX Notes

The important agent-facing behavior is that a profile denial is boring in the
best way: it says the missing capability, does not call the adapter, and keeps
the handle/resource diagnostic envelope consistent with ordinary resource
denials. This gives agents a clear repair path: request a narrower handle, run
under a stronger profile, or move the effect into an approved supervisor path.
