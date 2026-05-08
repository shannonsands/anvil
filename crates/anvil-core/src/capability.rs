use std::collections::BTreeSet;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityProfile {
    pub profile_id: String,
    pub principal: String,
    pub trust_zones: BTreeSet<String>,
    pub capabilities: BTreeSet<String>,
    pub denied_capabilities: BTreeSet<String>,
}

impl CapabilityProfile {
    pub fn new(
        profile_id: impl Into<String>,
        principal: impl Into<String>,
        trust_zone: impl Into<String>,
    ) -> Self {
        let mut profile = Self {
            profile_id: profile_id.into(),
            principal: principal.into(),
            trust_zones: BTreeSet::new(),
            capabilities: BTreeSet::new(),
            denied_capabilities: BTreeSet::new(),
        };
        profile.allow_trust_zone(trust_zone);
        profile
    }

    pub fn read_only(
        profile_id: impl Into<String>,
        principal: impl Into<String>,
        trust_zone: impl Into<String>,
    ) -> Self {
        Self::new(profile_id, principal, trust_zone).with_capabilities([
            "resource/open",
            "resource/read",
            "resource/inspect",
        ])
    }

    pub fn agent_dev(
        profile_id: impl Into<String>,
        principal: impl Into<String>,
        trust_zone: impl Into<String>,
    ) -> Self {
        Self::new(profile_id, principal, trust_zone).with_capabilities([
            "resource/open",
            "resource/read",
            "resource/write",
            "resource/call",
            "resource/stream",
            "resource/inspect",
            "resource/delegate",
            "resource/close",
        ])
    }

    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.allow_capability(capability);
        self
    }

    pub fn with_capabilities<I, S>(mut self, capabilities: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for capability in capabilities {
            self.allow_capability(capability);
        }
        self
    }

    pub fn with_denied_capability(mut self, capability: impl Into<String>) -> Self {
        self.deny_capability(capability);
        self
    }

    pub fn with_trust_zone(mut self, trust_zone: impl Into<String>) -> Self {
        self.allow_trust_zone(trust_zone);
        self
    }

    pub fn allow_capability(&mut self, capability: impl Into<String>) {
        self.capabilities.insert(capability.into());
    }

    pub fn deny_capability(&mut self, capability: impl Into<String>) {
        self.denied_capabilities.insert(capability.into());
    }

    pub fn allow_trust_zone(&mut self, trust_zone: impl Into<String>) {
        self.trust_zones.insert(trust_zone.into());
    }

    pub fn allows_capability(&self, capability: &str) -> bool {
        self.capabilities.contains(capability) && !self.denied_capabilities.contains(capability)
    }

    pub fn allows_any_capability<'capability>(
        &self,
        capabilities: impl IntoIterator<Item = &'capability str>,
    ) -> bool {
        capabilities
            .into_iter()
            .any(|capability| self.allows_capability(capability))
    }

    pub fn allows_trust_zone(&self, trust_zone: &str) -> bool {
        self.trust_zones.contains(trust_zone)
    }

    pub fn denies_capability(&self, capability: &str) -> bool {
        self.denied_capabilities.contains(capability)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_explicit_capabilities_inside_trust_zone() {
        let profile = CapabilityProfile::read_only("readonly", "agent.alpha", "project.markodb");

        assert!(profile.allows_trust_zone("project.markodb"));
        assert!(profile.allows_capability("resource/read"));
        assert!(!profile.allows_capability("resource/write"));
    }

    #[test]
    fn explicit_denials_override_grants() {
        let profile = CapabilityProfile::agent_dev("dev", "agent.alpha", "project.markodb")
            .with_denied_capability("resource/write");

        assert!(profile.denies_capability("resource/write"));
        assert!(!profile.allows_capability("resource/write"));
        assert!(profile.allows_capability("resource/read"));
    }
}
