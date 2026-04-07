//! SRBN Types
//!
//! Core types for the Stabilized Recursive Barrier Network.
//! Based on PSP-000004 and PSP-000005 specifications.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

/// Model tier for different agent roles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelTier {
    /// Deep reasoning model for planning and architecture
    Architect,
    /// Fast coding model for implementation
    Actuator,
    /// Sensor for LSP + Contract checking
    Verifier,
    /// Fast lookahead for speculation
    Speculator,
}

impl ModelTier {
    /// Get the recommended default model for this tier.
    ///
    /// Architect and Verifier tiers prefer higher-capability models for
    /// reasoning and evaluation. Actuator and Speculator default to the
    /// faster lower-cost Gemini tier. All defaults can be overridden per-tier
    /// via CLI.
    pub fn default_model(&self) -> &'static str {
        match self {
            ModelTier::Architect => "gemini-3.1-pro-preview",
            ModelTier::Verifier => "gemini-3.1-pro-preview",
            ModelTier::Actuator => "gemini-3.1-flash-lite-preview",
            ModelTier::Speculator => "gemini-3.1-flash-lite-preview",
        }
    }

    /// Get the default model name (static, for use when no instance is available).
    /// Returns the Actuator default as the general-purpose fallback.
    pub fn default_model_name() -> &'static str {
        "gemini-3.1-flash-lite-preview"
    }
}

#[cfg(test)]
mod tests {
    use super::ModelTier;

    #[test]
    fn gemini_defaults_use_requested_latest_models() {
        assert_eq!(
            ModelTier::Architect.default_model(),
            "gemini-3.1-pro-preview"
        );
        assert_eq!(
            ModelTier::Verifier.default_model(),
            "gemini-3.1-pro-preview"
        );
        assert_eq!(
            ModelTier::Actuator.default_model(),
            "gemini-3.1-flash-lite-preview"
        );
        assert_eq!(
            ModelTier::Speculator.default_model(),
            "gemini-3.1-flash-lite-preview"
        );
        assert_eq!(
            ModelTier::default_model_name(),
            "gemini-3.1-flash-lite-preview"
        );
    }
}

/// Test criticality levels for weighted tests
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Criticality {
    /// Critical tests - highest energy penalty on failure
    Critical,
    /// High priority tests
    High,
    /// Low priority tests
    Low,
}

impl Criticality {
    /// Get the energy weight multiplier
    pub fn weight(&self) -> f32 {
        match self {
            Criticality::Critical => 10.0,
            Criticality::High => 3.0,
            Criticality::Low => 1.0,
        }
    }
}

/// Weighted test definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightedTest {
    /// Test name or pattern
    pub test_name: String,
    /// Criticality level
    pub criticality: Criticality,
}

/// Behavioral contract for a node
///
/// Defines the constraints and expectations for an SRBN node.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BehavioralContract {
    /// Required public API signature (hard constraint)
    pub interface_signature: String,
    /// Semantic constraints (e.g., "Use RS256 algorithm")
    pub invariants: Vec<String>,
    /// Anti-patterns to reject (e.g., "no unwrap()")
    pub forbidden_patterns: Vec<String>,
    /// Weighted test cases
    pub weighted_tests: Vec<WeightedTest>,
    /// Energy weights (alpha, beta, gamma) for V(x) calculation
    /// Default: (1.0, 0.5, 2.0) - Logic failures weighted highest
    pub energy_weights: (f32, f32, f32),
}

impl BehavioralContract {
    /// Create a new contract with default weights
    pub fn new() -> Self {
        Self {
            interface_signature: String::new(),
            invariants: Vec::new(),
            forbidden_patterns: Vec::new(),
            weighted_tests: Vec::new(),
            energy_weights: (1.0, 0.5, 2.0), // alpha, beta, gamma from PSP
        }
    }

    /// Get the alpha weight (syntactic energy)
    pub fn alpha(&self) -> f32 {
        self.energy_weights.0
    }

    /// Get the beta weight (structural energy)
    pub fn beta(&self) -> f32 {
        self.energy_weights.1
    }

    /// Get the gamma weight (logic energy)
    pub fn gamma(&self) -> f32 {
        self.energy_weights.2
    }
}

/// Error type for determining retry limits per PSP-4
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ErrorType {
    /// Compilation/syntax/type errors (3 attempts)
    #[default]
    Compilation,
    /// Tool execution failures (5 attempts)
    ToolFailure,
    /// User/reviewer rejection (3 rejections)
    ReviewRejection,
    /// Unknown/other errors (3 attempts default)
    Other,
}

/// Retry policy configuration per PSP-4 specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Max retries for compilation errors (default: 3)
    pub max_compilation_retries: usize,
    /// Max retries for tool failures (default: 5)
    pub max_tool_retries: usize,
    /// Max reviewer rejections before escalation (default: 3)
    pub max_review_rejections: usize,
    /// Current consecutive failures by type
    pub compilation_failures: usize,
    pub tool_failures: usize,
    pub review_rejections: usize,
    /// Last error type encountered
    pub last_error_type: Option<ErrorType>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            // PSP-4 specified limits
            max_compilation_retries: 3,
            max_tool_retries: 5,
            max_review_rejections: 3,
            compilation_failures: 0,
            tool_failures: 0,
            review_rejections: 0,
            last_error_type: None,
        }
    }
}

impl RetryPolicy {
    /// Record a failure of a specific type
    pub fn record_failure(&mut self, error_type: ErrorType) {
        self.last_error_type = Some(error_type);
        match error_type {
            ErrorType::Compilation => self.compilation_failures += 1,
            ErrorType::ToolFailure => self.tool_failures += 1,
            ErrorType::ReviewRejection => self.review_rejections += 1,
            ErrorType::Other => self.compilation_failures += 1, // Treat as compilation
        }
    }

    /// Check if we should escalate for a specific error type
    pub fn should_escalate(&self, error_type: ErrorType) -> bool {
        match error_type {
            ErrorType::Compilation | ErrorType::Other => {
                self.compilation_failures >= self.max_compilation_retries
            }
            ErrorType::ToolFailure => self.tool_failures >= self.max_tool_retries,
            ErrorType::ReviewRejection => self.review_rejections >= self.max_review_rejections,
        }
    }

    /// Check if any error type has exceeded its limit
    pub fn any_exceeded(&self) -> bool {
        self.compilation_failures >= self.max_compilation_retries
            || self.tool_failures >= self.max_tool_retries
            || self.review_rejections >= self.max_review_rejections
    }

    /// Get remaining attempts for an error type
    pub fn remaining_attempts(&self, error_type: ErrorType) -> usize {
        match error_type {
            ErrorType::Compilation | ErrorType::Other => self
                .max_compilation_retries
                .saturating_sub(self.compilation_failures),
            ErrorType::ToolFailure => self.max_tool_retries.saturating_sub(self.tool_failures),
            ErrorType::ReviewRejection => self
                .max_review_rejections
                .saturating_sub(self.review_rejections),
        }
    }

    /// Get a formatted summary
    pub fn summary(&self) -> String {
        format!(
            "Retries: comp {}/{}, tool {}/{}, review {}/{}",
            self.compilation_failures,
            self.max_compilation_retries,
            self.tool_failures,
            self.max_tool_retries,
            self.review_rejections,
            self.max_review_rejections
        )
    }
}

/// Stability monitor for tracking Lyapunov Energy
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StabilityMonitor {
    /// History of V(x) values
    pub energy_history: Vec<f32>,
    /// Number of convergence attempts
    pub attempt_count: usize,
    /// Whether the node has converged to stability
    pub stable: bool,
    /// Stability threshold (epsilon)
    pub stability_epsilon: f32,
    /// Maximum retry attempts before escalation (legacy, use retry_policy)
    pub max_retries: usize,
    /// Retry policy with PSP-4 compliant limits
    pub retry_policy: RetryPolicy,
}

impl StabilityMonitor {
    /// Create with default epsilon = 0.1
    pub fn new() -> Self {
        Self {
            energy_history: Vec::new(),
            attempt_count: 0,
            stable: false,
            stability_epsilon: 0.1,
            max_retries: 3,
            retry_policy: RetryPolicy::default(),
        }
    }

    /// Record a new energy value
    pub fn record_energy(&mut self, energy: f32) {
        self.energy_history.push(energy);
        self.attempt_count += 1;
        self.stable = energy < self.stability_epsilon;
    }

    /// Record a failure with error type
    pub fn record_failure(&mut self, error_type: ErrorType) {
        self.retry_policy.record_failure(error_type);
    }

    /// Check if we should escalate (exceeded retries without stability)
    pub fn should_escalate(&self) -> bool {
        // Legacy check or new policy check
        (self.attempt_count >= self.max_retries && !self.stable) || self.retry_policy.any_exceeded()
    }

    /// Get remaining attempts for current error type
    pub fn remaining_attempts(&self) -> usize {
        match self.retry_policy.last_error_type {
            Some(et) => self.retry_policy.remaining_attempts(et),
            None => self.max_retries.saturating_sub(self.attempt_count),
        }
    }

    /// Get the current energy level (last recorded)
    pub fn current_energy(&self) -> f32 {
        self.energy_history.last().copied().unwrap_or(f32::INFINITY)
    }

    /// Check if energy is decreasing (converging)
    pub fn is_converging(&self) -> bool {
        if self.energy_history.len() < 2 {
            return true; // Not enough data
        }
        let last = self.energy_history.last().unwrap();
        let prev = &self.energy_history[self.energy_history.len() - 2];
        last < prev
    }

    /// Reset monitor state for a subgraph replan, preserving history but
    /// clearing attempt count and stability flag so the node can be retried.
    pub fn reset_for_replan(&mut self) {
        self.attempt_count = 0;
        self.stable = false;
        self.retry_policy = RetryPolicy::default();
    }
}

/// SRBN Node - the fundamental unit of control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SRBNNode {
    /// Unique node identifier
    pub node_id: String,
    /// High-level goal description for LLM reasoning
    pub goal: String,
    /// Files the LLM MUST read for context
    pub context_files: Vec<PathBuf>,
    /// Files the LLM MUST modify
    pub output_targets: Vec<PathBuf>,
    /// Behavioral contract defining constraints
    pub contract: BehavioralContract,
    /// Model tier for this node
    pub tier: ModelTier,
    /// Stability monitor
    pub monitor: StabilityMonitor,
    /// Current state
    pub state: NodeState,
    /// Parent node ID (for DAG structure)
    pub parent_id: Option<String>,
    /// Child node IDs
    pub children: Vec<String>,
    /// PSP-5 Phase 2: Node class (Interface / Implementation / Integration)
    pub node_class: NodeClass,
    /// PSP-5 Phase 2: The language plugin that owns this node's files
    pub owner_plugin: String,
    /// PSP-5 Phase 6: Provisional branch ID if this node is executing speculatively
    pub provisional_branch_id: Option<String>,
    /// PSP-5 Phase 6: Interface seal hash once this node's public interface is sealed
    pub interface_seal_hash: Option<[u8; 32]>,
    /// Declared dependency expectations from the architect plan.
    pub dependency_expectations: DependencyExpectation,
}

impl SRBNNode {
    /// Create a new node with the given goal
    pub fn new(node_id: String, goal: String, tier: ModelTier) -> Self {
        Self {
            node_id,
            goal,
            context_files: Vec::new(),
            output_targets: Vec::new(),
            contract: BehavioralContract::new(),
            tier,
            monitor: StabilityMonitor::new(),
            state: NodeState::TaskQueued,
            parent_id: None,
            children: Vec::new(),
            node_class: NodeClass::default(),
            owner_plugin: String::new(),
            provisional_branch_id: None,
            interface_seal_hash: None,
            dependency_expectations: DependencyExpectation::default(),
        }
    }
}

/// Outcome of a full orchestration session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionOutcome {
    /// All nodes completed successfully
    Success,
    /// Some nodes completed, some escalated or failed
    PartialSuccess,
    /// Critical failure or all nodes escalated/failed
    Failed,
}

/// Node execution state (from PSP state machine)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    /// Task is queued for execution
    TaskQueued,
    /// Planning phase
    Planning,
    /// Coding/implementation phase
    Coding,
    /// Verification phase (LSP + Tests)
    Verifying,
    /// Retry loop (convergence)
    Retry,
    /// Sheaf consistency check
    SheafCheck,
    /// Committing stable state
    Committing,
    /// Escalated to user
    Escalated,
    /// Successfully completed
    Completed,
    /// Failed after max retries
    Failed,
    /// Aborted by user
    Aborted,
    /// Superseded by a plan amendment (Phase 14)
    Superseded,
}

impl NodeState {
    /// Check if this is a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            NodeState::Completed | NodeState::Failed | NodeState::Aborted | NodeState::Superseded
        )
    }

    /// Check if the node finished successfully
    pub fn is_success(&self) -> bool {
        matches!(self, NodeState::Completed)
    }

    /// Check if the node is actively running (non-terminal, non-queued)
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            NodeState::Planning
                | NodeState::Coding
                | NodeState::Verifying
                | NodeState::Retry
                | NodeState::SheafCheck
                | NodeState::Committing
        )
    }

    /// Parse a state string from the database or display layer.
    ///
    /// Handles PascalCase, UPPERCASE, and lowercase variants that appear in
    /// the store, CLI, and dashboard.  Unknown strings map to `TaskQueued`.
    pub fn from_display_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "taskqueued" | "queued" | "task_queued" => NodeState::TaskQueued,
            "planning" => NodeState::Planning,
            "coding" | "in_progress" | "in-progress" | "running" => NodeState::Coding,
            "verifying" => NodeState::Verifying,
            "retry" | "retrying" => NodeState::Retry,
            "sheafcheck" | "sheaf_check" => NodeState::SheafCheck,
            "committing" | "committed" => NodeState::Committing,
            "escalated" => NodeState::Escalated,
            "completed" | "stable" | "verified" => NodeState::Completed,
            "failed" | "error" => NodeState::Failed,
            "aborted" => NodeState::Aborted,
            "superseded" => NodeState::Superseded,
            _ => NodeState::TaskQueued,
        }
    }
}

impl std::fmt::Display for NodeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            NodeState::TaskQueued => "queued",
            NodeState::Planning => "planning",
            NodeState::Coding => "coding",
            NodeState::Verifying => "verifying",
            NodeState::Retry => "retrying",
            NodeState::SheafCheck => "sheaf_check",
            NodeState::Committing => "committing",
            NodeState::Escalated => "escalated",
            NodeState::Completed => "completed",
            NodeState::Failed => "failed",
            NodeState::Aborted => "aborted",
            NodeState::Superseded => "superseded",
        };
        f.write_str(label)
    }
}

/// Token budget tracking for cost control
///
/// Tracks input/output token usage and enforces limits per PSP-4 --max-cost.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudget {
    /// Maximum total tokens allowed (input + output)
    pub max_tokens: usize,
    /// Maximum cost in dollars (optional)
    pub max_cost_usd: Option<f64>,
    /// Input tokens used
    pub input_tokens_used: usize,
    /// Output tokens used
    pub output_tokens_used: usize,
    /// Estimated cost so far (in USD)
    pub cost_usd: f64,
    /// Cost per 1K input tokens (varies by model)
    pub input_cost_per_1k: f64,
    /// Cost per 1K output tokens (varies by model)
    pub output_cost_per_1k: f64,
}

impl Default for TokenBudget {
    fn default() -> Self {
        Self {
            max_tokens: 100_000, // 100K default (PSP-4 mentions 100k+ context)
            max_cost_usd: None,  // No cost limit by default
            input_tokens_used: 0,
            output_tokens_used: 0,
            cost_usd: 0.0,
            // Default to Gemini Flash pricing (roughly)
            input_cost_per_1k: 0.075 / 1000.0, // $0.075 per 1M = $0.000075 per 1K
            output_cost_per_1k: 0.30 / 1000.0, // $0.30 per 1M = $0.0003 per 1K
        }
    }
}

impl TokenBudget {
    /// Create a new token budget with limits
    pub fn new(max_tokens: usize, max_cost_usd: Option<f64>) -> Self {
        Self {
            max_tokens,
            max_cost_usd,
            ..Default::default()
        }
    }

    /// Record token usage from an LLM call
    pub fn record_usage(&mut self, input_tokens: usize, output_tokens: usize) {
        self.input_tokens_used += input_tokens;
        self.output_tokens_used += output_tokens;

        // Update cost estimate
        let input_cost = (input_tokens as f64 / 1000.0) * self.input_cost_per_1k;
        let output_cost = (output_tokens as f64 / 1000.0) * self.output_cost_per_1k;
        self.cost_usd += input_cost + output_cost;
    }

    /// Get total tokens used
    pub fn total_tokens_used(&self) -> usize {
        self.input_tokens_used + self.output_tokens_used
    }

