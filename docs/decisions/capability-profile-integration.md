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

Not implemented yet:

- Manifest/profile composition across packages, modules, principals, roles, and
  resource policies.
- Budget, approval, and audit-sink integration beyond current denial events.
- TypeScript, WASM, or transport facade profile registration.
- Persistent profile storage or policy editing.

## AX Notes

The important agent-facing behavior is that a profile denial is boring in the
best way: it says the missing capability, does not call the adapter, and keeps
the handle/resource diagnostic envelope consistent with ordinary resource
denials. This gives agents a clear repair path: request a narrower handle, run
under a stronger profile, or move the effect into an approved supervisor path.
