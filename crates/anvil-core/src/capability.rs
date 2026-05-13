use std::collections::{BTreeMap, BTreeSet};

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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CapabilityPolicy {
    profiles: BTreeMap<String, CapabilityProfile>,
}

impl CapabilityPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_profile(&mut self, profile: CapabilityProfile) {
        self.profiles.insert(profile.profile_id.clone(), profile);
    }

    pub fn profile(&self, profile_id: &str) -> Option<&CapabilityProfile> {
        self.profiles.get(profile_id)
    }

    pub fn profiles(&self) -> impl Iterator<Item = &CapabilityProfile> {
        self.profiles.values()
    }

    pub fn profile_ids(&self) -> impl Iterator<Item = &String> {
        self.profiles.keys()
    }

    pub fn compose_profile<I, S>(
        &self,
        profile_id: impl Into<String>,
        component_ids: I,
    ) -> Result<CapabilityProfile, CapabilityPolicyError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let profile_id = profile_id.into();
        let component_ids = component_ids
            .into_iter()
            .map(|id| id.as_ref().to_string())
            .collect::<Vec<_>>();

        let first_id = component_ids.first().ok_or_else(|| CapabilityPolicyError {
            kind: CapabilityPolicyErrorKind::EmptyComposition,
            message: format!("capability profile {profile_id} needs components"),
            expected: vec!["one or more profile ids".to_string()],
            actual: None,
            suggestion: Some(
                "Compose from existing role/profile fragments or register a concrete profile."
                    .to_string(),
            ),
        })?;
        let first = self
            .profile(first_id)
            .ok_or_else(|| self.profile_not_found(first_id))?;

        let mut composed = CapabilityProfile {
            profile_id,
            principal: first.principal.clone(),
            trust_zones: BTreeSet::new(),
            capabilities: BTreeSet::new(),
            denied_capabilities: BTreeSet::new(),
        };

        for component_id in component_ids {
            let component = self
                .profile(&component_id)
                .ok_or_else(|| self.profile_not_found(&component_id))?;

            if component.principal != composed.principal {
                return Err(CapabilityPolicyError {
                    kind: CapabilityPolicyErrorKind::PrincipalMismatch,
                    message: format!(
                        "cannot compose profile {} for principal {} from component {} for principal {}",
                        composed.profile_id,
                        composed.principal,
                        component.profile_id,
                        component.principal
                    ),
                    expected: vec![format!("principal:{}", composed.principal)],
                    actual: Some(format!("principal:{}", component.principal)),
                    suggestion: Some(
                        "Compose profiles only within the same principal boundary.".to_string(),
                    ),
                });
            }

            composed
                .trust_zones
                .extend(component.trust_zones.iter().cloned());
            composed
                .capabilities
                .extend(component.capabilities.iter().cloned());
            composed
                .denied_capabilities
                .extend(component.denied_capabilities.iter().cloned());
        }

        for denied in composed.denied_capabilities.clone() {
            composed.capabilities.remove(&denied);
        }

        Ok(composed)
    }

    fn profile_not_found(&self, profile_id: &str) -> CapabilityPolicyError {
        CapabilityPolicyError {
            kind: CapabilityPolicyErrorKind::ProfileNotFound,
            message: format!("capability profile {profile_id} is not registered"),
            expected: self.profile_ids().cloned().collect(),
            actual: Some(profile_id.to_string()),
            suggestion: Some("Register the profile before composing it.".to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityPolicyError {
    pub kind: CapabilityPolicyErrorKind,
    pub message: String,
    pub expected: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityPolicyErrorKind {
    EmptyComposition,
    ProfileNotFound,
    PrincipalMismatch,
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

    #[test]
    fn policy_composes_profile_fragments_with_denials_winning() {
        let mut policy = CapabilityPolicy::new();
        policy.register_profile(
            CapabilityProfile::new("reader", "agent.alpha", "project.markodb")
                .with_capabilities(["resource/open", "resource/read", "resource/write"])
                .with_denied_capability("resource/write"),
        );
        policy.register_profile(
            CapabilityProfile::new("qbbn", "agent.alpha", "project.qbbn")
                .with_capability("qbbn/ask"),
        );

        let profile = policy
            .compose_profile("agent.alpha.composed", ["reader", "qbbn"])
            .expect("composed profile");

        assert_eq!(profile.profile_id, "agent.alpha.composed");
        assert!(profile.allows_trust_zone("project.markodb"));
        assert!(profile.allows_trust_zone("project.qbbn"));
        assert!(profile.allows_capability("resource/read"));
        assert!(profile.allows_capability("qbbn/ask"));
        assert!(profile.denies_capability("resource/write"));
        assert!(!profile.allows_capability("resource/write"));
    }

    #[test]
    fn policy_rejects_missing_or_cross_principal_composition() {
        let mut policy = CapabilityPolicy::new();
        policy.register_profile(CapabilityProfile::new(
            "alpha",
            "agent.alpha",
            "project.markodb",
        ));
        policy.register_profile(CapabilityProfile::new(
            "beta",
            "agent.beta",
            "project.markodb",
        ));

        let missing = policy
            .compose_profile("missing", ["alpha", "absent"])
            .expect_err("missing component");
        assert_eq!(missing.kind, CapabilityPolicyErrorKind::ProfileNotFound);
        assert_eq!(missing.actual.as_deref(), Some("absent"));

        let mismatch = policy
            .compose_profile("mixed", ["alpha", "beta"])
            .expect_err("cross-principal composition");
        assert_eq!(mismatch.kind, CapabilityPolicyErrorKind::PrincipalMismatch);

        let empty = policy
            .compose_profile("empty", Vec::<String>::new())
            .expect_err("empty composition");
        assert_eq!(empty.kind, CapabilityPolicyErrorKind::EmptyComposition);
    }
}