    /// Get remaining token budget
    pub fn remaining_tokens(&self) -> usize {
        self.max_tokens.saturating_sub(self.total_tokens_used())
    }

    /// Check if budget is exhausted
    pub fn is_exhausted(&self) -> bool {
        self.total_tokens_used() >= self.max_tokens
    }

    /// Check if cost limit exceeded
    pub fn cost_exceeded(&self) -> bool {
        if let Some(max_cost) = self.max_cost_usd {
            self.cost_usd >= max_cost
        } else {
            false
        }
    }

    /// Check if we should stop due to budget
    pub fn should_stop(&self) -> bool {
        self.is_exhausted() || self.cost_exceeded()
    }

    /// Get budget usage percentage
    pub fn usage_percent(&self) -> f32 {
        if self.max_tokens == 0 {
            0.0
        } else {
            (self.total_tokens_used() as f32 / self.max_tokens as f32) * 100.0
        }
    }

    /// Set model-specific pricing
    pub fn set_pricing(&mut self, input_per_1k: f64, output_per_1k: f64) {
        self.input_cost_per_1k = input_per_1k;
        self.output_cost_per_1k = output_per_1k;
    }

    /// Get formatted summary
    pub fn summary(&self) -> String {
        format!(
            "Tokens: {}/{} ({:.1}%), Cost: ${:.4}{}",
            self.total_tokens_used(),
            self.max_tokens,
            self.usage_percent(),
            self.cost_usd,
            self.max_cost_usd
                .map(|m| format!(" / ${:.2}", m))
                .unwrap_or_default()
        )
    }
}

/// Agent context containing workspace state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContext {
    /// Working directory for the agent
    pub working_dir: PathBuf,
    /// Conversation history
    pub history: Vec<AgentMessage>,
    /// Merkle root hash of current state
    pub merkle_root: [u8; 32],
    /// Complexity threshold K for sub-graph approval
    pub complexity_k: usize,
    /// Session ID
    pub session_id: String,
    /// Auto-approve mode
    pub auto_approve: bool,
    /// Defer tests until sheaf validation (skip V_log during coding)
    pub defer_tests: bool,
    /// Log all LLM requests/responses to database
    pub log_llm: bool,
    /// Last diagnostics from LSP (for correction prompts)
    #[serde(skip)]
    pub last_diagnostics: Vec<lsp_types::Diagnostic>,
    /// Token budget for cost control
    pub token_budget: TokenBudget,
    /// Last test output for correction prompts
    #[serde(skip)]
    pub last_test_output: Option<String>,
    /// PSP-5: Execution mode (Project vs Solo)
    #[serde(default)]
    pub execution_mode: ExecutionMode,
    /// PSP-5: Verifier strictness preset
    #[serde(default)]
    pub verifier_strictness: VerifierStrictness,
    /// PSP-5: Active language plugins detected for this workspace
    #[serde(default)]
    pub active_plugins: Vec<String>,
    /// PSP-5: Workspace state classification (existing, greenfield, or ambiguous)
    #[serde(default)]
    pub workspace_state: WorkspaceState,
    /// PSP-5 Phase 2: Ownership manifest for file-to-node bindings
    #[serde(default)]
    pub ownership_manifest: OwnershipManifest,
}

impl Default for AgentContext {
    fn default() -> Self {
        Self {
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            history: Vec::new(),
            merkle_root: [0u8; 32],
            complexity_k: 5, // Default from PSP
            session_id: uuid::Uuid::new_v4().to_string(),
            auto_approve: false,
            defer_tests: false,
            log_llm: false,
            last_diagnostics: Vec::new(),
            token_budget: TokenBudget::default(),
            last_test_output: None,
            execution_mode: ExecutionMode::default(),
            verifier_strictness: VerifierStrictness::default(),
            active_plugins: Vec::new(),
            workspace_state: WorkspaceState::default(),
            ownership_manifest: OwnershipManifest::default(),
        }
    }
}

/// Agent message in conversation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    /// Role/tier of the sender
    pub role: ModelTier,
    /// Message content
    pub content: String,
    /// Timestamp
    pub timestamp: SystemTime,
    /// Associated node ID
    pub node_id: Option<String>,
}

impl AgentMessage {
    /// Create a new message
    pub fn new(role: ModelTier, content: String) -> Self {
        Self {
            role,
            content,
            timestamp: SystemTime::now(),
            node_id: None,
        }
    }
}

/// Energy components for Lyapunov calculation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnergyComponents {
    /// Syntactic energy (from LSP diagnostics)
    pub v_syn: f32,
    /// Structural energy (from contract verification)
    pub v_str: f32,
    /// Logic energy (from test results)
    pub v_log: f32,
    /// Bootstrapping energy (from command exit codes)
    pub v_boot: f32,
    /// Sheaf validation energy (cross-node consistency)
    pub v_sheaf: f32,
}

impl EnergyComponents {
    /// Calculate total energy: V(x) = α*V_syn + β*V_str + γ*V_log + V_boot + V_sheaf
    pub fn total(&self, contract: &BehavioralContract) -> f32 {
        contract.alpha() * self.v_syn
            + contract.beta() * self.v_str
            + contract.gamma() * self.v_log
            + self.v_boot
            + self.v_sheaf
    }

    /// Calculate total energy for Solo Mode (implicit weights = 1.0)
    /// Used when no BehavioralContract is available
    pub fn total_simple(&self) -> f32 {
        self.v_syn + self.v_str + self.v_log + self.v_boot + self.v_sheaf
    }
}

// =============================================================================
// Task Plan Types - Structured output from Architect
// =============================================================================

/// Task type classification for planning
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// Implementation code
    #[default]
    Code,
    /// Shell command execution (e.g., cargo new, npm init)
    Command,
    /// Unit tests
    UnitTest,
    /// Integration/E2E tests
    IntegrationTest,
    /// Refactoring existing code
    Refactor,
    /// Documentation
    Documentation,
}

/// Structured task plan from Architect
/// Output as JSON for reliable parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    /// List of tasks to execute
    pub tasks: Vec<PlannedTask>,
}

impl TaskPlan {
    /// Create an empty plan
    pub fn new() -> Self {
        Self { tasks: Vec::new() }
    }

    /// Get the total number of tasks
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Check if plan is empty
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Get task by ID
    pub fn get_task(&self, id: &str) -> Option<&PlannedTask> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// Validate the plan structure
    pub fn validate(&self) -> Result<(), String> {
        if self.tasks.is_empty() {
            return Err("Plan has no tasks".to_string());
        }

        // Check for duplicate IDs
        let mut seen_ids = std::collections::HashSet::new();
        for task in &self.tasks {
            if !seen_ids.insert(&task.id) {
                return Err(format!("Duplicate task ID: {}", task.id));
            }
            if task.goal.is_empty() {
                return Err(format!("Task {} has empty goal", task.id));
            }
        }

        // Check for invalid dependencies
        for task in &self.tasks {
            for dep in &task.dependencies {
                if !seen_ids.contains(dep) {
                    return Err(format!("Task {} has unknown dependency: {}", task.id, dep));
                }
            }
        }

        // PSP-5: Check for duplicate output_files across tasks (ownership closure)
        let mut file_owners: std::collections::HashMap<&str, &str> =
            std::collections::HashMap::new();
        for task in &self.tasks {
            for file in &task.output_files {
                if let Some(prev_owner) = file_owners.insert(file.as_str(), task.id.as_str()) {
                    return Err(format!(
                        "Ownership violation in plan: file '{}' claimed by both '{}' and '{}'. \
                         Each output file must appear in exactly one task's output_files.",
                        file, prev_owner, task.id
                    ));
                }
            }
        }

        // PSP-7: Cycle detection via topological sort (Kahn's algorithm)
        {
            let id_to_idx: std::collections::HashMap<&str, usize> = self
                .tasks
                .iter()
                .enumerate()
                .map(|(i, t)| (t.id.as_str(), i))
                .collect();
            let n = self.tasks.len();
            let mut in_degree = vec![0usize; n];
            let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
            for (i, task) in self.tasks.iter().enumerate() {
                for dep in &task.dependencies {
                    if let Some(&dep_idx) = id_to_idx.get(dep.as_str()) {
                        adj[dep_idx].push(i);
                        in_degree[i] += 1;
                    }
                }
            }
            let mut queue: std::collections::VecDeque<usize> = in_degree
                .iter()
                .enumerate()
                .filter_map(|(i, &d)| if d == 0 { Some(i) } else { None })
                .collect();
            let mut visited = 0usize;
            while let Some(node) = queue.pop_front() {
                visited += 1;
                for &next in &adj[node] {
                    in_degree[next] -= 1;
                    if in_degree[next] == 0 {
                        queue.push_back(next);
                    }
                }
            }
            if visited != n {
                return Err("Plan contains a dependency cycle".to_string());
            }
        }

        // PSP-7: Implicit dependency enforcement — if task A reads a file that
        // task B produces (context_files ∩ output_files), A must depend on B.
        for task in &self.tasks {
            for ctx_file in &task.context_files {
                if let Some(&owner) = file_owners.get(ctx_file.as_str()) {
                    if owner != task.id && !task.dependencies.iter().any(|d| d == owner) {
                        return Err(format!(
                            "Task '{}' reads '{}' produced by '{}' but does not declare it as a dependency",
                            task.id, ctx_file, owner
                        ));
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for TaskPlan {
    fn default() -> Self {
        Self::new()
    }
}

/// A planned task from the Architect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedTask {
    /// Unique task identifier (e.g., "task_1", "test_auth")
    pub id: String,
    /// Human-readable goal description
    pub goal: String,
    /// Files to read for context
    #[serde(default)]
    pub context_files: Vec<String>,
    /// Files to create or modify
    #[serde(default)]
    pub output_files: Vec<String>,
    /// Task IDs this depends on (must complete first)
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Type of task
    #[serde(default)]
    pub task_type: TaskType,
    /// Behavioral contract for this task
    #[serde(default)]
    pub contract: PlannedContract,
    /// Command contract (only for TaskType::Command)
    #[serde(default)]
    pub command_contract: Option<CommandContract>,
    /// PSP-5: Node class (Interface / Implementation / Integration)
    #[serde(default)]
    pub node_class: NodeClass,
    /// Declared dependency expectations (packages, setup, toolchain).
    #[serde(default)]
    pub dependency_expectations: DependencyExpectation,
}

impl PlannedTask {
    /// Create a simple task
    pub fn new(id: impl Into<String>, goal: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            goal: goal.into(),
            context_files: Vec::new(),
            output_files: Vec::new(),
            dependencies: Vec::new(),
            task_type: TaskType::Code,
            contract: PlannedContract::default(),
            command_contract: None,
            node_class: NodeClass::default(),
            dependency_expectations: DependencyExpectation::default(),
        }
    }

    /// Convert to SRBNNode
    pub fn to_srbn_node(&self, tier: ModelTier) -> SRBNNode {
        let mut node = SRBNNode::new(self.id.clone(), self.goal.clone(), tier);
        node.context_files = self.context_files.iter().map(PathBuf::from).collect();
        node.output_targets = self.output_files.iter().map(PathBuf::from).collect();
        node.contract = self.contract.to_behavioral_contract();
        node.node_class = self.node_class;
        node.dependency_expectations = self.dependency_expectations.clone();
        node
    }
}

/// Contract specified in the plan
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlannedContract {
    /// Required public API signature
    #[serde(default)]
    pub interface_signature: Option<String>,
    /// Semantic constraints
    #[serde(default)]
    pub invariants: Vec<String>,
    /// Patterns to avoid
    #[serde(default)]
    pub forbidden_patterns: Vec<String>,
    /// Test cases with criticality
    #[serde(default)]
    pub tests: Vec<PlannedTest>,
}

impl PlannedContract {
    /// Convert to BehavioralContract
    pub fn to_behavioral_contract(&self) -> BehavioralContract {
        BehavioralContract {
            interface_signature: self.interface_signature.clone().unwrap_or_default(),
            invariants: self.invariants.clone(),
            forbidden_patterns: self.forbidden_patterns.clone(),
            weighted_tests: self
                .tests
                .iter()
                .map(|t| WeightedTest {
                    test_name: t.name.clone(),
                    criticality: t.criticality,
                })
                .collect(),
            energy_weights: (1.0, 0.5, 2.0),
        }
    }
}

/// A test case in the plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedTest {
    /// Test name or pattern
    pub name: String,
    /// Criticality level
    #[serde(default = "default_criticality")]
    pub criticality: Criticality,
}

fn default_criticality() -> Criticality {
    Criticality::High
}

/// Contract for command-type tasks (shell commands)
/// Defines expected outcomes for V_boot calculation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommandContract {
    /// The shell command to execute
    pub command: String,
    /// Expected exit code (default: 0)
    #[serde(default)]
    pub expected_exit_code: i32,
    /// Files that should exist after command completes
    #[serde(default)]
    pub expected_files: Vec<String>,
    /// Patterns that should NOT appear in stderr
    #[serde(default)]
    pub forbidden_stderr_patterns: Vec<String>,
    /// Working directory for the command (relative to project root)
    #[serde(default)]
    pub working_dir: Option<String>,
}

impl CommandContract {
    /// Create a new command contract
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            expected_exit_code: 0,
            expected_files: Vec::new(),
            forbidden_stderr_patterns: Vec::new(),
            working_dir: None,
        }
    }

    /// Calculate V_boot energy from command result
    pub fn calculate_energy(&self, exit_code: i32, stderr: &str, existing_files: &[String]) -> f32 {
        let mut energy = 0.0;

        // Exit code mismatch
        if exit_code != self.expected_exit_code {
            energy += 1.0;
        }

        // Missing expected files
        for expected in &self.expected_files {
            if !existing_files.contains(expected) {
                energy += 0.5;
            }
        }

        // Forbidden stderr patterns
        for pattern in &self.forbidden_stderr_patterns {
            if stderr.contains(pattern) {
                energy += 0.3;
            }
        }

        energy
    }
}

// =============================================================================
// PSP-000005 Types — Project-First Execution Model
// =============================================================================

/// PSP-5: Execution mode for the runtime
///
/// Project mode is the default. Solo mode only activates on explicit single-file
/// intent keywords or via `--single-file` CLI flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Default: treat task as a multi-file project
    #[default]
    Project,
    /// Explicit single-file execution
    Solo,
}

impl std::fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionMode::Project => write!(f, "project"),
            ExecutionMode::Solo => write!(f, "solo"),
        }
    }
}

/// PSP-5: Workspace state classification
///
/// Determined at session start by inspecting the working directory for project
/// metadata and cross-referencing with the task description. Drives the
/// init/bootstrap/context strategy for the rest of the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceState {
    /// Directory contains recognized project metadata (Cargo.toml, pyproject.toml, etc.)
    ExistingProject {
        /// Plugin names detected in the workspace
        plugins: Vec<String>,
    },
    /// Empty or non-project directory; language inferred from the task description
    Greenfield {
        /// Language inferred from task keywords (e.g. "rust", "python")
        inferred_lang: Option<String>,
    },
    /// Directory has files but no recognized project metadata and no language inferred
    #[default]
    Ambiguous,
}

impl std::fmt::Display for WorkspaceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkspaceState::ExistingProject { plugins } => {
                write!(f, "existing-project({})", plugins.join(", "))
            }
            WorkspaceState::Greenfield { inferred_lang } => {
                write!(
                    f,
                    "greenfield({})",
                    inferred_lang.as_deref().unwrap_or("unknown")
                )
            }
            WorkspaceState::Ambiguous => write!(f, "ambiguous"),
        }
    }
}

/// PSP-5: Node class distinguishing interface, implementation, and integration nodes
///
/// - **Interface** nodes define exported signatures, schemas, and verifier scope.
/// - **Implementation** nodes operate on node-owned files plus sealed interfaces.
/// - **Integration** nodes reconcile cross-owner or cross-plugin boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeClass {
    /// Defines exported signatures, schemas, ownership manifests
    Interface,
    /// Operates on node-owned files plus adjacent sealed interfaces
    #[default]
    Implementation,
    /// Reconciles cross-owner or cross-plugin boundaries
    Integration,
}

impl std::fmt::Display for NodeClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeClass::Interface => write!(f, "interface"),
            NodeClass::Implementation => write!(f, "implementation"),
            NodeClass::Integration => write!(f, "integration"),
        }
    }
}

/// PSP-5: Verifier strictness presets
///
/// Controls which verification stages are required for stability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VerifierStrictness {
    /// Default: compilation + tests required, warnings allowed
    #[default]
    Default,
    /// Strict: compilation + tests + linting (e.g. clippy -D warnings)
    Strict,
    /// Minimal: syntax/parse check only, no tests required
    Minimal,
}

// =============================================================================
// PSP-5 Phase 2: Ownership Manifests
// =============================================================================

