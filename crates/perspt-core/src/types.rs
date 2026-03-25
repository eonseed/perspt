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
    /// Get the recommended model for this tier
    /// Default: gemini-flash-lite-latest for all tiers (can be overridden via CLI)
    pub fn default_model(&self) -> &'static str {
        // Use gemini-flash-lite-latest as the default for all tiers
        // This can be overridden per-tier via CLI: --architect-model, --actuator-model, etc.
        Self::default_model_name()
    }

    /// Get the default model name (static, for use when no instance is available)
    pub fn default_model_name() -> &'static str {
        "gemini-flash-lite-latest"
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

    /// Reset failures of a specific type (on success)
    pub fn reset_failures(&mut self, error_type: ErrorType) {
        match error_type {
            ErrorType::Compilation => self.compilation_failures = 0,
            ErrorType::ToolFailure => self.tool_failures = 0,
            ErrorType::ReviewRejection => self.review_rejections = 0,
            ErrorType::Other => self.compilation_failures = 0,
        }
    }

    /// Reset all failure counters
    pub fn reset_all(&mut self) {
        self.compilation_failures = 0;
        self.tool_failures = 0;
        self.review_rejections = 0;
        self.last_error_type = None;
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

    /// Get the current failure count for an error type
    pub fn failure_count(&self, error_type: ErrorType) -> usize {
        match error_type {
            ErrorType::Compilation | ErrorType::Other => self.compilation_failures,
            ErrorType::ToolFailure => self.tool_failures,
            ErrorType::ReviewRejection => self.review_rejections,
        }
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

    /// Check if we should escalate for a specific error type
    pub fn should_escalate_for(&self, error_type: ErrorType) -> bool {
        self.retry_policy.should_escalate(error_type)
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
        }
    }
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
}

impl NodeState {
    /// Check if this is a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            NodeState::Completed | NodeState::Failed | NodeState::Aborted
        )
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
        }
    }

    /// Convert to SRBNNode
    pub fn to_srbn_node(&self, tier: ModelTier) -> SRBNNode {
        let mut node = SRBNNode::new(self.id.clone(), self.goal.clone(), tier);
        node.context_files = self.context_files.iter().map(PathBuf::from).collect();
        node.output_targets = self.output_files.iter().map(PathBuf::from).collect();
        node.contract = self.contract.to_behavioral_contract();
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
}

impl ArtifactOperation {
    /// Get the file path this operation targets
    pub fn path(&self) -> &str {
        match self {
            ArtifactOperation::Write { path, .. } => path,
            ArtifactOperation::Diff { path, .. } => path,
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

    /// Validate the bundle: checks for empty paths and duplicate targets
    pub fn validate(&self) -> Result<(), String> {
        if self.artifacts.is_empty() {
            return Err("Artifact bundle is empty".to_string());
        }

        for (i, op) in self.artifacts.iter().enumerate() {
            if op.path().is_empty() {
                return Err(format!("Artifact {} has empty path", i));
            }
            // Reject absolute paths
            if op.path().starts_with('/') || op.path().starts_with('\\') {
                return Err(format!(
                    "Artifact {} has absolute path '{}', must be relative",
                    i,
                    op.path()
                ));
            }
            // Reject path traversal
            if op.path().contains("..") {
                return Err(format!(
                    "Artifact {} has path traversal in '{}'",
                    i,
                    op.path()
                ));
            }
        }

        Ok(())
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
            artifacts: vec![
                ArtifactOperation::Write {
                    path: "src/main.rs".to_string(),
                    content: "fn main() {}".to_string(),
                },
            ],
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
}
