//! SRBN Types
//!
//! Core types for the Stabilized Recursive Barrier Network.
//! Based on PSP-000004 specification.

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
    /// Maximum retry attempts before escalation
    pub max_retries: usize,
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
        }
    }

    /// Record a new energy value
    pub fn record_energy(&mut self, energy: f32) {
        self.energy_history.push(energy);
        self.attempt_count += 1;
        self.stable = energy < self.stability_epsilon;
    }

    /// Check if we should escalate (exceeded retries without stability)
    pub fn should_escalate(&self) -> bool {
        self.attempt_count >= self.max_retries && !self.stable
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
    /// Last diagnostics from LSP (for correction prompts)
    #[serde(skip)]
    pub last_diagnostics: Vec<lsp_types::Diagnostic>,
    /// Token budget for cost control
    pub token_budget: TokenBudget,
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
            last_diagnostics: Vec::new(),
            token_budget: TokenBudget::default(),
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
}

impl EnergyComponents {
    /// Calculate total energy: V(x) = α*V_syn + β*V_str + γ*V_log
    pub fn total(&self, contract: &BehavioralContract) -> f32 {
        contract.alpha() * self.v_syn + contract.beta() * self.v_str + contract.gamma() * self.v_log
    }
}

// =============================================================================
// Task Plan Types - Structured output from Architect
// =============================================================================

/// Task type classification for planning
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// Implementation code
    Code,
    /// Unit tests
    UnitTest,
    /// Integration/E2E tests
    IntegrationTest,
    /// Refactoring existing code
    Refactor,
    /// Documentation
    Documentation,
}

impl Default for TaskType {
    fn default() -> Self {
        TaskType::Code
    }
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
