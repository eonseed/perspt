use super::*;

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