/// PSP-5 Phase 2: A single ownership entry mapping a file to its owning node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipEntry {
    /// The node ID that owns this file
    pub owner_node_id: String,
    /// The language plugin responsible for this file
    pub owner_plugin: String,
    /// The node class of the owning node
    pub node_class: NodeClass,
}

/// PSP-5 Phase 2: Ownership manifest tracking file-to-node bindings
///
/// Enforces ownership closure: a node may only modify files it owns,
/// unless it is an Integration node (which may cross ownership boundaries).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipManifest {
    /// File path → ownership entry
    entries: std::collections::HashMap<String, OwnershipEntry>,
    /// Maximum files a single node may touch (bounded fanout)
    #[serde(default = "OwnershipManifest::default_fanout")]
    fanout_limit: usize,
}

impl Default for OwnershipManifest {
    fn default() -> Self {
        Self::new()
    }
}

impl OwnershipManifest {
    /// Create a new empty manifest with the default fanout limit
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            fanout_limit: Self::default_fanout(),
        }
    }

    /// Create with a custom fanout limit
    pub fn with_fanout_limit(limit: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            fanout_limit: limit,
        }
    }

    fn default_fanout() -> usize {
        20
    }

    /// Assign a file to an owning node.
    ///
    /// The path is normalized before insertion so that `src/main.rs` and
    /// `./src/main.rs` resolve to the same key.
    pub fn assign(
        &mut self,
        path: impl Into<String>,
        owner_node_id: impl Into<String>,
        owner_plugin: impl Into<String>,
        node_class: NodeClass,
    ) {
        let key = crate::path::normalize_path_key(&path.into()).unwrap_or_default();
        if key.is_empty() {
            return; // silently skip invalid paths
        }
        self.entries.insert(
            key,
            OwnershipEntry {
                owner_node_id: owner_node_id.into(),
                owner_plugin: owner_plugin.into(),
                node_class,
            },
        );
    }

    /// Look up the owner of a file path.
    ///
    /// The path is normalized before lookup.
    pub fn owner_of(&self, path: &str) -> Option<&OwnershipEntry> {
        let key = crate::path::normalize_path_key(path)?;
        self.entries.get(&key)
    }

    /// List all files owned by a specific node
    pub fn files_owned_by(&self, node_id: &str) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.owner_node_id == node_id)
            .map(|(path, _)| path.as_str())
            .collect()
    }

    /// Get the total number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the manifest is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the fanout limit
    pub fn fanout_limit(&self) -> usize {
        self.fanout_limit
    }

    /// Validate that a bundle respects ownership boundaries
    ///
    /// Rules:
    /// - **Implementation** nodes: all paths must be owned by this node
    /// - **Interface** nodes: all paths must be owned by this node
    /// - **Integration** nodes: paths may cross ownership boundaries
    /// - Fanout limit: bundle must not exceed max files per node
    /// - Unregistered paths (new files) are allowed and will be auto-assigned
    pub fn validate_bundle(
        &self,
        bundle: &ArtifactBundle,
        node_id: &str,
        node_class: NodeClass,
    ) -> Result<(), String> {
        let artifact_count = bundle.len();

        // Check fanout limit
        if artifact_count > self.fanout_limit {
            return Err(format!(
                "Bundle has {} artifacts, exceeding fanout limit of {}",
                artifact_count, self.fanout_limit
            ));
        }

        // Integration nodes can cross ownership boundaries
        if node_class == NodeClass::Integration {
            return Ok(());
        }

        // For Interface and Implementation nodes, check ownership
        for op in &bundle.artifacts {
            let raw_path = op.path();
            let key =
                crate::path::normalize_path_key(raw_path).unwrap_or_else(|| raw_path.to_string());
            if let Some(entry) = self.entries.get(&key) {
                if entry.owner_node_id != node_id {
                    return Err(format!(
                        "Ownership violation: file '{}' is owned by node '{}', \
                         but node '{}' ({}) attempted to modify it. \
                         Only Integration nodes may cross ownership boundaries.",
                        raw_path, entry.owner_node_id, node_id, node_class
                    ));
                }
            }
            // Unregistered paths (new files) are allowed — they'll be assigned to this node
        }

        Ok(())
    }

    /// Auto-assign unregistered paths from a bundle to a node
    ///
    /// Called after validate_bundle succeeds, this registers any new paths
    /// in the manifest so future nodes can't claim them.
    pub fn assign_new_paths(
        &mut self,
        bundle: &ArtifactBundle,
        node_id: &str,
        owner_plugin: &str,
        node_class: NodeClass,
    ) {
        for op in &bundle.artifacts {
            let raw_path = op.path();
            let key =
                crate::path::normalize_path_key(raw_path).unwrap_or_else(|| raw_path.to_string());
            if !self.entries.contains_key(&key) {
                self.assign(raw_path, node_id, owner_plugin, node_class);
            }
        }
    }
}

/// PSP-5: A single artifact operation within an artifact bundle
///
/// Each operation represents one file mutation: either a full write or a diff patch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum ArtifactOperation {
    /// Write the full file contents
    Write {
        /// Relative path within the workspace
        path: String,
        /// Full file content
        content: String,
    },
    /// Apply a unified diff patch
    Diff {
        /// Relative path within the workspace
        path: String,
        /// Unified diff content
        patch: String,
    },
    /// Delete a file from the workspace
    Delete {
        /// Relative path to delete
        path: String,
    },
    /// Move/rename a file within the workspace
    Move {
        /// Current relative path
        from: String,
        /// New relative path
        to: String,
    },
}

impl ArtifactOperation {
    /// Get the primary file path this operation targets
    pub fn path(&self) -> &str {
        match self {
            ArtifactOperation::Write { path, .. } => path,
            ArtifactOperation::Diff { path, .. } => path,
            ArtifactOperation::Delete { path } => path,
            ArtifactOperation::Move { from, .. } => from,
        }
    }

    /// Check if this is a write (new file) operation
    pub fn is_write(&self) -> bool {
        matches!(self, ArtifactOperation::Write { .. })
    }

    /// Check if this is a diff (patch) operation
    pub fn is_diff(&self) -> bool {
        matches!(self, ArtifactOperation::Diff { .. })
    }

    /// Check if this is a delete operation
    pub fn is_delete(&self) -> bool {
        matches!(self, ArtifactOperation::Delete { .. })
    }

    /// Check if this is a move/rename operation
    pub fn is_move(&self) -> bool {
        matches!(self, ArtifactOperation::Move { .. })
    }
}

/// PSP-5: Multi-artifact bundle from the Actuator
///
/// A node response containing one or more file operations applied as a unit.
/// The orchestrator SHALL parse all operations before mutating the workspace
/// and SHALL fail atomically if any operation is invalid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactBundle {
    /// File operations to apply
    pub artifacts: Vec<ArtifactOperation>,
    /// Optional commands to run after file operations
    #[serde(default)]
    pub commands: Vec<String>,
}

impl ArtifactBundle {
    /// Create an empty bundle
    pub fn new() -> Self {
        Self {
            artifacts: Vec::new(),
            commands: Vec::new(),
        }
    }

    /// Number of file operations
    pub fn len(&self) -> usize {
        self.artifacts.len()
    }

    /// Check if bundle is empty
    pub fn is_empty(&self) -> bool {
        self.artifacts.is_empty()
    }

    /// Get all unique file paths affected by this bundle
    pub fn affected_paths(&self) -> Vec<&str> {
        let mut paths: Vec<&str> = self.artifacts.iter().map(|a| a.path()).collect();
        // For Move operations, also include the destination path
        for op in &self.artifacts {
            if let ArtifactOperation::Move { to, .. } = op {
                paths.push(to.as_str());
            }
        }
        paths.sort();
        paths.dedup();
        paths
    }

    /// Count of file writes (new files)
    pub fn writes_count(&self) -> usize {
        self.artifacts.iter().filter(|a| a.is_write()).count()
    }

    /// Count of file diffs (patches)
    pub fn diffs_count(&self) -> usize {
        self.artifacts.iter().filter(|a| a.is_diff()).count()
    }

    /// Count of file deletes
    pub fn deletes_count(&self) -> usize {
        self.artifacts.iter().filter(|a| a.is_delete()).count()
    }

    /// Count of file moves
    pub fn moves_count(&self) -> usize {
        self.artifacts.iter().filter(|a| a.is_move()).count()
    }

    /// Validate the bundle: checks for empty paths and duplicate targets
    pub fn validate(&self) -> Result<(), String> {
        if self.artifacts.is_empty() {
            return Err("Artifact bundle is empty".to_string());
        }

        for (i, op) in self.artifacts.iter().enumerate() {
            // Validate the primary path
            Self::validate_path(op.path(), i)?;

            // For Move operations, also validate the destination path
            if let ArtifactOperation::Move { to, .. } = op {
                if to.is_empty() {
                    return Err(format!("Artifact {} (move) has empty destination path", i));
                }
                Self::validate_path(to, i)?;
            }
        }

        Ok(())
    }

    /// Validate a single path: reject empty, absolute, and traversal paths.
    ///
    /// Uses the canonical `normalize_artifact_path` utility so that all path
    /// consumers (bundle validation, ownership manifest, policy checks) agree
    /// on path identity.
    fn validate_path(path: &str, artifact_index: usize) -> Result<(), String> {
        use crate::path::{normalize_artifact_path, PathError};
        match normalize_artifact_path(path) {
            Ok(_) => Ok(()),
            Err(PathError::Empty) => Err(format!("Artifact {} has empty path", artifact_index)),
            Err(PathError::Absolute(_)) => Err(format!(
                "Artifact {} has absolute path '{}', must be relative",
                artifact_index, path
            )),
            Err(PathError::Escapes(_)) => Err(format!(
                "Artifact {} has path traversal in '{}'",
                artifact_index, path
            )),
            Err(PathError::Invalid(_)) => Err(format!(
                "Artifact {} has invalid path '{}'",
                artifact_index, path
            )),
        }
    }
}

impl Default for ArtifactBundle {
    fn default() -> Self {
        Self::new()
    }
}

/// PSP-5: Structured verification result from a plugin-driven verifier
///
/// Holds the outcome of running syntax checks, build, tests, and lint
/// through the active language plugin's toolchain.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether the syntax/type check passed
    pub syntax_ok: bool,
    /// Whether the build succeeded
    pub build_ok: bool,
    /// Whether tests passed
    pub tests_ok: bool,
    /// Whether lint passed (only in Strict mode)
    pub lint_ok: bool,
    /// Number of diagnostics from LSP / compiler
    pub diagnostics_count: usize,
    /// Number of tests passed
    pub tests_passed: usize,
    /// Number of tests failed
    pub tests_failed: usize,
    /// Summary output from verification tools
    pub summary: String,
    /// Raw tool output (for correction prompts)
    pub raw_output: Option<String>,
    /// Whether verification ran in degraded mode (missing tools)
    pub degraded: bool,
    /// Reason for degraded mode
    pub degraded_reason: Option<String>,
    /// Per-stage outcomes with sensor status
    #[serde(default)]
    pub stage_outcomes: Vec<StageOutcome>,
}

impl VerificationResult {
    /// Check if all verification stages passed
    pub fn all_passed(&self) -> bool {
        self.syntax_ok && self.build_ok && self.tests_ok && !self.degraded
    }

    /// Create a degraded result when tools are unavailable
    pub fn degraded(reason: impl Into<String>) -> Self {
        Self {
            degraded: true,
            degraded_reason: Some(reason.into()),
            summary: "Verification ran in degraded mode".to_string(),
            ..Default::default()
        }
    }

    /// Check whether any stage ran with a fallback or unavailable sensor.
    ///
    /// When true the caller should NOT treat a passing result as a genuine
    /// stability proof — the energy surface was only partially observable.
    pub fn has_degraded_stages(&self) -> bool {
        self.stage_outcomes
            .iter()
            .any(|s| !matches!(s.sensor_status, SensorStatus::Available))
    }

    /// Collect human-readable descriptions of all degraded stages.
    pub fn degraded_stage_reasons(&self) -> Vec<String> {
        self.stage_outcomes
            .iter()
            .filter_map(|s| match &s.sensor_status {
                SensorStatus::Available => None,
                SensorStatus::Fallback { actual, reason } => Some(format!(
                    "{}: used fallback '{}' ({})",
                    s.stage, actual, reason
                )),
                SensorStatus::Unavailable { reason } => {
                    Some(format!("{}: unavailable ({})", s.stage, reason))
                }
            })
            .collect()
    }
}

/// Sensor availability status for a single verification stage.
///
/// Tells downstream consumers whether the preferred tool was available,
/// a fallback was used, or the stage had no usable sensor at all.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SensorStatus {
    /// The preferred tool ran successfully.
    Available,
    /// A fallback tool was used instead of the primary.
    Fallback {
        /// Name of the tool that actually ran.
        actual: String,
        /// Why the primary was not available.
        reason: String,
    },
    /// No tool was available for this stage.
    Unavailable {
        /// What went wrong.
        reason: String,
    },
}

impl std::fmt::Display for SensorStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SensorStatus::Available => write!(f, "available"),
            SensorStatus::Fallback { actual, .. } => write!(f, "fallback({})", actual),
            SensorStatus::Unavailable { reason } => write!(f, "unavailable({})", reason),
        }
    }
}

/// Outcome of a single verification stage (syntax, build, test, lint).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageOutcome {
    /// Which verification stage this covers.
    pub stage: String,
    /// Whether the stage passed.
    pub passed: bool,
    /// Sensor status for this stage.
    pub sensor_status: SensorStatus,
    /// Optional output captured from the tool.
    pub output: Option<String>,
}

// =============================================================================
// PSP-5 Phase 3: Context Provenance, Structural Digests, Restriction Maps
// =============================================================================

/// PSP-5 Phase 3: Kind of structural artifact being digested
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    /// Exported function/trait/class signature
    Signature,
    /// API schema (JSON schema, protobuf, etc.)
    Schema,
    /// Module-level symbol inventory
    SymbolInventory,
    /// Interface seal for dependency checking
    InterfaceSeal,
}

impl std::fmt::Display for ArtifactKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArtifactKind::Signature => write!(f, "signature"),
            ArtifactKind::Schema => write!(f, "schema"),
            ArtifactKind::SymbolInventory => write!(f, "symbol_inventory"),
            ArtifactKind::InterfaceSeal => write!(f, "interface_seal"),
        }
    }
}

/// PSP-5 Phase 3: Hash of a compile-critical structural artifact
///
/// Structural digests represent machine-verifiable content (exported signatures,
/// schemas, symbol inventories) that nodes depend on. When the digest changes,
/// dependent nodes must re-verify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralDigest {
    /// Unique digest identifier
    pub digest_id: String,
    /// What kind of structural artifact this is
    pub artifact_kind: ArtifactKind,
    /// SHA-256 hash of the artifact content
    pub hash: [u8; 32],
    /// Node that produced this artifact
    pub source_node_id: String,
    /// Source file path (relative to workspace)
    pub source_path: String,
    /// Monotonically increasing version
    pub version: u32,
}

impl StructuralDigest {
    /// Create a new digest from raw content
    pub fn from_content(
        source_node_id: impl Into<String>,
        source_path: impl Into<String>,
        artifact_kind: ArtifactKind,
        content: &[u8],
    ) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut sha = [0u8; 32];
        // Use a simple hash for the digest (real impl would use SHA-256)
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        let h = hasher.finish().to_le_bytes();
        sha[..8].copy_from_slice(&h);

        let node_id = source_node_id.into();
        let path = source_path.into();
        let digest_id = format!("{}:{}:{}", node_id, path, artifact_kind);

        Self {
            digest_id,
            artifact_kind,
            hash: sha,
            source_node_id: node_id,
            source_path: path,
            version: 1,
        }
    }

    /// Check if this digest matches another (same content hash)
    pub fn matches(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

/// PSP-5 Phase 3: Kind of semantic summary being digested
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryKind {
    /// Intent summary from parent/architect
    IntentSummary,
    /// Verifier results summary
    VerifierResults,
    /// Design rationale
    DesignRationale,
}

/// PSP-5 Phase 3: Condensed summary with hash for provenance tracking
///
/// Summary digests represent advisory semantic content (intent summaries,
/// verifier results) whose hashes are recorded for reproducibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryDigest {
    /// Unique identifier
    pub digest_id: String,
    /// Node that produced this summary
    pub source_node_id: String,
    /// What kind of summary this is
    pub kind: SummaryKind,
    /// SHA-256 hash of the summary content
    pub hash: [u8; 32],
    /// Byte length of original content
    pub original_byte_length: usize,
    /// The condensed summary text
    pub summary_text: String,
}

/// PSP-5 Phase 3: Context budget controlling node context assembly
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextBudget {
    /// Maximum total bytes for the context package
    pub byte_limit: usize,
    /// Maximum number of files to include
    pub file_count_limit: usize,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            byte_limit: 100 * 1024, // 100KB default
            file_count_limit: 20,
        }
    }
}

