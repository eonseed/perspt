use super::*;

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
    /// Total energy `V(x) = Σ_comp V_comp` (PSP-8 System 2).
    ///
    /// The five fields are the *derived component rollups* of the single
    /// canonical quadratic energy `V(x) = Σ_e w_e‖r_e(x)‖²`: each already carries
    /// its squared, weighted residual contribution (`V_comp = Σ_{e∈comp} w_e‖r_e‖²`),
    /// so the total is their plain sum. There is no second `α/β/γ` weighting pass —
    /// those per-component weights are folded into the residual weights `w_e` of
    /// the [`crate`]'s energy model before the rollups are formed.
    pub fn total(&self) -> f32 {
        self.v_syn + self.v_str + self.v_log + self.v_boot + self.v_sheaf
    }

    /// Deprecated alias for [`EnergyComponents::total`]. The component rollups are
    /// already weighted, so the `contract` argument is ignored; retained only so
    /// older call sites keep compiling during the migration.
    #[deprecated(note = "weights are folded into the residual model; use total()")]
    pub fn total_weighted(&self, _contract: &BehavioralContract) -> f32 {
        self.total()
    }

    /// Total energy for Solo Mode. Identical to [`EnergyComponents::total`] now
    /// that aggregation carries no separate weights.
    pub fn total_simple(&self) -> f32 {
        self.total()
    }
}
