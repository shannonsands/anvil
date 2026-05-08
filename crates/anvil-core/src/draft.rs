use serde::Serialize;

use crate::{
    diagnostic::Diagnostic,
    module::{ModuleCandidate, ModuleRootKind, ModuleSource},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DraftOverlay {
    pub id: String,
    pub owner: String,
    pub status: DraftStatus,
    pub modules: Vec<DraftModule>,
}

impl DraftOverlay {
    pub fn new(id: impl Into<String>, owner: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            owner: owner.into(),
            status: DraftStatus::Editing,
            modules: Vec::new(),
        }
    }

    pub fn with_status(mut self, status: DraftStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_module(mut self, module: impl Into<String>, source: impl Into<String>) -> Self {
        self.add_module(module, source);
        self
    }

    pub fn add_module(&mut self, module: impl Into<String>, source: impl Into<String>) {
        let module = module.into();
        let path = default_draft_module_path(&self.id, &module);
        self.modules.push(DraftModule {
            module,
            source: source.into(),
            path,
            base: None,
            diagnostics: Vec::new(),
        });
    }

    pub fn module_sources(&self) -> Vec<ModuleSource> {
        self.modules
            .iter()
            .map(|module| {
                ModuleSource::new(
                    ModuleRootKind::Draft,
                    self.id.clone(),
                    module.module.clone(),
                    module.path.clone(),
                )
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftStatus {
    Editing,
    ReadyForTest,
    Tested,
    Approved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DraftModule {
    pub module: String,
    pub source: String,
    pub path: String,
    pub base: Option<ModuleCandidate>,
    pub diagnostics: Vec<Diagnostic>,
}

fn default_draft_module_path(draft_id: &str, module: &str) -> String {
    format!(
        ".anvil/drafts/{draft_id}/src/{}.anv",
        module.replace('.', "/")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_draft_module_sources() {
        let overlay = DraftOverlay::new("session-1", "agent.alpha")
            .with_module("planner.search", "(define x 1)");
        let sources = overlay.module_sources();

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].root_kind, ModuleRootKind::Draft);
        assert_eq!(sources[0].root_name, "session-1");
        assert_eq!(sources[0].module, "planner.search");
        assert_eq!(
            sources[0].path,
            ".anvil/drafts/session-1/src/planner/search.anv"
        );
    }
}