/// PSP-5 Phase 3: Restriction map defining a node's context boundary
///
/// The restriction map bounds what a node can see. It is derived from the
/// task graph, ownership manifest, and parent scope. A node SHALL NOT receive
/// the full repository by default.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RestrictionMap {
    /// The node this restriction applies to
    pub node_id: String,
    /// Context budget (byte and file-count limits)
    #[serde(default)]
    pub budget: ContextBudget,
    /// Files the node owns and can see in full
    #[serde(default)]
    pub owned_files: Vec<String>,
    /// Adjacent sealed interfaces the node can reference
    #[serde(default)]
    pub sealed_interfaces: Vec<String>,
    /// Structural digests for external dependencies (preferred over raw files)
    #[serde(default)]
    pub structural_digests: Vec<StructuralDigest>,
    /// Summary digests for advisory context
    #[serde(default)]
    pub summary_digests: Vec<SummaryDigest>,
    /// Dependency commit hashes this node relies on
    #[serde(default)]
    pub dependency_commits: std::collections::HashMap<String, Vec<u8>>,
}

impl RestrictionMap {
    /// Create a restriction map for a node with default budget
    pub fn for_node(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            ..Default::default()
        }
    }

    /// Total structural bytes (approximation)
    pub fn structural_bytes(&self) -> usize {
        self.structural_digests
            .iter()
            .map(|d| d.source_path.len() + 64)
            .sum::<usize>()
            + self.sealed_interfaces.len() * 128
    }
}

/// PSP-5 Phase 3: Reproducible context package for node execution
///
/// A context package is the complete, bounded input assembled for a node's
/// LLM prompt. It records exactly what was included so the same context can
/// be reconstructed from the ledger and repository state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextPackage {
    /// Unique package identifier
    pub package_id: String,
    /// The node this context was assembled for
    pub node_id: String,
    /// The restriction map used
    pub restriction_map: RestrictionMap,
    /// Raw file contents included (path → content)
    #[serde(default)]
    pub included_files: std::collections::HashMap<String, String>,
    /// Structural digests included in this package
    #[serde(default)]
    pub structural_digests: Vec<StructuralDigest>,
    /// Summary digests included in this package
    #[serde(default)]
    pub summary_digests: Vec<SummaryDigest>,
    /// Total byte size of the assembled context
    pub total_bytes: usize,
    /// Whether budget was exceeded and content was trimmed
    pub budget_exceeded: bool,
    /// Timestamp of assembly
    pub created_at: i64,
}

impl ContextPackage {
    /// Create a new empty context package for a node
    pub fn new(node_id: impl Into<String>) -> Self {
        let nid = node_id.into();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Self {
            package_id: format!("ctx_{}_{}", nid, ts),
            node_id: nid,
            created_at: ts,
            ..Default::default()
        }
    }

    /// Add a file to the context package, respecting budget
    pub fn add_file(&mut self, path: &str, content: String) -> bool {
        let new_bytes = self.total_bytes + content.len();
        if new_bytes > self.restriction_map.budget.byte_limit {
            self.budget_exceeded = true;
            return false;
        }
        if self.included_files.len() >= self.restriction_map.budget.file_count_limit {
            self.budget_exceeded = true;
            return false;
        }
        self.total_bytes = new_bytes;
        self.included_files.insert(path.to_string(), content);
        true
    }

    /// Add a structural digest (always fits, they're small)
    pub fn add_structural_digest(&mut self, digest: StructuralDigest) {
        self.structural_digests.push(digest);
    }

    /// Add a summary digest
    pub fn add_summary_digest(&mut self, digest: SummaryDigest) {
        self.total_bytes += digest.summary_text.len();
        self.summary_digests.push(digest);
    }

    /// Get the provenance record for this package
    pub fn provenance(&self) -> ContextProvenance {
        ContextProvenance {
            node_id: self.node_id.clone(),
            context_package_id: self.package_id.clone(),
            structural_digest_hashes: self
                .structural_digests
                .iter()
                .map(|d| (d.digest_id.clone(), d.hash))
                .collect(),
            summary_digest_hashes: self
                .summary_digests
                .iter()
                .map(|d| (d.digest_id.clone(), d.hash))
                .collect(),
            dependency_commit_hashes: self
                .restriction_map
                .dependency_commits
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            included_file_count: self.included_files.len(),
            total_bytes: self.total_bytes,
            created_at: self.created_at,
        }
    }
}

/// PSP-5 Phase 3: Provenance record tracking what context was used
///
/// Records the hashes of all summaries, contracts, and dependency commits
/// used to derive a node's prompt context. This enables reproducibility:
/// the same context package can be reconstructed from persisted state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextProvenance {
    /// Node this provenance belongs to
    pub node_id: String,
    /// Context package ID
    pub context_package_id: String,
    /// Structural digest ID → hash pairs used
    #[serde(default)]
    pub structural_digest_hashes: Vec<(String, [u8; 32])>,
    /// Summary digest ID → hash pairs used
    #[serde(default)]
    pub summary_digest_hashes: Vec<(String, [u8; 32])>,
    /// Dependency node → commit hash pairs
    #[serde(default)]
    pub dependency_commit_hashes: Vec<(String, Vec<u8>)>,
    /// Number of raw files included
    pub included_file_count: usize,
    /// Total bytes in context package
    pub total_bytes: usize,
    /// When this provenance was recorded
    pub created_at: i64,
}

// =============================================================================
// PSP-5 Phase 5: Escalation Semantics, Local Graph Rewrite, Sheaf Targeting
// =============================================================================

/// PSP-5 Phase 5: Category of non-convergence detected by the verifier.
///
/// When a node exceeds its retry budget or fails to decrease energy, the
/// orchestrator classifies the failure into one of these categories so the
/// runtime can choose a targeted repair action instead of only escalating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationCategory {
    /// Compilation, type, or syntax errors that remain after retries.
    ImplementationError,
    /// Node output violates its behavioral contract or interface seal.
    ContractMismatch,
    /// Model is unable to produce acceptable output for this node's tier.
    InsufficientModelCapability,
    /// Required verifier tools are missing or degraded.
    DegradedSensors,
    /// Node scope does not match ownership or dependency graph structure.
    TopologyMismatch,
}

impl std::fmt::Display for EscalationCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EscalationCategory::ImplementationError => write!(f, "implementation_error"),
            EscalationCategory::ContractMismatch => write!(f, "contract_mismatch"),
            EscalationCategory::InsufficientModelCapability => {
                write!(f, "insufficient_model_capability")
            }
            EscalationCategory::DegradedSensors => write!(f, "degraded_sensors"),
            EscalationCategory::TopologyMismatch => write!(f, "topology_mismatch"),
        }
    }
}

/// PSP-5 Phase 5: Repair action chosen by the orchestrator after classifying
/// non-convergence.
///
/// Actions are ordered from least destructive (retry with evidence) to most
/// disruptive (user escalation).  The orchestrator picks the first action
/// that is safe given the current evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewriteAction {
    /// Re-attempt the node with a correction prompt grounded in verifier output.
    GroundedRetry {
        /// Human-readable summary of the evidence fed back to the LLM.
        evidence_summary: String,
    },
    /// Refine or tighten the node's behavioral contract or interface seal.
    ContractRepair {
        /// Which contract fields need adjustment.
        fields: Vec<String>,
    },
    /// Promote the node to a higher-capability model tier.
    CapabilityPromotion {
        /// Current tier.
        from_tier: ModelTier,
        /// Proposed tier.
        to_tier: ModelTier,
    },
    /// Attempt to recover a degraded sensor or stop with explicit degradation.
    SensorRecovery {
        /// Stages that are degraded.
        degraded_stages: Vec<String>,
    },
    /// Stop the node with an explicit degraded-validation marker rather than
    /// claiming false stability.
    DegradedValidationStop {
        /// Reason the runtime is stopping without full verification.
        reason: String,
    },
    /// Split the current node by ownership closure into smaller nodes.
    NodeSplit {
        /// Proposed child node IDs after splitting.
        proposed_children: Vec<String>,
    },
    /// Insert an interface node between this node and its dependents.
    InterfaceInsertion {
        /// The boundary that motivated the insertion.
        boundary: String,
    },
    /// Re-plan a local subgraph rooted at the failing node.
    SubgraphReplan {
        /// Node IDs in the affected subgraph.
        affected_nodes: Vec<String>,
    },
    /// Escalate to the user with stored evidence (last resort).
    UserEscalation {
        /// Structured evidence for the user.
        evidence: String,
    },
}

impl std::fmt::Display for RewriteAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RewriteAction::GroundedRetry { .. } => write!(f, "grounded_retry"),
            RewriteAction::ContractRepair { .. } => write!(f, "contract_repair"),
            RewriteAction::CapabilityPromotion { .. } => write!(f, "capability_promotion"),
            RewriteAction::SensorRecovery { .. } => write!(f, "sensor_recovery"),
            RewriteAction::DegradedValidationStop { .. } => {
                write!(f, "degraded_validation_stop")
            }
            RewriteAction::NodeSplit { .. } => write!(f, "node_split"),
            RewriteAction::InterfaceInsertion { .. } => write!(f, "interface_insertion"),
            RewriteAction::SubgraphReplan { .. } => write!(f, "subgraph_replan"),
            RewriteAction::UserEscalation { .. } => write!(f, "user_escalation"),
        }
    }
}

/// PSP-5 Phase 5: Sheaf validator class.
///
/// Each class checks a different cross-node consistency property after child
/// nodes converge and before the parent node is committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SheafValidatorClass {
    /// Exported symbols, trait impls, and module imports match dependency interfaces.
    ExportImportConsistency,
    /// Repository dependency edges remain acyclic and node-local changes do not
    /// introduce invalid module or package references.
    DependencyGraphConsistency,
    /// JSON schemas, API types, and serialization contracts remain compatible.
    SchemaContractCompatibility,
    /// Plugin-selected build targets remain satisfiable for the affected subgraph.
    BuildGraphConsistency,
    /// Failing tests are attributed to the owning node or interface boundary.
    TestOwnershipConsistency,
    /// FFI layers, generated clients, and protocol bindings across plugin boundaries.
    CrossLanguageBoundary,
    /// Repository-wide invariants and forbidden patterns still hold.
    PolicyInvariantConsistency,
}

impl std::fmt::Display for SheafValidatorClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SheafValidatorClass::ExportImportConsistency => write!(f, "export_import"),
            SheafValidatorClass::DependencyGraphConsistency => write!(f, "dependency_graph"),
            SheafValidatorClass::SchemaContractCompatibility => write!(f, "schema_contract"),
            SheafValidatorClass::BuildGraphConsistency => write!(f, "build_graph"),
            SheafValidatorClass::TestOwnershipConsistency => write!(f, "test_ownership"),
            SheafValidatorClass::CrossLanguageBoundary => write!(f, "cross_language"),
            SheafValidatorClass::PolicyInvariantConsistency => write!(f, "policy_invariant"),
        }
    }
}

/// PSP-5 Phase 5: Result of a single sheaf validation pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheafValidationResult {
    /// Which validator class produced this result.
    pub validator_class: SheafValidatorClass,
    /// Plugin that owns the validator (if any).
    pub plugin_source: Option<String>,
    /// Whether the validation passed.
    pub passed: bool,
    /// Boundaries that were validated.
    pub validated_boundaries: Vec<String>,
    /// Evidence summary when validation fails.
    pub evidence_summary: String,
    /// Files or interfaces affected by the failure.
    pub affected_files: Vec<String>,
    /// Energy contribution to V_sheaf.
    pub v_sheaf_contribution: f32,
    /// Node IDs recommended for requeue on failure.
    pub requeue_targets: Vec<String>,
}

impl SheafValidationResult {
    /// Create a passing result.
    pub fn passed(class: SheafValidatorClass, boundaries: Vec<String>) -> Self {
        Self {
            validator_class: class,
            plugin_source: None,
            passed: true,
            validated_boundaries: boundaries,
            evidence_summary: String::new(),
            affected_files: Vec::new(),
            v_sheaf_contribution: 0.0,
            requeue_targets: Vec::new(),
        }
    }

    /// Create a failing result with evidence.
    pub fn failed(
        class: SheafValidatorClass,
        evidence: impl Into<String>,
        affected: Vec<String>,
        requeue: Vec<String>,
        v_sheaf: f32,
    ) -> Self {
        Self {
            validator_class: class,
            plugin_source: None,
            passed: false,
            validated_boundaries: Vec::new(),
            evidence_summary: evidence.into(),
            affected_files: affected,
            v_sheaf_contribution: v_sheaf,
            requeue_targets: requeue,
        }
    }
}

/// PSP-5 Phase 5: Full escalation report assembled by the orchestrator.
///
/// Captures everything needed for persistence, user display, and later
/// resume: the failing node, the classified category, the chosen repair
/// action, verifier evidence, and energy snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationReport {
    /// Node that triggered escalation.
    pub node_id: String,
    /// Session this report belongs to.
    pub session_id: String,
    /// Classified failure category.
    pub category: EscalationCategory,
    /// Repair action chosen (or UserEscalation if none was safe).
    pub action: RewriteAction,
    /// Energy at the time of escalation.
    pub energy_snapshot: EnergyComponents,
    /// Verifier stage outcomes at the time of escalation.
    pub stage_outcomes: Vec<StageOutcome>,
    /// Human-readable evidence summary.
    pub evidence: String,
    /// Node IDs affected by the chosen action (requeue targets).
    pub affected_node_ids: Vec<String>,
    /// Timestamp (epoch seconds).
    pub timestamp: i64,
}

/// PSP-5 Phase 5: Record of a local graph rewrite applied by the orchestrator.
///
/// Stored in the ledger so Phase 8 resume can replay or audit rewrite history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewriteRecord {
    /// Node that was rewritten.
    pub node_id: String,
    /// Session this record belongs to.
    pub session_id: String,
    /// The rewrite action that was applied.
    pub action: RewriteAction,
    /// Category that triggered the rewrite.
    pub category: EscalationCategory,
    /// Node IDs that were requeued as a result.
    pub requeued_nodes: Vec<String>,
    /// Node IDs that were newly inserted (e.g. interface insertion).
    pub inserted_nodes: Vec<String>,
    /// Timestamp (epoch seconds).
    pub timestamp: i64,
}

/// PSP-5 Phase 5: Targeted requeue entry.
///
/// When a sheaf validator or escalation identifies a subset of nodes for
/// re-execution, this record tracks the targeting metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetedRequeue {
    /// Node IDs targeted for requeue.
    pub node_ids: Vec<String>,
    /// Reason for the requeue (validator class or escalation category).
    pub reason: String,
    /// Evidence that justified targeting these specific nodes.
    pub evidence: String,
    /// Sheaf validation results that triggered this requeue (if any).
    pub sheaf_results: Vec<SheafValidationResult>,
    /// Timestamp (epoch seconds).
    pub timestamp: i64,
}

// =============================================================================
// PSP-5 Phase 6: Provisional Branch Ledger and Interface-Sealed Speculation
// =============================================================================

/// PSP-5 Phase 6: State of a provisional branch.
///
/// Provisional branches store speculative child work separately from committed
/// ledger state.  A branch transitions through Active → Sealed → Merged or
/// Flushed, and never enters committed node state without explicit merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionalBranchState {
    /// Branch is executing speculatively; verification has not yet completed.
    Active,
    /// Interface for the branch's parent node is sealed; child work may proceed.
    Sealed,
    /// Branch was merged into committed state after parent met stability threshold.
    Merged,
    /// Branch was discarded because parent verification failed.
    Flushed,
}

impl std::fmt::Display for ProvisionalBranchState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProvisionalBranchState::Active => write!(f, "active"),
            ProvisionalBranchState::Sealed => write!(f, "sealed"),
            ProvisionalBranchState::Merged => write!(f, "merged"),
            ProvisionalBranchState::Flushed => write!(f, "flushed"),
        }
    }
}

/// PSP-5 Phase 6: Provisional branch tracking speculative child work.
///
/// Created before speculative generation begins so the runtime can track
/// branch lifecycle, enforce seal prerequisites, and flush on parent failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionalBranch {
    /// Unique branch identifier.
    pub branch_id: String,
    /// Session this branch belongs to.
    pub session_id: String,
    /// The node executing speculatively in this branch.
    pub node_id: String,
    /// Parent node whose interface this branch depends on.
    pub parent_node_id: String,
    /// Current branch state.
    pub state: ProvisionalBranchState,
    /// SHA-256 hash of the parent interface seal this branch depends on.
    /// `None` if the parent has not yet produced a seal.
    pub parent_seal_hash: Option<[u8; 32]>,
    /// Sandbox workspace directory (if verification ran in sandbox).
    pub sandbox_dir: Option<String>,
    /// Timestamp of branch creation (epoch seconds).
    pub created_at: i64,
    /// Timestamp of last state transition (epoch seconds).
    pub updated_at: i64,
}

