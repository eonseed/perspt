//! SRBN Orchestrator
//!
//! Manages the Task DAG and orchestrates agent execution following the 7-step control loop.

mod bundle;
mod commit;
mod convergence;
mod init;
mod planning;
mod repair;
pub mod sdk_bridge;
mod solo;
mod verification;

mod branches;
mod events;
mod execute;
mod session;
mod speculate;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_workspace;

use crate::agent::{ActuatorAgent, Agent, ArchitectAgent, SpeculatorAgent, VerifierAgent};
use crate::context_retriever::ContextRetriever;
use crate::lsp::LspClient;
use crate::test_runner::{self};
use crate::tools::{AgentTools, ToolCall};
use crate::types::{AgentContext, EnergyComponents, ModelTier, NodeState, SRBNNode, TaskPlan};
use anyhow::{Context, Result};
use perspt_core::types::{
    EscalationCategory, EscalationReport, NodeClass, ProvisionalBranch, ProvisionalBranchState,
    RewriteAction, RewriteRecord, SheafValidationResult, SheafValidatorClass, WorkspaceState,
};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::{EdgeRef, Topo, Walker};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Dependency edge type
#[derive(Debug, Clone)]
pub struct Dependency {
    /// Dependency type description
    pub kind: String,
}

/// Result of an approval request
#[derive(Debug, Clone)]
pub enum ApprovalResult {
    /// User approved the action
    Approved,
    /// User approved with an edited value (e.g., project name)
    ApprovedWithEdit(String),
    /// User rejected the action
    Rejected,
}

/// Outcome of executing a single graph node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeOutcome {
    /// Node converged and committed successfully.
    Completed,
    /// Node failed to converge and was escalated (terminal).
    Escalated,
    /// A repair/rewrite was applied (node set back to Retry, or split/interface/
    /// replan inserted new work). The node is NOT terminal — the control loop
    /// should re-evaluate the graph and re-run the affected node(s) next round.
    Reworked,
}

/// Decision taken when the control loop "settles" (no runnable node remains).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettleDecision {
    /// The user's overall goal is judged achieved — finish with success.
    Achieved,
    /// The plan was amended (new nodes queued) — keep looping.
    Replanned,
    /// Stop the loop (goal not achievable within bounds, or not in auto mode).
    Stop,
}

/// Bookkeeping for the goal-driven re-plan loop, threaded through the control
/// loop so no orchestrator field is needed. Bounds re-planning by a revision
/// count and a progress (Φ / completed-count) non-regression check.
#[derive(Debug, Clone)]
pub(crate) struct ReplanState {
    /// Number of architect amendments applied so far.
    count: usize,
    /// Maximum amendments allowed (from the FeatureCharter revision budget).
    max: usize,
    /// Completed-node count at the previous re-plan, to detect non-progress.
    last_completed: usize,
    /// Workflow potential Φ at the previous settle, to detect non-regression.
    last_phi: Option<f64>,
}

impl ReplanState {
    fn new(max: usize) -> Self {
        Self {
            count: 0,
            max,
            last_completed: 0,
            last_phi: None,
        }
    }
}

/// Tolerantly parse a [`perspt_core::types::GoalVerdict`] from a model response
/// by extracting the outermost `{ ... }` JSON object. Returns `None` if no valid
/// object is found.
fn parse_goal_verdict(resp: &str) -> Option<perspt_core::types::GoalVerdict> {
    let start = resp.find('{')?;
    let end = resp.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&resp[start..=end]).ok()
}

