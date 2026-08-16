use super::*;

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
    /// Goal-completion verdict: judge whether the user's overall goal is met.
    GoalCompletionCheck,
    /// Plan amendment: produce additional tasks to close an unmet goal gap.
    PlanAmendment,
}

/// Structured verdict from the goal-completion check (PSP-8 closed loop).
///
/// In auto mode, when the work graph settles and the deterministic gate passes,
/// a verifier-tier model is asked whether the user's overall intent is met. The
/// verdict drives the controller: `achieved` ends the session; otherwise
/// `missing` seeds an architect re-plan amendment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GoalVerdict {
    /// Whether the user's overall goal is judged fully achieved.
    pub achieved: bool,
    /// Aspects of the goal still missing or incomplete.
    #[serde(default)]
    pub missing: Vec<String>,
    /// Suggested next steps to close the gap.
    #[serde(default)]
    pub next_steps: Vec<String>,
    /// Optional one-line rationale for the verdict.
    #[serde(default)]
    pub rationale: String,
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
    /// Number of previous correction attempts (used when full records are unavailable).
    pub previous_attempt_count: usize,
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
    /// Restriction map context for correction prompts (pre-formatted).
    pub restriction_map_context: Option<String>,
    /// Project file tree for correction prompts (pre-formatted lines).
    pub project_file_tree: Option<String>,
    /// Raw build/test output for correction prompts (truncated).
    pub build_test_output: Option<String>,
    /// Owner plugin name (e.g. "rust", "python") for language-specific guidance.
    pub owner_plugin: Option<String>,
    /// Syntactic energy score from the last verification pass.
    pub energy_v_syn: Option<f32>,
    /// SRBN residual-directed correction instructions (PSP-8 / Paper II),
    /// pre-formatted, derived from the dominant residual clusters in the
    /// verifier output. Steers the actuator with specific fixes rather than
    /// undirected retries.
    #[serde(default)]
    pub directed_corrections: Option<String>,
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