impl ProvisionalBranch {
    /// Create a new active provisional branch.
    pub fn new(
        branch_id: impl Into<String>,
        session_id: impl Into<String>,
        node_id: impl Into<String>,
        parent_node_id: impl Into<String>,
    ) -> Self {
        let now = epoch_secs();
        Self {
            branch_id: branch_id.into(),
            session_id: session_id.into(),
            node_id: node_id.into(),
            parent_node_id: parent_node_id.into(),
            state: ProvisionalBranchState::Active,
            parent_seal_hash: None,
            sandbox_dir: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Whether this branch is still eligible for merge (active or sealed).
    pub fn is_live(&self) -> bool {
        matches!(
            self.state,
            ProvisionalBranchState::Active | ProvisionalBranchState::Sealed
        )
    }

    /// Whether this branch has been discarded.
    pub fn is_flushed(&self) -> bool {
        self.state == ProvisionalBranchState::Flushed
    }
}

/// PSP-5 Phase 6: Parent → child branch lineage record.
///
/// Records the dependency edge between a parent branch (or committed node)
/// and a child provisional branch.  Used by flush propagation to find all
/// descendants that must be discarded when a parent fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchLineage {
    /// Unique lineage record ID.
    pub lineage_id: String,
    /// Parent branch ID (or committed node ID if the parent is committed).
    pub parent_branch_id: String,
    /// Child branch ID.
    pub child_branch_id: String,
    /// Whether the dependency is on the parent's sealed interface (vs. full output).
    pub depends_on_seal: bool,
}

/// PSP-5 Phase 6: Record of a sealed interface produced by a node.
///
/// An interface seal is a hash over the exported signatures, schemas, or symbol
/// inventories that downstream nodes depend on.  Once sealed, the interface is
/// immutable within the current SRBN iteration — dependent context is assembled
/// from the seal rather than from mutable parent implementation files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceSealRecord {
    /// Unique seal identifier.
    pub seal_id: String,
    /// Session this seal belongs to.
    pub session_id: String,
    /// Node that produced (and owns) this seal.
    pub node_id: String,
    /// Path of the sealed artifact (relative to workspace).
    pub sealed_path: String,
    /// The kind of structural artifact that was sealed.
    pub artifact_kind: ArtifactKind,
    /// SHA-256 hash of the sealed content.
    pub seal_hash: [u8; 32],
    /// Monotonically increasing version (incremented on re-seal after parent retry).
    pub version: u32,
    /// Timestamp of seal creation (epoch seconds).
    pub created_at: i64,
}

impl InterfaceSealRecord {
    /// Create a new seal from existing structural digest data.
    pub fn from_digest(
        session_id: impl Into<String>,
        node_id: impl Into<String>,
        digest: &StructuralDigest,
    ) -> Self {
        let nid = node_id.into();
        let sid = session_id.into();
        let seal_id = format!("seal_{}_{}", nid, digest.source_path);
        Self {
            seal_id,
            session_id: sid,
            node_id: nid,
            sealed_path: digest.source_path.clone(),
            artifact_kind: digest.artifact_kind,
            seal_hash: digest.hash,
            version: digest.version,
            created_at: epoch_secs(),
        }
    }

    /// Check whether this seal matches a given digest hash.
    pub fn matches_hash(&self, hash: &[u8; 32]) -> bool {
        self.seal_hash == *hash
    }
}

/// PSP-5 Phase 6: Record of a branch flush decision.
///
/// Persisted so that resume and status surfaces can show why speculative work
/// was discarded and which nodes need re-execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchFlushRecord {
    /// Unique flush record ID.
    pub flush_id: String,
    /// Session this flush belongs to.
    pub session_id: String,
    /// Parent node whose failure triggered the flush.
    pub parent_node_id: String,
    /// Branch IDs that were flushed.
    pub flushed_branch_ids: Vec<String>,
    /// Node IDs that should be requeued after the parent stabilizes.
    pub requeue_node_ids: Vec<String>,
    /// Human-readable reason for the flush.
    pub reason: String,
    /// Timestamp of the flush decision (epoch seconds).
    pub created_at: i64,
}

impl BranchFlushRecord {
    /// Create a new flush record.
    pub fn new(
        session_id: impl Into<String>,
        parent_node_id: impl Into<String>,
        flushed_branch_ids: Vec<String>,
        requeue_node_ids: Vec<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            flush_id: format!("flush_{}", uuid_v4()),
            session_id: session_id.into(),
            parent_node_id: parent_node_id.into(),
            flushed_branch_ids,
            requeue_node_ids,
            reason: reason.into(),
            created_at: epoch_secs(),
        }
    }
}

/// PSP-5 Phase 6: Dependency tracking for nodes blocked on a parent seal.
///
/// When a child node depends on a parent's sealed interface that has not yet
/// been produced, the child is registered as a blocked dependent.  Once the
/// parent seals its interface, blocked dependents are unblocked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedDependency {
    /// Child node that is blocked.
    pub child_node_id: String,
    /// Parent node whose seal the child requires.
    pub parent_node_id: String,
    /// Sealed interface paths the child depends on.
    pub required_seal_paths: Vec<String>,
    /// Timestamp when the block was registered (epoch seconds).
    pub blocked_at: i64,
}

impl BlockedDependency {
    /// Create a new blocked dependency record.
    pub fn new(
        child_node_id: impl Into<String>,
        parent_node_id: impl Into<String>,
        required_seal_paths: Vec<String>,
    ) -> Self {
        Self {
            child_node_id: child_node_id.into(),
            parent_node_id: parent_node_id.into(),
            required_seal_paths,
            blocked_at: epoch_secs(),
        }
    }
}

// =========================================================================
// Plan Revision and Repair Domain Types
// =========================================================================

/// Status of a plan revision within a session.
///
/// Each session may produce multiple plan revisions as the architect responds
/// to verification failures, scope changes, or governance policies.  Only one
/// revision is active at any time; previous revisions are superseded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanRevisionStatus {
    /// The revision is the current active plan driving execution.
    #[default]
    Active,
    /// A newer revision has replaced this one.
    Superseded,
    /// The revision was explicitly abandoned (e.g., user abort).
    Cancelled,
}

impl std::fmt::Display for PlanRevisionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Superseded => write!(f, "superseded"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// A single plan revision within a session.
///
/// Tracks the evolution of the architect's plan over time.  When the verifier
/// or governance policy triggers a replan, a new `PlanRevision` is created,
/// the previous one is marked `Superseded`, and the new revision becomes
/// the active plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanRevision {
    /// Unique revision identifier.
    pub revision_id: String,
    /// Session this revision belongs to.
    pub session_id: String,
    /// Monotonically-increasing sequence number within the session (1-based).
    pub sequence: u32,
    /// The plan content.
    pub plan: TaskPlan,
    /// Why this revision was created (`"initial"`, `"verification_failure"`,
    /// `"scope_change"`, `"governance_budget_exceeded"`, …).
    pub reason: String,
    /// If this revision supersedes an earlier one, its ID.
    pub supersedes: Option<String>,
    /// Current status of this revision.
    pub status: PlanRevisionStatus,
    /// Epoch seconds when this revision was created.
    pub created_at: i64,
}

impl PlanRevision {
    /// Create the initial plan revision for a session.
    pub fn initial(session_id: impl Into<String>, plan: TaskPlan) -> Self {
        Self {
            revision_id: uuid_v4(),
            session_id: session_id.into(),
            sequence: 1,
            plan,
            reason: "initial".to_string(),
            supersedes: None,
            status: PlanRevisionStatus::Active,
            created_at: epoch_secs(),
        }
    }

    /// Create a successor revision that supersedes `previous`.
    pub fn successor(previous: &PlanRevision, plan: TaskPlan, reason: impl Into<String>) -> Self {
        Self {
            revision_id: uuid_v4(),
            session_id: previous.session_id.clone(),
            sequence: previous.sequence + 1,
            plan,
            reason: reason.into(),
            supersedes: Some(previous.revision_id.clone()),
            status: PlanRevisionStatus::Active,
            created_at: epoch_secs(),
        }
    }

    /// Whether this is the current active revision.
    pub fn is_active(&self) -> bool {
        self.status == PlanRevisionStatus::Active
    }
}

/// Adaptive planning policy that selects the agent phase stack
/// based on task scale and workspace type.
///
/// Each variant maps to a different level of orchestration complexity:
/// - `LocalEdit` — Actuator + Verifier only; no architect needed
/// - `FeatureIncrement` — Architect + Actuator + Verifier
/// - `LargeFeature` — Full 4-agent stack with Speculator
/// - `GreenfieldBuild` — Full stack with workspace-setup node first
/// - `ArchitecturalRevision` — Architect + Speculator first, then execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PlanningPolicy {
    /// Small, localized change: skip architect planning.
    LocalEdit,
    /// Mid-size feature: architect decomposes, actuator implements.
    #[default]
    FeatureIncrement,
    /// Large feature: full SRBN loop with speculative execution.
    LargeFeature,
    /// New project: full stack with bootstrap ordering.
    GreenfieldBuild,
    /// Cross-cutting redesign: plan-first, execute later.
    ArchitecturalRevision,
}

impl PlanningPolicy {
    /// Whether this policy requires architect planning.
    pub fn needs_architect(&self) -> bool {
        !matches!(self, Self::LocalEdit)
    }

    /// Whether this policy activates the speculator.
    pub fn needs_speculator(&self) -> bool {
        matches!(
            self,
            Self::LargeFeature | Self::GreenfieldBuild | Self::ArchitecturalRevision
        )
    }
}

/// A scoping document that constrains what the architect may plan.
///
/// The `FeatureCharter` sits above individual task plans and provides
/// boundaries: maximum module count, maximum files, language policy,
/// and a human-readable description of the intended outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureCharter {
    /// Unique charter identifier (typically per session).
    pub charter_id: String,
    /// Session ID.
    pub session_id: String,
    /// Human-readable scope description (the user's original request).
    pub scope_description: String,
    /// Maximum number of modules/nodes the architect may produce.
    pub max_modules: Option<u32>,
    /// Maximum total files the plan may create.
    pub max_files: Option<u32>,
    /// Maximum plan revisions before hard escalation.
    pub max_revisions: Option<u32>,
    /// Language or plugin constraint (e.g. `"rust"`, `"python"`).
    pub language_constraint: Option<String>,
    /// Epoch seconds when the charter was created.
    pub created_at: i64,
}

impl FeatureCharter {
    /// Create a new charter with just a scope description.
    pub fn new(session_id: impl Into<String>, scope_description: impl Into<String>) -> Self {
        Self {
            charter_id: uuid_v4(),
            session_id: session_id.into(),
            scope_description: scope_description.into(),
            max_modules: None,
            max_files: None,
            max_revisions: None,
            language_constraint: None,
            created_at: epoch_secs(),
        }
    }
}

/// A bounded repair unit that records what was changed during a correction.
///
/// Instead of raw `last_written_file` tracking, every correction pass creates
/// a `RepairFootprint` that records the affected files, applied bundle,
/// verification result before/after, and the node being repaired.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairFootprint {
    /// Unique footprint identifier.
    pub footprint_id: String,
    /// Session ID.
    pub session_id: String,
    /// Node ID being repaired.
    pub node_id: String,
    /// Which plan revision was active when the repair happened.
    pub revision_id: String,
    /// Correction attempt number within this node (1-based).
    pub attempt: u32,
    /// Files that were modified by the repair bundle.
    pub affected_files: Vec<String>,
    /// The artifact bundle applied during this repair.
    pub applied_bundle: ArtifactBundle,
    /// Brief summary of what was wrong (from verifier output).
    pub diagnosis: String,
    /// Whether the repair resolved the issue.
    pub resolved: bool,
    /// Epoch seconds.
    pub created_at: i64,
}

impl RepairFootprint {
    /// Create a new repair footprint.
    pub fn new(
        session_id: impl Into<String>,
        node_id: impl Into<String>,
        revision_id: impl Into<String>,
        attempt: u32,
        bundle: &ArtifactBundle,
        diagnosis: impl Into<String>,
    ) -> Self {
        let affected_files = bundle
            .affected_paths()
            .into_iter()
            .map(String::from)
            .collect();
        Self {
            footprint_id: uuid_v4(),
            session_id: session_id.into(),
            node_id: node_id.into(),
            revision_id: revision_id.into(),
            attempt,
            affected_files,
            applied_bundle: bundle.clone(),
            diagnosis: diagnosis.into(),
            resolved: false,
            created_at: epoch_secs(),
        }
    }

    /// Mark this footprint as having resolved the issue.
    pub fn mark_resolved(&mut self) {
        self.resolved = true;
    }
}

/// Declared dependency expectations for a planned task.
///
/// Used during verification to confirm that the environment matches what
/// the architect assumed when producing the plan (e.g. required packages,
/// expected setup commands, or required toolchain version).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DependencyExpectation {
    /// Packages or crates the task expects to be available.
    pub required_packages: Vec<String>,
    /// Setup commands that must have succeeded before this task runs.
    pub setup_commands: Vec<String>,
    /// Minimum toolchain version string (e.g. `"1.75"` for Rust).
    pub min_toolchain_version: Option<String>,
}

/// Budget envelope for plan execution.
///
/// Tracks cost, step, and revision budgets for a session.  The governance
/// layer checks these limits before allowing further execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetEnvelope {
    /// Session ID.
    pub session_id: String,
    /// Maximum number of node execution steps allowed.
    pub max_steps: Option<u32>,
    /// Steps consumed so far.
    pub steps_used: u32,
    /// Maximum number of plan revisions allowed.
    pub max_revisions: Option<u32>,
    /// Revisions consumed so far.
    pub revisions_used: u32,
    /// Maximum total cost in USD.
    pub max_cost_usd: Option<f64>,
    /// Cost consumed so far.
    pub cost_used_usd: f64,
}

impl BudgetEnvelope {
    /// Create a new budget envelope with no limits.
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            max_steps: None,
            steps_used: 0,
            max_revisions: None,
            revisions_used: 0,
            max_cost_usd: None,
            cost_used_usd: 0.0,
        }
    }

    /// Whether the step budget is exhausted.
    pub fn steps_exhausted(&self) -> bool {
        self.max_steps.is_some_and(|max| self.steps_used >= max)
    }

    /// Whether the revision budget is exhausted.
    pub fn revisions_exhausted(&self) -> bool {
        self.max_revisions
            .is_some_and(|max| self.revisions_used >= max)
    }

    /// Whether the cost budget is exhausted.
    pub fn cost_exhausted(&self) -> bool {
        self.max_cost_usd
            .is_some_and(|max| self.cost_used_usd >= max)
    }

    /// Whether any budget limit has been exceeded.
    pub fn any_exhausted(&self) -> bool {
        self.steps_exhausted() || self.revisions_exhausted() || self.cost_exhausted()
    }

    /// Record a step.
    pub fn record_step(&mut self) {
        self.steps_used += 1;
    }

    /// Record a plan revision.
    pub fn record_revision(&mut self) {
        self.revisions_used += 1;
    }

    /// Record cost.
    pub fn record_cost(&mut self, usd: f64) {
        self.cost_used_usd += usd;
    }
}

/// Helper: current epoch seconds.
fn epoch_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Helper: generate a UUID v4 string (simplified).
fn uuid_v4() -> String {
    // Use timestamp + random-ish counter for unique IDs without pulling uuid crate
    // The orchestrator and ledger layers use the `uuid` crate directly when available.
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    now.as_nanos().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

// =============================================================================
// PSP-7: Runtime Barrier Types
// =============================================================================

/// Result state from the typed parse pipeline (PSP-7 Layers A-E).
///
/// Every LLM response is classified into one of these states. The correction
/// loop and telemetry both key off this enum instead of ad-hoc Option checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseResultState {
    /// Layer C: strict JSON parse succeeded and semantic validation passed.
    StrictJsonOk,
    /// Layer D: tolerant file-marker recovery produced a valid bundle.
    TolerantRecoveryOk,
    /// No structured payload could be extracted from the response at all.
    NoStructuredPayload,
    /// JSON parsed but failed schema validation (missing required fields, wrong types).
    SchemaInvalid,
    /// Parsed and schema-valid but rejected by semantic validation (Layer E):
    /// unknown output files, disallowed commands, ownership violations, etc.
    SemanticallyRejected,
    /// Bundle is empty — parsed successfully but contained zero artifacts.
    EmptyBundle,
}

impl ParseResultState {
    /// Whether this state represents a usable bundle.
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::StrictJsonOk | Self::TolerantRecoveryOk)
    }
}