/// The SRBN Orchestrator - manages the agent workflow
pub struct SRBNOrchestrator {
    /// Task DAG managed by petgraph
    pub graph: DiGraph<SRBNNode, Dependency>,
    /// Node ID to graph index mapping
    node_indices: HashMap<String, NodeIndex>,
    /// Agent context
    pub context: AgentContext,
    /// Auto-approve mode
    pub auto_approve: bool,
    /// LSP clients per language
    lsp_clients: HashMap<String, LspClient>,
    /// Agents for different roles
    agents: Vec<Box<dyn Agent>>,
    /// Agent tools for file/command operations
    tools: AgentTools,
    /// Last written file path (for LSP tracking)
    last_written_file: Option<PathBuf>,
    /// File version counter for LSP
    file_version: i32,
    /// LLM provider for correction calls
    provider: std::sync::Arc<perspt_core::llm_provider::GenAIProvider>,
    /// Architect model name for planning
    architect_model: String,
    /// Actuator model name for corrections
    actuator_model: String,
    /// Verifier model name for correction guidance
    verifier_model: String,
    /// Speculator model name for lookahead hints
    speculator_model: String,
    /// PSP-5: Fallback model for Architect tier (used when primary fails structured-output contract)
    architect_fallback_model: Option<String>,
    /// PSP-5: Fallback model for Actuator tier
    actuator_fallback_model: Option<String>,
    /// PSP-5: Fallback model for Verifier tier
    verifier_fallback_model: Option<String>,
    /// PSP-5: Fallback model for Speculator tier
    speculator_fallback_model: Option<String>,
    /// Event sender for TUI updates (optional)
    event_sender: Option<perspt_core::events::channel::EventSender>,
    /// Action receiver for TUI commands (optional)
    action_receiver: Option<perspt_core::events::channel::ActionReceiver>,
    /// Persistence ledger
    pub ledger: crate::ledger::MerkleLedger,
    /// Last tool failure message (for energy calculation)
    pub last_tool_failure: Option<String>,
    /// PSP-5 Phase 3: Last assembled context provenance (for commit recording)
    last_context_provenance: Option<perspt_core::types::ContextProvenance>,
    /// PSP-5 Phase 3: Last formatted context from restriction map (for correction prompts)
    last_formatted_context: String,
    /// PSP-5 Phase 4: Last plugin-driven verification result (for convergence checks)
    last_verification_result: Option<perspt_core::types::VerificationResult>,
    /// PSP-8: SDK measured-gate bridge. Translates verification results into the
    /// canonical quadratic energy `V = sum_e w_e r_e^2` and runs the SDK
    /// acceptance gate alongside the StabilityMonitor for telemetry.
    sdk_gate: sdk_bridge::SdkGateState,
    /// PSP-5 Phase 9: Last applied artifact bundle (for persistence in step_commit)
    last_applied_bundle: Option<perspt_core::types::ArtifactBundle>,
    /// Last recorded RepairFootprint (for multi-file correction context)
    last_repair_footprint: Option<perspt_core::RepairFootprint>,
    /// PSP-5 Phase 6: Blocked dependencies awaiting parent interface seals
    blocked_dependencies: Vec<perspt_core::types::BlockedDependency>,
    /// Session-level budget envelope for step/cost/revision caps.
    budget: perspt_core::types::BudgetEnvelope,
    /// Adaptive planning policy for agent phase selection.
    pub planning_policy: perspt_core::PlanningPolicy,
    /// Preferred package manager for greenfield project init. Plugin-driven:
    /// fed verbatim into `InitOptions.package_manager`; each language plugin maps
    /// it to its own init command and default (Python → uv, JS → npm).
    pub package_manager: Option<String>,
    /// Session-level stability threshold (ε for V(x) < ε convergence)
    pub stability_epsilon: f32,
    /// Energy weight α (syntax/build errors)
    pub energy_alpha: f32,
    /// Energy weight β (structural concerns)
    pub energy_beta: f32,
    /// Energy weight γ (test/lint failures)
    pub energy_gamma: f32,
    /// Session abort flag — set by external signal handlers or TUI
    abort_requested: Arc<AtomicBool>,
}

