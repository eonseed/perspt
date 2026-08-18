//! Prompt stages (PSP-10 system 23's runtime types).

use serde::{Deserialize, Serialize};

/// The declared prompt stages. Platform stages compose the universal
/// envelope; domain stages compose the domain program layered on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptStage {
    // Platform stages.
    SessionBootstrap,
    GraphPlan,
    ToolDiscovery,
    ConversationCompact,
    ExternalReconcile,
    // Domain stages.
    RepositoryExplore,
    BranchStrategy,
    BranchPropose,
    BranchCorrect,
    BranchReview,
    Adjudicate,
    EvidenceSummarize,
}

impl PromptStage {
    /// The directory name a stage's section files live under.
    pub fn dir_name(&self) -> &'static str {
        match self {
            PromptStage::SessionBootstrap => "session_bootstrap",
            PromptStage::GraphPlan => "graph_plan",
            PromptStage::ToolDiscovery => "tool_discovery",
            PromptStage::ConversationCompact => "conversation_compact",
            PromptStage::ExternalReconcile => "external_reconcile",
            PromptStage::RepositoryExplore => "repository_explore",
            PromptStage::BranchStrategy => "branch_strategy",
            PromptStage::BranchPropose => "branch_propose",
            PromptStage::BranchCorrect => "branch_correct",
            PromptStage::BranchReview => "branch_review",
            PromptStage::Adjudicate => "adjudicate",
            PromptStage::EvidenceSummarize => "evidence_summarize",
        }
    }

    /// Parse a stage directory name.
    pub fn from_dir_name(name: &str) -> Option<Self> {
        [
            PromptStage::SessionBootstrap,
            PromptStage::GraphPlan,
            PromptStage::ToolDiscovery,
            PromptStage::ConversationCompact,
            PromptStage::ExternalReconcile,
            PromptStage::RepositoryExplore,
            PromptStage::BranchStrategy,
            PromptStage::BranchPropose,
            PromptStage::BranchCorrect,
            PromptStage::BranchReview,
            PromptStage::Adjudicate,
            PromptStage::EvidenceSummarize,
        ]
        .into_iter()
        .find(|stage| stage.dir_name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dir_names_round_trip() {
        for stage in [
            PromptStage::SessionBootstrap,
            PromptStage::GraphPlan,
            PromptStage::BranchCorrect,
            PromptStage::EvidenceSummarize,
        ] {
            assert_eq!(PromptStage::from_dir_name(stage.dir_name()), Some(stage));
        }
        assert_eq!(PromptStage::from_dir_name("nonsense"), None);
    }
}