impl std::fmt::Display for ParseResultState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StrictJsonOk => write!(f, "strict_json_ok"),
            Self::TolerantRecoveryOk => write!(f, "tolerant_recovery_ok"),
            Self::NoStructuredPayload => write!(f, "no_structured_payload"),
            Self::SchemaInvalid => write!(f, "schema_invalid"),
            Self::SemanticallyRejected => write!(f, "semantically_rejected"),
            Self::EmptyBundle => write!(f, "empty_bundle"),
        }
    }
}

/// Retry classification for correction-loop failures (PSP-7 §3.3).
///
/// When a parse or semantic check fails, the correction loop classifies the
/// failure to decide between retrying, retargeting, or escalating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryClassification {
    /// Response was malformed — retry with schema-clarification feedback.
    MalformedRetry,
    /// Artifacts targeted wrong files — retarget with ownership guidance.
    Retarget,
    /// LLM added unrequested support files — retry with legal-files guidance.
    SupportFileViolation,
    /// Failure is structural enough that replanning is needed.
    Replan,
    /// Budget is exhausted — cannot retry further.
    BudgetExhausted,
}

impl std::fmt::Display for RetryClassification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MalformedRetry => write!(f, "malformed_retry"),
            Self::Retarget => write!(f, "retarget"),
            Self::SupportFileViolation => write!(f, "support_file_violation"),
            Self::Replan => write!(f, "replan"),
            Self::BudgetExhausted => write!(f, "budget_exhausted"),
        }
    }
}

/// Telemetry record for a single correction attempt (PSP-7 §6).
///
/// Captures the full pipeline state for each correction round-trip so the
/// store can reconstruct exactly what happened during convergence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionAttemptRecord {
    /// Which correction attempt within this node (1-based).
    pub attempt: u32,
    /// Parse result state from the typed pipeline.
    pub parse_state: ParseResultState,
    /// Retry classification (None if parse succeeded).
    pub retry_classification: Option<RetryClassification>,
    /// Raw response fingerprint (hash of the LLM response).
    pub response_fingerprint: String,
    /// Raw response byte length.
    pub response_length: usize,
    /// Energy snapshot after this attempt's verification.
    pub energy_after: Option<EnergyComponents>,
    /// Whether the correction was accepted and applied.
    pub accepted: bool,
    /// Human-readable rejection reason (if not accepted).
    pub rejection_reason: Option<String>,
    /// Epoch seconds when this attempt was recorded.
    pub created_at: i64,
}

/// Intent tag for the prompt compiler (PSP-7 §5).
///
/// Each prompt emitted by the system carries an intent that determines which
/// template family and evidence inputs the compiler selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptIntent {
    /// Architect planning for an existing project.
    ArchitectExisting,
    /// Architect planning for a greenfield project.
    ArchitectGreenfield,
    /// Actuator coding (multi-output node).
    ActuatorMultiOutput,
    /// Actuator coding (single-output node).
    ActuatorSingleOutput,
    /// Verifier analysis.
    VerifierAnalysis,
    /// Correction retry after verification failure.
    CorrectionRetry,
    /// Bundle retarget after ownership/path rejection.
    BundleRetarget,
    /// Speculator basic lookahead.
    SpeculatorBasic,
    /// Speculator extended lookahead.
    SpeculatorLookahead,
    /// Solo mode generation.
    SoloGenerate,
    /// Solo mode correction.
    SoloCorrect,
    /// Project name suggestion.
    ProjectNameSuggest,
}

/// Provenance metadata for a compiled prompt (PSP-7 §5).
///
/// Records which template, evidence sources, and plugin fragments contributed
/// to a final prompt so that observers can trace prompt lineage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptProvenance {
    /// The intent that selected the template family.
    pub intent: PromptIntent,
    /// Which plugin contributed correction fragments (if any).
    pub plugin_fragment_source: Option<String>,
    /// Brief names of evidence sources folded into the prompt.
    pub evidence_sources: Vec<String>,
    /// Epoch seconds when the prompt was compiled.
    pub compiled_at: i64,
}

/// A compiled prompt ready for submission to the LLM (PSP-7 §5).
///
/// Replaces raw string concatenation with a typed container that carries
/// the prompt text alongside its provenance metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledPrompt {
    /// The final prompt text to send to the LLM.
    pub text: String,
    /// Provenance metadata for observability.
    pub provenance: PromptProvenance,
}

/// Evidence inputs for the prompt compiler (PSP-7 §5).
///
/// Each prompt family draws from a different subset of these fields.
/// The compiler ignores fields that are irrelevant for the selected intent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptEvidence {
    /// The user's high-level goal or request.
    pub user_goal: Option<String>,
    /// Project structure summary (file tree, detected languages, etc.).
    pub project_summary: Option<String>,
    /// Node-scoped goal for actuator/verifier prompts.
    pub node_goal: Option<String>,
    /// Output files this node is expected to produce.
    pub output_files: Vec<String>,
    /// Context files the node should read.
    pub context_files: Vec<String>,
    /// Verifier diagnostics from the last verification pass.
    pub verifier_diagnostics: Option<String>,
    /// Previous correction attempt records for retry prompts.
    pub previous_attempts: Vec<CorrectionAttemptRecord>,
    /// Plugin-contributed correction guidance.
    pub plugin_correction_fragment: Option<String>,
    /// Legal support files declared by the plugin.
    pub legal_support_files: Vec<String>,
    /// Existing file contents for context injection.
    pub existing_file_contents: Vec<(String, String)>,
    /// Dependency expectations for the current task.
    pub dependency_expectations: Option<DependencyExpectation>,
    /// Bundle that was rejected (for retarget prompts).
    pub rejected_bundle_summary: Option<String>,
    /// Solo mode file path.
    pub solo_file_path: Option<String>,
    /// Solo mode language hint.
    pub solo_language: Option<String>,
    /// Working directory path for context-aware prompts.
    pub working_dir: Option<String>,
    /// Active language plugins (e.g. `["rust", "python"]`).
    pub active_plugins: Vec<String>,
    /// Contract interface signature for actuator/verifier prompts.
    pub interface_signature: Option<String>,
    /// Contract invariants for actuator/verifier prompts.
    pub invariants: Option<String>,
    /// Contract forbidden patterns for actuator/verifier prompts.
    pub forbidden_patterns: Option<String>,
    /// Contract weighted tests for verifier prompts.
    pub weighted_tests: Option<String>,
    /// Workspace import hints for cross-module references.
    pub workspace_import_hints: Option<String>,
    /// Pre-formatted evidence section for architect prompts.
    pub evidence_section: Option<String>,
    /// Error feedback from previous planning attempts.
    pub error_feedback: Option<String>,
}

/// Policy decision for a dependency command (PSP-7 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandPolicyDecision {
    /// Command is allowed.
    Allow,
    /// Command is denied.
    Deny,
    /// Command requires user approval before execution.
    RequireApproval,
}

/// Policy decision for a manifest mutation (PSP-7 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManifestMutationPolicy {
    /// Mutation is allowed.
    Allow,
    /// Mutation is denied.
    Deny,
}

#[cfg(test)]
mod psp5_tests {
    use super::*;

    #[test]
    fn test_execution_mode_default_is_project() {
        assert_eq!(ExecutionMode::default(), ExecutionMode::Project);
    }

    #[test]
    fn test_node_class_default_is_implementation() {
        assert_eq!(NodeClass::default(), NodeClass::Implementation);
    }

    #[test]
    fn test_artifact_bundle_roundtrip() {
        let bundle = ArtifactBundle {
            artifacts: vec![
                ArtifactOperation::Write {
                    path: "src/main.rs".to_string(),
                    content: "fn main() {}".to_string(),
                },
                ArtifactOperation::Diff {
                    path: "src/lib.rs".to_string(),
                    patch: "--- a\n+++ b\n@@ -1 +1 @@\n-old\n+new".to_string(),
                },
            ],
            commands: vec!["cargo build".to_string()],
        };

        let json = serde_json::to_string(&bundle).unwrap();
        let deser: ArtifactBundle = serde_json::from_str(&json).unwrap();

        assert_eq!(deser.len(), 2);
        assert_eq!(deser.writes_count(), 1);
        assert_eq!(deser.diffs_count(), 1);
        assert_eq!(deser.commands.len(), 1);
    }

    #[test]
    fn test_artifact_bundle_validate_empty() {
        let bundle = ArtifactBundle::new();
        assert!(bundle.validate().is_err());
    }

    #[test]
    fn test_artifact_bundle_validate_absolute_path() {
        let bundle = ArtifactBundle {
            artifacts: vec![ArtifactOperation::Write {
                path: "/etc/passwd".to_string(),
                content: "bad".to_string(),
            }],
            commands: vec![],
        };
        assert!(bundle.validate().is_err());
        assert!(bundle.validate().unwrap_err().contains("absolute path"));
    }

    #[test]
    fn test_artifact_bundle_validate_path_traversal() {
        let bundle = ArtifactBundle {
            artifacts: vec![ArtifactOperation::Write {
                path: "../../etc/passwd".to_string(),
                content: "bad".to_string(),
            }],
            commands: vec![],
        };
        assert!(bundle.validate().is_err());
        assert!(bundle.validate().unwrap_err().contains("path traversal"));
    }

    #[test]
    fn test_artifact_bundle_validate_ok() {
        let bundle = ArtifactBundle {
            artifacts: vec![ArtifactOperation::Write {
                path: "src/main.rs".to_string(),
                content: "fn main() {}".to_string(),
            }],
            commands: vec![],
        };
        assert!(bundle.validate().is_ok());
    }

    #[test]
    fn test_artifact_operation_accessors() {
        let write = ArtifactOperation::Write {
            path: "foo.rs".to_string(),
            content: "bar".to_string(),
        };
        assert_eq!(write.path(), "foo.rs");
        assert!(write.is_write());
        assert!(!write.is_diff());

        let diff = ArtifactOperation::Diff {
            path: "baz.rs".to_string(),
            patch: "patch".to_string(),
        };
        assert_eq!(diff.path(), "baz.rs");
        assert!(!diff.is_write());
        assert!(diff.is_diff());
    }

    #[test]
    fn test_affected_paths_deduplication() {
        let bundle = ArtifactBundle {
            artifacts: vec![
                ArtifactOperation::Write {
                    path: "src/main.rs".to_string(),
                    content: "v1".to_string(),
                },
                ArtifactOperation::Diff {
                    path: "src/main.rs".to_string(),
                    patch: "patch".to_string(),
                },
            ],
            commands: vec![],
        };
        assert_eq!(bundle.affected_paths().len(), 1);
    }

    #[test]
    fn test_verification_result_all_passed() {
        let mut result = VerificationResult::default();
        assert!(!result.all_passed()); // all false by default

        result.syntax_ok = true;
        result.build_ok = true;
        result.tests_ok = true;
        assert!(result.all_passed());
    }

    #[test]
    fn test_verification_result_degraded() {
        let result = VerificationResult::degraded("no cargo");
        assert!(result.degraded);
        assert!(!result.all_passed());
        assert_eq!(result.degraded_reason.unwrap(), "no cargo");
    }

    // =========================================================================
    // PSP-5 Phase 2: Ownership Manifest Tests
    // =========================================================================

    #[test]
    fn test_ownership_manifest_assign_and_lookup() {
        let mut manifest = OwnershipManifest::new();
        manifest.assign("src/main.rs", "node_1", "rust", NodeClass::Implementation);
        manifest.assign("src/lib.rs", "node_1", "rust", NodeClass::Implementation);
        manifest.assign("tests/test.rs", "node_2", "rust", NodeClass::Integration);

        // owner_of
        let entry = manifest.owner_of("src/main.rs").unwrap();
        assert_eq!(entry.owner_node_id, "node_1");
        assert_eq!(entry.owner_plugin, "rust");
        assert_eq!(entry.node_class, NodeClass::Implementation);

        assert!(manifest.owner_of("nonexistent.rs").is_none());

        // files_owned_by
        let mut files = manifest.files_owned_by("node_1");
        files.sort();
        assert_eq!(files, vec!["src/lib.rs", "src/main.rs"]);

        let files_2 = manifest.files_owned_by("node_2");
        assert_eq!(files_2, vec!["tests/test.rs"]);

        assert_eq!(manifest.len(), 3);
        assert!(!manifest.is_empty());
    }

    #[test]
    fn test_ownership_manifest_validate_bundle_ok() {
        let mut manifest = OwnershipManifest::new();
        manifest.assign("src/main.rs", "node_1", "rust", NodeClass::Implementation);
        manifest.assign("src/lib.rs", "node_1", "rust", NodeClass::Implementation);

        let bundle = ArtifactBundle {
            artifacts: vec![
                ArtifactOperation::Write {
                    path: "src/main.rs".to_string(),
                    content: "fn main() {}".to_string(),
                },
                ArtifactOperation::Write {
                    path: "src/lib.rs".to_string(),
                    content: "pub fn lib() {}".to_string(),
                },
            ],
            commands: vec![],
        };

        // node_1 owns both files → should pass
        assert!(manifest
            .validate_bundle(&bundle, "node_1", NodeClass::Implementation)
            .is_ok());
    }

    #[test]
    fn test_ownership_manifest_validate_bundle_cross_owner_rejected() {
        let mut manifest = OwnershipManifest::new();
        manifest.assign("src/main.rs", "node_1", "rust", NodeClass::Implementation);
        manifest.assign("src/other.rs", "node_2", "rust", NodeClass::Implementation);

        let bundle = ArtifactBundle {
            artifacts: vec![
                ArtifactOperation::Write {
                    path: "src/main.rs".to_string(),
                    content: "fn main() {}".to_string(),
                },
                ArtifactOperation::Write {
                    path: "src/other.rs".to_string(),
                    content: "fn other() {}".to_string(),
                },
            ],
            commands: vec![],
        };

        // node_1 tries to modify node_2's file → rejected
        let result = manifest.validate_bundle(&bundle, "node_1", NodeClass::Implementation);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Ownership violation"));
    }

    #[test]
    fn test_ownership_manifest_validate_integration_cross_owner_ok() {
        let mut manifest = OwnershipManifest::new();
        manifest.assign("src/main.rs", "node_1", "rust", NodeClass::Implementation);
        manifest.assign("src/other.rs", "node_2", "rust", NodeClass::Implementation);

        let bundle = ArtifactBundle {
            artifacts: vec![
                ArtifactOperation::Write {
                    path: "src/main.rs".to_string(),
                    content: "fn main() {}".to_string(),
                },
                ArtifactOperation::Write {
                    path: "src/other.rs".to_string(),
                    content: "fn other() {}".to_string(),
                },
            ],
            commands: vec![],
        };

        // Integration node can cross ownership boundaries
        let result = manifest.validate_bundle(&bundle, "node_3", NodeClass::Integration);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ownership_manifest_fanout_limit() {
        let manifest = OwnershipManifest::with_fanout_limit(2);

        let bundle = ArtifactBundle {
            artifacts: vec![
                ArtifactOperation::Write {
                    path: "a.rs".to_string(),
                    content: "a".to_string(),
                },
                ArtifactOperation::Write {
                    path: "b.rs".to_string(),
                    content: "b".to_string(),
                },
                ArtifactOperation::Write {
                    path: "c.rs".to_string(),
                    content: "c".to_string(),
                },
            ],
            commands: vec![],
        };

        // 3 artifacts exceeds fanout limit of 2
        let result = manifest.validate_bundle(&bundle, "node_1", NodeClass::Implementation);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("fanout limit"));

        // Exactly at the limit should pass
        let small_bundle = ArtifactBundle {
            artifacts: vec![
                ArtifactOperation::Write {
                    path: "a.rs".to_string(),
                    content: "a".to_string(),
                },
                ArtifactOperation::Write {
                    path: "b.rs".to_string(),
                    content: "b".to_string(),
                },
            ],
            commands: vec![],
        };
        assert!(manifest
            .validate_bundle(&small_bundle, "node_1", NodeClass::Implementation)
            .is_ok());
    }

    #[test]
    fn test_ownership_manifest_assign_new_paths() {
        let mut manifest = OwnershipManifest::new();
        manifest.assign("src/main.rs", "node_1", "rust", NodeClass::Implementation);

        let bundle = ArtifactBundle {
            artifacts: vec![
                ArtifactOperation::Write {
                    path: "src/main.rs".to_string(),
                    content: "existing".to_string(),
                },
                ArtifactOperation::Write {
                    path: "src/new_file.rs".to_string(),
                    content: "new".to_string(),
                },
            ],
            commands: vec![],
        };

        manifest.assign_new_paths(&bundle, "node_1", "rust", NodeClass::Implementation);

        // Existing entry unchanged
        assert_eq!(
            manifest.owner_of("src/main.rs").unwrap().owner_node_id,
            "node_1"
        );
        // New path auto-assigned
        let new_entry = manifest.owner_of("src/new_file.rs").unwrap();
        assert_eq!(new_entry.owner_node_id, "node_1");
        assert_eq!(new_entry.owner_plugin, "rust");
        assert_eq!(manifest.len(), 2);
    }

