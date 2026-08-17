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