/// Get current timestamp as epoch seconds.
fn epoch_seconds() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Detect stub/placeholder content in a generated source file.
///
/// Returns `Some(reason)` if the file is predominantly stub content (i.e. it
/// contains a known stub pattern AND has fewer than 5 lines of real code).
/// Returns `None` for files that contain a real implementation.
///
/// Language detection uses `plugin_hint` ("rust", "python", "javascript") with
/// a fallback to file extension so this works for any project type.
fn detect_stub_content(path: &std::path::Path, plugin_hint: &str) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;

    // Determine language from plugin hint or file extension.
    let lang = if !plugin_hint.is_empty() && plugin_hint != "unknown" {
        plugin_hint.to_ascii_lowercase()
    } else {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| match e {
                "rs" => "rust",
                "py" => "python",
                "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" => "javascript",
                _ => "",
            })
            .unwrap_or("")
            .to_string()
    };

    // Universal stub markers (case-insensitive substring match).
    let universal_patterns = [
        "// stub",
        "# stub",
        "// placeholder",
        "# placeholder",
        "// will be replaced",
        "# will be replaced",
        "/* todo */",
    ];

    // Language-specific stub patterns.
    let lang_patterns: &[&str] = match lang.as_str() {
        "rust" => &["todo!()", "unimplemented!()"],
        "python" => &["raise NotImplementedError", "raise NotImplementedError()"],
        "javascript" | "typescript" => &[
            "throw new Error(\"not implemented\")",
            "throw new Error('not implemented')",
            "throw new Error(\"TODO\")",
            "throw new Error('TODO')",
        ],
        _ => &[],
    };

    let content_lower = content.to_ascii_lowercase();

    // Check for any matching stub pattern.
    let mut matched_pattern = None;
    for pat in &universal_patterns {
        if content_lower.contains(pat) {
            matched_pattern = Some(*pat);
            break;
        }
    }
    if matched_pattern.is_none() {
        for pat in lang_patterns {
            if content.contains(pat) {
                matched_pattern = Some(*pat);
                break;
            }
        }
    }

    // Python-specific: detect `pass` or `...` as sole function/class body.
    if matched_pattern.is_none() && lang == "python" {
        let trimmed_lines: Vec<&str> = content
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        let body_only: Vec<&&str> = trimmed_lines
            .iter()
            .filter(|l| {
                !l.starts_with("def ")
                    && !l.starts_with("class ")
                    && !l.starts_with("import ")
                    && !l.starts_with("from ")
            })
            .collect();
        if body_only.len() <= 2 && body_only.iter().all(|l| **l == "pass" || **l == "...") {
            matched_pattern = Some("only pass/... body");
        }
    }

    let pattern = matched_pattern?;

    // Count real code lines: non-blank, non-comment, non-import.
    let real_lines = count_real_code_lines(&content, &lang);
    if real_lines >= 5 {
        // File has enough real code — a single stub marker inside a large
        // implementation is acceptable (e.g. a todo!() in one branch).
        return None;
    }

    Some(format!(
        "found '{}' with only {} line(s) of real code",
        pattern, real_lines
    ))
}

/// Count non-blank, non-comment, non-import lines of code.
fn count_real_code_lines(content: &str, lang: &str) -> usize {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return false;
            }
            // Skip comments.
            match lang {
                "rust" => {
                    if trimmed.starts_with("//")
                        || trimmed.starts_with("/*")
                        || trimmed.starts_with('*')
                    {
                        return false;
                    }
                    // Skip use/extern/mod declarations (imports).
                    if trimmed.starts_with("use ")
                        || trimmed.starts_with("extern ")
                        || trimmed.starts_with("mod ")
                    {
                        return false;
                    }
                }
                "python" => {
                    if trimmed.starts_with('#')
                        || trimmed.starts_with("\"\"\"")
                        || trimmed.starts_with("'''")
                    {
                        return false;
                    }
                    if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
                        return false;
                    }
                }
                "javascript" | "typescript" => {
                    if trimmed.starts_with("//")
                        || trimmed.starts_with("/*")
                        || trimmed.starts_with('*')
                    {
                        return false;
                    }
                    if trimmed.starts_with("import ")
                        || trimmed.starts_with("require(")
                        || trimmed.starts_with("const ") && trimmed.contains("require(")
                    {
                        return false;
                    }
                }
                _ => {
                    if trimmed.starts_with("//")
                        || trimmed.starts_with('#')
                        || trimmed.starts_with("/*")
                    {
                        return false;
                    }
                }
            }
            true
        })
        .count()
}