    // =========================================================================
    // PSP-5: Plan Ownership Closure Validation Tests
    // =========================================================================

    #[test]
    fn test_plan_validate_duplicate_output_files_rejected() {
        let plan = TaskPlan {
            tasks: vec![
                PlannedTask {
                    id: "task_1".into(),
                    goal: "Create math module".into(),
                    output_files: vec!["src/math.py".into(), "tests/test_math.py".into()],
                    ..PlannedTask::new("task_1", "Create math module")
                },
                PlannedTask {
                    id: "task_2".into(),
                    goal: "Create tests".into(),
                    output_files: vec!["tests/test_math.py".into()],
                    ..PlannedTask::new("task_2", "Create tests")
                },
            ],
        };
        let result = plan.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("tests/test_math.py"),
            "Error should mention the duplicate file: {}",
            err
        );
        assert!(
            err.contains("Ownership violation"),
            "Error should mention ownership: {}",
            err
        );
    }

    #[test]
    fn test_plan_validate_unique_output_files_ok() {
        let plan = TaskPlan {
            tasks: vec![
                PlannedTask {
                    id: "task_1".into(),
                    goal: "Create math module".into(),
                    output_files: vec!["src/math.py".into()],
                    ..PlannedTask::new("task_1", "Create math module")
                },
                PlannedTask {
                    id: "test_1".into(),
                    goal: "Tests for math".into(),
                    output_files: vec!["tests/test_math.py".into()],
                    dependencies: vec!["task_1".into()],
                    ..PlannedTask::new("test_1", "Tests for math")
                },
            ],
        };
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn test_plan_validate_context_files_do_not_conflict_with_output_files() {
        // Reading another task's file via context_files is fine
        let plan = TaskPlan {
            tasks: vec![
                PlannedTask {
                    id: "task_1".into(),
                    goal: "Create math module".into(),
                    output_files: vec!["src/math.py".into()],
                    ..PlannedTask::new("task_1", "Create math module")
                },
                PlannedTask {
                    id: "test_1".into(),
                    goal: "Tests for math".into(),
                    context_files: vec!["src/math.py".into()], // reading, not owning
                    output_files: vec!["tests/test_math.py".into()],
                    dependencies: vec!["task_1".into()],
                    ..PlannedTask::new("test_1", "Tests for math")
                },
            ],
        };
        assert!(plan.validate().is_ok());
    }

    // =========================================================================
    // PSP-5 Phase 3: Structural Digests, Context Packages, Provenance Tests
    // =========================================================================

    #[test]
    fn test_structural_digest_from_content() {
        let digest = StructuralDigest::from_content(
            "node_1",
            "src/main.rs",
            ArtifactKind::Signature,
            b"fn main() {}",
        );

        assert_eq!(digest.source_node_id, "node_1");
        assert_eq!(digest.source_path, "src/main.rs");
        assert_eq!(digest.artifact_kind, ArtifactKind::Signature);
        assert_eq!(digest.version, 1);
        assert!(!digest.digest_id.is_empty());
        // Hash must be non-zero
        assert_ne!(digest.hash, [0u8; 32]);
    }

    #[test]
    fn test_structural_digest_matches() {
        let d1 = StructuralDigest::from_content(
            "node_1",
            "src/main.rs",
            ArtifactKind::Signature,
            b"fn main() {}",
        );
        let d2 = StructuralDigest::from_content(
            "node_1",
            "src/main.rs",
            ArtifactKind::Signature,
            b"fn main() {}",
        );
        let d3 = StructuralDigest::from_content(
            "node_1",
            "src/main.rs",
            ArtifactKind::Signature,
            b"fn main() { println!(); }",
        );

        assert!(d1.matches(&d2));
        assert!(!d1.matches(&d3));
    }

    #[test]
    fn test_context_budget_default() {
        let budget = ContextBudget::default();
        assert_eq!(budget.byte_limit, 100 * 1024); // 100KB
        assert_eq!(budget.file_count_limit, 20);
    }

    #[test]
    fn test_restriction_map_for_node() {
        let map = RestrictionMap::for_node("node_1".to_string());
        assert_eq!(map.node_id, "node_1");
        assert!(map.owned_files.is_empty());
        assert!(map.sealed_interfaces.is_empty());
        assert_eq!(map.budget, ContextBudget::default());
    }

    #[test]
    fn test_restriction_map_structural_bytes() {
        let mut map = RestrictionMap::for_node("node_1".to_string());
        let d = StructuralDigest::from_content(
            "n1",
            "src/a.rs",
            ArtifactKind::InterfaceSeal,
            b"content",
        );
        map.structural_digests.push(d);
        // structural_bytes = source_path.len() + 64 per digest + sealed_interfaces * 128
        assert!(map.structural_bytes() > 0);
    }

    #[test]
    fn test_context_package_add_file_within_budget() {
        let mut pkg = ContextPackage::new("node_1".to_string());
        pkg.restriction_map.budget.byte_limit = 1024;

        assert!(pkg.add_file("a.rs", "hello world".to_string()));
        assert_eq!(pkg.included_files.len(), 1);
        assert_eq!(pkg.total_bytes, 11);
        assert!(!pkg.budget_exceeded);
    }

    #[test]
    fn test_context_package_add_file_exceeds_budget() {
        let mut pkg = ContextPackage::new("node_1".to_string());
        pkg.restriction_map.budget.byte_limit = 10;

        let result = pkg.add_file("big.rs", "this is more than ten bytes".to_string());
        assert!(!result);
        assert!(pkg.budget_exceeded);
        // File should not have been added
        assert!(pkg.included_files.is_empty());
    }

    #[test]
    fn test_context_package_provenance() {
        let mut pkg = ContextPackage::new("node_1".to_string());
        pkg.add_file("a.rs", "content".to_string());

        let d = StructuralDigest::from_content("n1", "src/a.rs", ArtifactKind::Signature, b"data");
        pkg.add_structural_digest(d);

        let prov = pkg.provenance();
        assert_eq!(prov.node_id, "node_1");
        assert_eq!(prov.context_package_id, pkg.package_id);
        assert_eq!(prov.included_file_count, 1);
        assert_eq!(prov.structural_digest_hashes.len(), 1);
        assert!(prov.total_bytes > 0);
    }

    #[test]
    fn test_context_provenance_default() {
        let prov = ContextProvenance::default();
        assert!(prov.node_id.is_empty());
        assert!(prov.structural_digest_hashes.is_empty());
        assert_eq!(prov.included_file_count, 0);
    }

    #[test]
    fn test_artifact_kind_display() {
        assert_eq!(format!("{}", ArtifactKind::Signature), "signature");
        assert_eq!(format!("{}", ArtifactKind::InterfaceSeal), "interface_seal");
    }

    #[test]
    fn test_sensor_status_display() {
        assert_eq!(format!("{}", SensorStatus::Available), "available");
        assert_eq!(
            format!(
                "{}",
                SensorStatus::Fallback {
                    actual: "ruff".into(),
                    reason: "primary not found".into()
                }
            ),
            "fallback(ruff)"
        );
        assert_eq!(
            format!(
                "{}",
                SensorStatus::Unavailable {
                    reason: "not installed".into()
                }
            ),
            "unavailable(not installed)"
        );
    }

    #[test]
    fn test_verification_result_no_degraded_stages() {
        let result = VerificationResult {
            syntax_ok: true,
            build_ok: true,
            tests_ok: true,
            lint_ok: true,
            stage_outcomes: vec![StageOutcome {
                stage: "syntax_check".into(),
                passed: true,
                sensor_status: SensorStatus::Available,
                output: None,
            }],
            ..Default::default()
        };
        assert!(result.all_passed());
        assert!(!result.has_degraded_stages());
        assert!(result.degraded_stage_reasons().is_empty());
    }

    #[test]
    fn test_verification_result_with_fallback_blocks_stability() {
        let result = VerificationResult {
            syntax_ok: true,
            build_ok: true,
            tests_ok: true,
            lint_ok: true,
            stage_outcomes: vec![
                StageOutcome {
                    stage: "syntax_check".into(),
                    passed: true,
                    sensor_status: SensorStatus::Available,
                    output: None,
                },
                StageOutcome {
                    stage: "test".into(),
                    passed: true,
                    sensor_status: SensorStatus::Fallback {
                        actual: "python -m pytest".into(),
                        reason: "uv not found".into(),
                    },
                    output: None,
                },
            ],
            ..Default::default()
        };
        // All tools passed but a fallback was used — should flag degraded
        assert!(result.has_degraded_stages());
        let reasons = result.degraded_stage_reasons();
        assert_eq!(reasons.len(), 1);
        assert!(reasons[0].contains("test"));
        assert!(reasons[0].contains("fallback"));
    }

    #[test]
    fn test_verification_result_unavailable_stage() {
        let result = VerificationResult {
            syntax_ok: false,
            stage_outcomes: vec![StageOutcome {
                stage: "lint".into(),
                passed: false,
                sensor_status: SensorStatus::Unavailable {
                    reason: "clippy not installed".into(),
                },
                output: None,
            }],
            ..Default::default()
        };
        assert!(result.has_degraded_stages());
        let reasons = result.degraded_stage_reasons();
        assert!(reasons[0].contains("clippy not installed"));
    }

    #[test]
    fn test_verification_result_mixed_stages() {
        // A realistic result: syntax passed on primary, lint fell back, tests unavailable
        let result = VerificationResult {
            syntax_ok: true,
            tests_ok: false,
            lint_ok: false,
            stage_outcomes: vec![
                StageOutcome {
                    stage: "syntax_check".into(),
                    passed: true,
                    sensor_status: SensorStatus::Available,
                    output: Some("OK".into()),
                },
                StageOutcome {
                    stage: "lint".into(),
                    passed: true,
                    sensor_status: SensorStatus::Fallback {
                        actual: "cargo check".into(),
                        reason: "clippy not found".into(),
                    },
                    output: Some("warnings only".into()),
                },
                StageOutcome {
                    stage: "test".into(),
                    passed: false,
                    sensor_status: SensorStatus::Unavailable {
                        reason: "no test runner".into(),
                    },
                    output: None,
                },
            ],
            ..Default::default()
        };
        assert!(result.has_degraded_stages());
        let reasons = result.degraded_stage_reasons();
        // Both lint (fallback) and test (unavailable) should be degraded
        assert_eq!(reasons.len(), 2);
        assert!(reasons.iter().any(|r| r.contains("lint")));
        assert!(reasons.iter().any(|r| r.contains("test")));
    }

    // =========================================================================
    // Phase 5: Escalation, graph rewrite, and sheaf validator types
    // =========================================================================

    #[test]
    fn test_escalation_category_display() {
        assert_eq!(
            EscalationCategory::ImplementationError.to_string(),
            "implementation_error"
        );
        assert_eq!(
            EscalationCategory::ContractMismatch.to_string(),
            "contract_mismatch"
        );
        assert_eq!(
            EscalationCategory::DegradedSensors.to_string(),
            "degraded_sensors"
        );
    }

    #[test]
    fn test_rewrite_action_grounded_retry() {
        let action = RewriteAction::GroundedRetry {
            evidence_summary: "build failed twice".into(),
        };
        match action {
            RewriteAction::GroundedRetry { evidence_summary } => {
                assert!(evidence_summary.contains("build failed"));
            }
            _ => panic!("Expected GroundedRetry"),
        }
    }

    #[test]
    fn test_rewrite_action_node_split() {
        let action = RewriteAction::NodeSplit {
            proposed_children: vec!["child_a".into(), "child_b".into()],
        };
        match action {
            RewriteAction::NodeSplit { proposed_children } => {
                assert_eq!(proposed_children.len(), 2);
            }
            _ => panic!("Expected NodeSplit"),
        }
    }

    #[test]
    fn test_sheaf_validator_class_display() {
        assert_eq!(
            SheafValidatorClass::DependencyGraphConsistency.to_string(),
            "dependency_graph"
        );
        assert_eq!(
            SheafValidatorClass::CrossLanguageBoundary.to_string(),
            "cross_language"
        );
    }

    #[test]
    fn test_sheaf_validation_result_passed() {
        let result = SheafValidationResult::passed(
            SheafValidatorClass::DependencyGraphConsistency,
            vec!["node_1".into()],
        );
        assert!(result.passed);
        assert_eq!(result.v_sheaf_contribution, 0.0);
        assert!(result.evidence_summary.is_empty());
        assert!(result.requeue_targets.is_empty());
    }

    #[test]
    fn test_sheaf_validation_result_failed() {
        let result = SheafValidationResult::failed(
            SheafValidatorClass::ExportImportConsistency,
            "ownership mismatch on 2 files",
            vec!["src/a.rs".into(), "src/b.rs".into()],
            vec!["node_2".into()],
            0.3,
        );
        assert!(!result.passed);
        assert_eq!(result.v_sheaf_contribution, 0.3);
        assert!(result.evidence_summary.contains("ownership mismatch"));
        assert_eq!(result.affected_files.len(), 2);
        assert_eq!(result.requeue_targets, vec!["node_2"]);
    }

    #[test]
    fn test_escalation_report_roundtrip() {
        let report = EscalationReport {
            node_id: "test_node".into(),
            session_id: "sess_1".into(),
            category: EscalationCategory::TopologyMismatch,
            action: RewriteAction::InterfaceInsertion {
                boundary: "module_boundary".into(),
            },
            energy_snapshot: EnergyComponents::default(),
            stage_outcomes: Vec::new(),
            evidence: "violation at boundary".into(),
            affected_node_ids: vec!["dep_1".into()],
            timestamp: 12345,
        };
        let json = serde_json::to_string(&report).unwrap();
        let deser: EscalationReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.node_id, "test_node");
        assert_eq!(deser.category, EscalationCategory::TopologyMismatch);
        assert_eq!(deser.affected_node_ids.len(), 1);
    }

    #[test]
    fn test_stability_monitor_reset_for_replan() {
        let mut monitor = StabilityMonitor::new();
        monitor.record_energy(0.8);
        monitor.record_energy(0.5);
        monitor.record_failure(ErrorType::Compilation);
        assert_eq!(monitor.attempt_count, 2);

        monitor.reset_for_replan();
        assert_eq!(monitor.attempt_count, 0);
        assert!(!monitor.stable);
        // History is preserved
        assert_eq!(monitor.energy_history.len(), 2);
    }

    #[test]
    fn test_rewrite_record_serialization() {
        let record = RewriteRecord {
            node_id: "n1".into(),
            session_id: "s1".into(),
            action: RewriteAction::SubgraphReplan {
                affected_nodes: vec!["n2".into(), "n3".into()],
            },
            category: EscalationCategory::InsufficientModelCapability,
            requeued_nodes: vec!["n2".into(), "n3".into()],
            inserted_nodes: Vec::new(),
            timestamp: 99999,
        };
        let json = serde_json::to_string(&record).unwrap();
        let deser: RewriteRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.requeued_nodes.len(), 2);
        assert!(deser.inserted_nodes.is_empty());
    }

    // =========================================================================
    // PSP-5 Phase 6: Provisional Branch and Seal Tests
    // =========================================================================

    #[test]
    fn test_provisional_branch_state_display() {
        assert_eq!(ProvisionalBranchState::Active.to_string(), "active");
        assert_eq!(ProvisionalBranchState::Sealed.to_string(), "sealed");
        assert_eq!(ProvisionalBranchState::Merged.to_string(), "merged");
        assert_eq!(ProvisionalBranchState::Flushed.to_string(), "flushed");
    }

    #[test]
    fn test_provisional_branch_lifecycle() {
        let branch = ProvisionalBranch::new("b1", "s1", "node_child", "node_parent");
        assert_eq!(branch.state, ProvisionalBranchState::Active);
        assert!(branch.is_live());
        assert!(!branch.is_flushed());
        assert!(branch.parent_seal_hash.is_none());
        assert!(branch.sandbox_dir.is_none());
        assert!(branch.created_at > 0);
    }

    #[test]
    fn test_provisional_branch_flushed_not_live() {
        let mut branch = ProvisionalBranch::new("b1", "s1", "n1", "p1");
        branch.state = ProvisionalBranchState::Flushed;
        assert!(!branch.is_live());
        assert!(branch.is_flushed());
    }

    #[test]
    fn test_provisional_branch_sealed_is_live() {
        let mut branch = ProvisionalBranch::new("b1", "s1", "n1", "p1");
        branch.state = ProvisionalBranchState::Sealed;
        assert!(branch.is_live());
        assert!(!branch.is_flushed());
    }

    #[test]
    fn test_provisional_branch_serialization() {
        let branch = ProvisionalBranch::new("b1", "s1", "n1", "p1");
        let json = serde_json::to_string(&branch).unwrap();
        let deser: ProvisionalBranch = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.branch_id, "b1");
        assert_eq!(deser.state, ProvisionalBranchState::Active);
    }

    #[test]
    fn test_branch_lineage_serialization() {
        let lineage = BranchLineage {
            lineage_id: "lin_1".into(),
            parent_branch_id: "parent_b".into(),
            child_branch_id: "child_b".into(),
            depends_on_seal: true,
        };
        let json = serde_json::to_string(&lineage).unwrap();
        let deser: BranchLineage = serde_json::from_str(&json).unwrap();
        assert!(deser.depends_on_seal);
        assert_eq!(deser.parent_branch_id, "parent_b");
    }

    #[test]
    fn test_interface_seal_from_digest() {
        let digest = StructuralDigest::from_content(
            "node_iface",
            "src/api.rs",
            ArtifactKind::InterfaceSeal,
            b"pub fn hello() -> String",
        );
        let seal = InterfaceSealRecord::from_digest("sess1", "node_iface", &digest);
        assert_eq!(seal.node_id, "node_iface");
        assert_eq!(seal.sealed_path, "src/api.rs");
        assert!(seal.matches_hash(&digest.hash));
        assert!(!seal.matches_hash(&[0u8; 32]));
    }

    #[test]
    fn test_branch_flush_record() {
        let flush = BranchFlushRecord::new(
            "s1",
            "parent_node",
            vec!["b1".into(), "b2".into()],
            vec!["child1".into(), "child2".into()],
            "Parent failed verification",
        );
        assert!(flush.flush_id.starts_with("flush_"));
        assert_eq!(flush.flushed_branch_ids.len(), 2);
        assert_eq!(flush.requeue_node_ids.len(), 2);
        assert!(flush.created_at > 0);
    }

    #[test]
    fn test_blocked_dependency() {
        let dep = BlockedDependency::new("child_node", "parent_node", vec!["src/api.rs".into()]);
        assert_eq!(dep.child_node_id, "child_node");
        assert_eq!(dep.parent_node_id, "parent_node");
        assert_eq!(dep.required_seal_paths.len(), 1);
        assert!(dep.blocked_at > 0);
    }

    #[test]
    fn test_srbn_node_phase6_fields() {
        let node = SRBNNode::new("n1".into(), "goal".into(), ModelTier::Actuator);
        assert!(node.provisional_branch_id.is_none());
        assert!(node.interface_seal_hash.is_none());
    }

    // =========================================================================
    // Plan Revision, Charter, Repair, and Budget Tests
    // =========================================================================

    #[test]
    fn test_plan_revision_initial() {
        let plan = TaskPlan {
            tasks: vec![PlannedTask::new("t1", "Do something")],
        };
        let rev = PlanRevision::initial("session_1", plan);
        assert_eq!(rev.sequence, 1);
        assert_eq!(rev.reason, "initial");
        assert!(rev.supersedes.is_none());
        assert!(rev.is_active());
        assert_eq!(rev.status, PlanRevisionStatus::Active);
    }

    #[test]
    fn test_plan_revision_successor() {
        let plan1 = TaskPlan {
            tasks: vec![PlannedTask::new("t1", "First")],
        };
        let rev1 = PlanRevision::initial("s1", plan1);

        let plan2 = TaskPlan {
            tasks: vec![PlannedTask::new("t2", "Second")],
        };
        let rev2 = PlanRevision::successor(&rev1, plan2, "verification_failure");

        assert_eq!(rev2.sequence, 2);
        assert_eq!(rev2.reason, "verification_failure");
        assert_eq!(rev2.supersedes, Some(rev1.revision_id.clone()));
        assert!(rev2.is_active());
    }

    #[test]
    fn test_plan_revision_status_display() {
        assert_eq!(PlanRevisionStatus::Active.to_string(), "active");
        assert_eq!(PlanRevisionStatus::Superseded.to_string(), "superseded");
        assert_eq!(PlanRevisionStatus::Cancelled.to_string(), "cancelled");
    }

    #[test]
    fn test_planning_policy_defaults_and_queries() {
        let policy = PlanningPolicy::default();
        assert_eq!(policy, PlanningPolicy::FeatureIncrement);
        assert!(policy.needs_architect());
        assert!(!policy.needs_speculator());

        assert!(!PlanningPolicy::LocalEdit.needs_architect());
        assert!(!PlanningPolicy::LocalEdit.needs_speculator());

        assert!(PlanningPolicy::LargeFeature.needs_architect());
        assert!(PlanningPolicy::LargeFeature.needs_speculator());

        assert!(PlanningPolicy::GreenfieldBuild.needs_architect());
        assert!(PlanningPolicy::GreenfieldBuild.needs_speculator());

        assert!(PlanningPolicy::ArchitecturalRevision.needs_architect());
        assert!(PlanningPolicy::ArchitecturalRevision.needs_speculator());
    }

    #[test]
    fn test_repair_footprint_creation() {
        let bundle = ArtifactBundle {
            artifacts: vec![ArtifactOperation::Write {
                path: "src/fix.rs".into(),
                content: "fixed".into(),
            }],
            commands: vec![],
        };
        let fp = RepairFootprint::new("s1", "node1", "rev1", 1, &bundle, "Syntax error");
        assert_eq!(fp.node_id, "node1");
        assert_eq!(fp.attempt, 1);
        assert_eq!(fp.affected_files, vec!["src/fix.rs"]);
        assert!(!fp.resolved);

        let mut fp = fp;
        fp.mark_resolved();
        assert!(fp.resolved);
    }

    #[test]
    fn test_budget_envelope_tracking() {
        let mut budget = BudgetEnvelope::new("s1");
        budget.max_steps = Some(3);
        budget.max_revisions = Some(2);
        budget.max_cost_usd = Some(1.0);

        assert!(!budget.any_exhausted());

        budget.record_step();
        budget.record_step();
        assert!(!budget.steps_exhausted());
        budget.record_step();
        assert!(budget.steps_exhausted());
        assert!(budget.any_exhausted());
    }

    #[test]
    fn test_budget_envelope_cost_tracking() {
        let mut budget = BudgetEnvelope::new("s1");
        budget.max_cost_usd = Some(0.50);
        budget.record_cost(0.25);
        assert!(!budget.cost_exhausted());
        budget.record_cost(0.30);
        assert!(budget.cost_exhausted());
    }

    #[test]
    fn test_artifact_operation_delete_and_move() {
        let del = ArtifactOperation::Delete {
            path: "src/old.rs".into(),
        };
        assert!(del.is_delete());
        assert!(!del.is_write());
        assert_eq!(del.path(), "src/old.rs");

        let mv = ArtifactOperation::Move {
            from: "src/old.rs".into(),
            to: "src/new.rs".into(),
        };
        assert!(mv.is_move());
        assert!(!mv.is_write());
        assert_eq!(mv.path(), "src/old.rs");
    }

    #[test]
    fn test_artifact_bundle_with_delete_and_move() {
        let bundle = ArtifactBundle {
            artifacts: vec![
                ArtifactOperation::Write {
                    path: "src/new.rs".into(),
                    content: "code".into(),
                },
                ArtifactOperation::Delete {
                    path: "src/old.rs".into(),
                },
                ArtifactOperation::Move {
                    from: "src/a.rs".into(),
                    to: "src/b.rs".into(),
                },
            ],
            commands: vec![],
        };
        assert_eq!(bundle.writes_count(), 1);
        assert_eq!(bundle.deletes_count(), 1);
        assert_eq!(bundle.moves_count(), 1);
        assert!(bundle.validate().is_ok());

        let paths = bundle.affected_paths();
        assert!(paths.contains(&"src/new.rs"));
        assert!(paths.contains(&"src/old.rs"));
        assert!(paths.contains(&"src/a.rs"));
        assert!(paths.contains(&"src/b.rs"));
    }

    #[test]
    fn test_artifact_bundle_move_validation() {
        // Move with traversal in destination should fail
        let bundle = ArtifactBundle {
            artifacts: vec![ArtifactOperation::Move {
                from: "src/a.rs".into(),
                to: "../outside.rs".into(),
            }],
            commands: vec![],
        };
        assert!(bundle.validate().is_err());
    }

    #[test]
    fn test_dependency_expectation_default() {
        let de = DependencyExpectation::default();
        assert!(de.required_packages.is_empty());
        assert!(de.setup_commands.is_empty());
        assert!(de.min_toolchain_version.is_none());
    }

    #[test]
    fn test_planned_task_has_dependency_expectations() {
        let task = PlannedTask::new("t1", "Build module");
        assert!(task.dependency_expectations.required_packages.is_empty());
    }

    #[test]
    fn test_srbn_node_carries_dependency_expectations() {
        let mut task = PlannedTask::new("t1", "Build module");
        task.dependency_expectations = DependencyExpectation {
            required_packages: vec!["serde".to_string(), "tokio".to_string()],
            setup_commands: vec!["cargo fetch".to_string()],
            min_toolchain_version: Some("1.75".to_string()),
        };
        let node = task.to_srbn_node(ModelTier::Actuator);
        assert_eq!(node.dependency_expectations.required_packages.len(), 2);
        assert_eq!(node.dependency_expectations.required_packages[0], "serde");
        assert_eq!(node.dependency_expectations.setup_commands, ["cargo fetch"]);
        assert_eq!(
            node.dependency_expectations
                .min_toolchain_version
                .as_deref(),
            Some("1.75")
        );
    }

    #[test]
    fn test_dependency_expectations_deserialized_from_json() {
        let json = r#"{
            "id": "t1",
            "goal": "Build module",
            "dependency_expectations": {
                "required_packages": ["requests", "pydantic"],
                "setup_commands": [],
                "min_toolchain_version": "3.11"
            }
        }"#;
        let task: PlannedTask = serde_json::from_str(json).unwrap();
        assert_eq!(task.dependency_expectations.required_packages.len(), 2);
        assert_eq!(
            task.dependency_expectations
                .min_toolchain_version
                .as_deref(),
            Some("3.11")
        );
    }

    #[test]
    fn test_dependency_expectations_default_when_omitted() {
        let json = r#"{"id": "t1", "goal": "Build module"}"#;
        let task: PlannedTask = serde_json::from_str(json).unwrap();
        assert!(task.dependency_expectations.required_packages.is_empty());
        assert!(task.dependency_expectations.setup_commands.is_empty());
        assert!(task.dependency_expectations.min_toolchain_version.is_none());
    }

    #[test]
    fn test_node_state_from_display_str_case_insensitive() {
        assert_eq!(
            NodeState::from_display_str("Completed"),
            NodeState::Completed
        );
        assert_eq!(
            NodeState::from_display_str("COMPLETED"),
            NodeState::Completed
        );
        assert_eq!(
            NodeState::from_display_str("completed"),
            NodeState::Completed
        );
        assert_eq!(
            NodeState::from_display_str("TaskQueued"),
            NodeState::TaskQueued
        );
        assert_eq!(
            NodeState::from_display_str("TASKQUEUED"),
            NodeState::TaskQueued
        );
        assert_eq!(NodeState::from_display_str("coding"), NodeState::Coding);
        assert_eq!(NodeState::from_display_str("STABLE"), NodeState::Completed);
        assert_eq!(NodeState::from_display_str("RUNNING"), NodeState::Coding);
        // Unknown strings map to TaskQueued (default)
        assert_eq!(
            NodeState::from_display_str("garbage"),
            NodeState::TaskQueued
        );
    }

    #[test]
    fn test_node_state_display_roundtrip() {
        let states = [
            NodeState::TaskQueued,
            NodeState::Planning,
            NodeState::Coding,
            NodeState::Verifying,
            NodeState::Retry,
            NodeState::SheafCheck,
            NodeState::Committing,
            NodeState::Escalated,
            NodeState::Completed,
            NodeState::Failed,
        ];
        for state in &states {
            let display = state.to_string();
            let parsed = NodeState::from_display_str(&display);
            assert_eq!(parsed, *state, "Roundtrip failed for {:?}", state);
        }
    }

    #[test]
    fn test_node_state_is_success() {
        assert!(NodeState::Completed.is_success());
        assert!(!NodeState::Escalated.is_success());
        assert!(!NodeState::Failed.is_success());
        assert!(!NodeState::Coding.is_success());
    }

    #[test]
    fn test_node_state_is_active() {
        assert!(NodeState::Coding.is_active());
        assert!(NodeState::Verifying.is_active());
        assert!(NodeState::Planning.is_active());
        assert!(NodeState::Retry.is_active());
        assert!(NodeState::SheafCheck.is_active());
        assert!(NodeState::Committing.is_active());
        assert!(!NodeState::Completed.is_active());
        assert!(!NodeState::Escalated.is_active());
        assert!(!NodeState::TaskQueued.is_active());
    }

    #[test]
    fn test_session_outcome_equality() {
        assert_eq!(SessionOutcome::Success, SessionOutcome::Success);
        assert_ne!(SessionOutcome::Success, SessionOutcome::PartialSuccess);
        assert_ne!(SessionOutcome::Success, SessionOutcome::Failed);
        assert_ne!(SessionOutcome::PartialSuccess, SessionOutcome::Failed);
    }

    // PSP-7 type tests

    #[test]
    fn test_parse_result_state_is_ok() {
        assert!(ParseResultState::StrictJsonOk.is_ok());
        assert!(ParseResultState::TolerantRecoveryOk.is_ok());
        assert!(!ParseResultState::NoStructuredPayload.is_ok());
        assert!(!ParseResultState::SchemaInvalid.is_ok());
        assert!(!ParseResultState::SemanticallyRejected.is_ok());
        assert!(!ParseResultState::EmptyBundle.is_ok());
    }

    #[test]
    fn test_parse_result_state_display() {
        assert_eq!(ParseResultState::StrictJsonOk.to_string(), "strict_json_ok");
        assert_eq!(
            ParseResultState::NoStructuredPayload.to_string(),
            "no_structured_payload"
        );
        assert_eq!(
            ParseResultState::SemanticallyRejected.to_string(),
            "semantically_rejected"
        );
    }

    #[test]
    fn test_retry_classification_display() {
        assert_eq!(
            RetryClassification::MalformedRetry.to_string(),
            "malformed_retry"
        );
        assert_eq!(RetryClassification::Retarget.to_string(), "retarget");
        assert_eq!(RetryClassification::Replan.to_string(), "replan");
        assert_eq!(
            RetryClassification::BudgetExhausted.to_string(),
            "budget_exhausted"
        );
    }

    #[test]
    fn test_prompt_intent_serde_roundtrip() {
        let intents = [
            PromptIntent::ArchitectExisting,
            PromptIntent::ActuatorMultiOutput,
            PromptIntent::CorrectionRetry,
            PromptIntent::SoloGenerate,
        ];
        for intent in &intents {
            let json = serde_json::to_string(intent).unwrap();
            let back: PromptIntent = serde_json::from_str(&json).unwrap();
            assert_eq!(*intent, back);
        }
    }

    #[test]
    fn test_task_plan_cycle_detection() {
        let mut a = PlannedTask::new("a", "goal a");
        a.dependencies = vec!["b".to_string()];
        let mut b = PlannedTask::new("b", "goal b");
        b.dependencies = vec!["c".to_string()];
        let mut c = PlannedTask::new("c", "goal c");
        c.dependencies = vec!["a".to_string()];
        let plan = TaskPlan {
            tasks: vec![a, b, c],
        };
        let err = plan.validate().unwrap_err();
        assert!(err.contains("cycle"), "Expected cycle error, got: {err}");
    }

    #[test]
    fn test_task_plan_implicit_dependency_enforcement() {
        // Task B produces "src/lib.rs", Task A reads it but doesn't depend on B
        let mut a = PlannedTask::new("a", "use lib");
        a.context_files = vec!["src/lib.rs".to_string()];
        a.output_files = vec!["src/main.rs".to_string()];
        let mut b = PlannedTask::new("b", "create lib");
        b.output_files = vec!["src/lib.rs".to_string()];

        let mut plan = TaskPlan { tasks: vec![a, b] };
        let err = plan.validate().unwrap_err();
        assert!(
            err.contains("does not declare it as a dependency"),
            "Expected implicit dep error, got: {err}"
        );
        // Fix: add the dependency
        plan.tasks[0].dependencies.push("b".to_string());
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn test_task_plan_valid_acyclic() {
        let a = PlannedTask::new("a", "goal a");
        let mut b = PlannedTask::new("b", "goal b");
        b.dependencies = vec!["a".to_string()];
        let mut c = PlannedTask::new("c", "goal c");
        c.dependencies = vec!["a".to_string(), "b".to_string()];
        let plan = TaskPlan {
            tasks: vec![a, b, c],
        };
        assert!(plan.validate().is_ok());
    }
}