impl SRBNOrchestrator {
    /// Create a new orchestrator with default models
    pub fn new(working_dir: PathBuf, auto_approve: bool) -> Self {
        Self::new_with_models(
            working_dir,
            auto_approve,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
    }

    /// Create a new orchestrator with custom model configuration
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_models(
        working_dir: PathBuf,
        auto_approve: bool,
        architect_model: Option<String>,
        actuator_model: Option<String>,
        verifier_model: Option<String>,
        speculator_model: Option<String>,
        architect_fallback_model: Option<String>,
        actuator_fallback_model: Option<String>,
        verifier_fallback_model: Option<String>,
        speculator_fallback_model: Option<String>,
    ) -> Self {
        // Create a shared LLM provider - agents will use this for LLM calls.
        let provider = std::sync::Arc::new(
            perspt_core::llm_provider::GenAIProvider::new().unwrap_or_else(|e| {
                log::warn!("Failed to create GenAIProvider: {}, using default", e);
                perspt_core::llm_provider::GenAIProvider::new().expect("GenAI must initialize")
            }),
        );

        Self::new_with_models_and_provider(
            working_dir,
            auto_approve,
            provider,
            architect_model,
            actuator_model,
            verifier_model,
            speculator_model,
            architect_fallback_model,
            actuator_fallback_model,
            verifier_fallback_model,
            speculator_fallback_model,
        )
    }

    /// Create a new orchestrator with custom models and an injected provider.
    ///
    /// The injected provider should already be bound to the resolved adapter
    /// (e.g. via `GenAIProvider::from_config`) so custom/local model names route
    /// correctly. The bound adapter is only the fallback, so the four tiers may
    /// still use recognized provider model names that route by prefix.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_models_and_provider(
        working_dir: PathBuf,
        auto_approve: bool,
        provider: std::sync::Arc<perspt_core::llm_provider::GenAIProvider>,
        architect_model: Option<String>,
        actuator_model: Option<String>,
        verifier_model: Option<String>,
        speculator_model: Option<String>,
        architect_fallback_model: Option<String>,
        actuator_fallback_model: Option<String>,
        verifier_fallback_model: Option<String>,
        speculator_fallback_model: Option<String>,
    ) -> Self {
        let context = AgentContext {
            working_dir: working_dir.clone(),
            auto_approve,
            ..Default::default()
        };

        // Create agent tools for file/command operations
        let tools = AgentTools::new(working_dir.clone(), !auto_approve);

        // Store model names for direct LLM calls
        let stored_architect_model = architect_model
            .clone()
            .unwrap_or_else(|| ModelTier::Architect.default_model().to_string());
        let stored_actuator_model = actuator_model
            .clone()
            .unwrap_or_else(|| ModelTier::Actuator.default_model().to_string());
        let stored_verifier_model = verifier_model
            .clone()
            .unwrap_or_else(|| ModelTier::Verifier.default_model().to_string());
        let stored_speculator_model = speculator_model
            .clone()
            .unwrap_or_else(|| ModelTier::Speculator.default_model().to_string());

        Self {
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
            context,
            auto_approve,
            lsp_clients: HashMap::new(),
            agents: vec![
                Box::new(ArchitectAgent::new(provider.clone(), architect_model)),
                Box::new(ActuatorAgent::new(provider.clone(), actuator_model)),
                Box::new(VerifierAgent::new(provider.clone(), verifier_model)),
                Box::new(SpeculatorAgent::new(provider.clone(), speculator_model)),
            ],
            tools,
            last_written_file: None,
            file_version: 0,
            provider,
            architect_model: stored_architect_model,
            actuator_model: stored_actuator_model,
            verifier_model: stored_verifier_model,
            speculator_model: stored_speculator_model,
            architect_fallback_model,
            actuator_fallback_model,
            verifier_fallback_model,
            speculator_fallback_model,
            event_sender: None,
            action_receiver: None,
            #[cfg(test)]
            ledger: crate::ledger::MerkleLedger::in_memory().expect("Failed to create test ledger"),
            #[cfg(not(test))]
            ledger: crate::ledger::MerkleLedger::new().expect("Failed to create ledger"),
            last_tool_failure: None,
            last_context_provenance: None,
            last_formatted_context: String::new(),
            last_verification_result: None,
            sdk_gate: sdk_bridge::SdkGateState::new(),
            last_applied_bundle: None,
            last_repair_footprint: None,
            blocked_dependencies: Vec::new(),
            budget: perspt_core::types::BudgetEnvelope::new("pending"),
            planning_policy: perspt_core::PlanningPolicy::default(),
            package_manager: None,
            stability_epsilon: 0.1,
            energy_alpha: 1.0,
            energy_beta: 0.5,
            energy_gamma: 2.0,
            abort_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a new orchestrator for testing with an in-memory ledger
    #[cfg(test)]
    pub fn new_for_testing(working_dir: PathBuf) -> Self {
        let context = AgentContext {
            working_dir: working_dir.clone(),
            auto_approve: true,
            ..Default::default()
        };

        let provider = std::sync::Arc::new(
            perspt_core::llm_provider::GenAIProvider::new().unwrap_or_else(|e| {
                log::warn!("Failed to create GenAIProvider: {}, using default", e);
                perspt_core::llm_provider::GenAIProvider::new().expect("GenAI must initialize")
            }),
        );

        let tools = AgentTools::new(working_dir.clone(), false);

        Self {
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
            context,
            auto_approve: true,
            lsp_clients: HashMap::new(),
            agents: vec![
                Box::new(ArchitectAgent::new(provider.clone(), None)),
                Box::new(ActuatorAgent::new(provider.clone(), None)),
                Box::new(VerifierAgent::new(provider.clone(), None)),
                Box::new(SpeculatorAgent::new(provider.clone(), None)),
            ],
            tools,
            last_written_file: None,
            file_version: 0,
            provider,
            architect_model: ModelTier::Architect.default_model().to_string(),
            actuator_model: ModelTier::Actuator.default_model().to_string(),
            verifier_model: ModelTier::Verifier.default_model().to_string(),
            speculator_model: ModelTier::Speculator.default_model().to_string(),
            architect_fallback_model: None,
            actuator_fallback_model: None,
            verifier_fallback_model: None,
            speculator_fallback_model: None,
            event_sender: None,
            action_receiver: None,
            ledger: crate::ledger::MerkleLedger::in_memory().expect("Failed to create test ledger"),
            last_tool_failure: None,
            last_context_provenance: None,
            last_formatted_context: String::new(),
            last_verification_result: None,
            sdk_gate: sdk_bridge::SdkGateState::new(),
            last_applied_bundle: None,
            last_repair_footprint: None,
            blocked_dependencies: Vec::new(),
            budget: perspt_core::types::BudgetEnvelope::new("test"),
            planning_policy: perspt_core::PlanningPolicy::default(),
            package_manager: None,
            stability_epsilon: 0.1,
            energy_alpha: 1.0,
            energy_beta: 0.5,
            energy_gamma: 2.0,
            abort_requested: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// Parse a persisted state string back into a NodeState enum
fn parse_node_state(s: &str) -> NodeState {
    NodeState::from_display_str(s)
}

/// Parse a persisted node class string back into a NodeClass enum
fn parse_node_class(s: &str) -> NodeClass {
    match s {
        "Interface" => NodeClass::Interface,
        "Implementation" => NodeClass::Implementation,
        "Integration" => NodeClass::Integration,
        _ => NodeClass::default(),
    }
}
