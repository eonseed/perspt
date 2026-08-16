use super::*;

/// Record for a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    pub task: String,
    pub working_dir: String,
    pub merkle_root: Option<Vec<u8>>,
    pub detected_toolchain: Option<String>,
    pub status: String,
}

/// Record for node state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStateRecord {
    pub node_id: String,
    pub session_id: String,
    pub state: String,
    pub v_total: f32,
    pub merkle_hash: Option<Vec<u8>>,
    pub attempt_count: i32,
    // PSP-5 Phase 8: Richer node snapshot for resume reconstruction
    pub node_class: Option<String>,
    pub owner_plugin: Option<String>,
    pub goal: Option<String>,
    pub parent_id: Option<String>,
    /// JSON-serialized `Vec<String>`
    pub children: Option<String>,
    pub last_error_type: Option<String>,
    pub committed_at: Option<String>,
}

/// Record for energy history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyRecord {
    pub node_id: String,
    pub session_id: String,
    pub v_syn: f32,
    pub v_str: f32,
    pub v_log: f32,
    pub v_boot: f32,
    pub v_sheaf: f32,
    pub v_total: f32,
}

/// Record for LLM request/response logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequestRecord {
    pub session_id: String,
    pub node_id: Option<String>,
    pub model: String,
    pub prompt: String,
    pub response: String,
    pub tokens_in: i32,
    pub tokens_out: i32,
    pub latency_ms: i32,
}

/// PSP-5 Phase 3: Record for structural digest persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralDigestRecord {
    pub digest_id: String,
    pub session_id: String,
    pub node_id: String,
    pub source_path: String,
    pub artifact_kind: String,
    pub hash: Vec<u8>,
    pub version: i32,
}

/// PSP-5 Phase 3: Record for context provenance persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextProvenanceRecord {
    pub session_id: String,
    pub node_id: String,
    pub context_package_id: String,
    /// JSON-serialized structural digest hashes
    pub structural_hashes: String,
    /// JSON-serialized summary hashes
    pub summary_hashes: String,
    /// JSON-serialized dependency commit hashes
    pub dependency_hashes: String,
    pub included_file_count: i32,
    pub total_bytes: i32,
}

/// PSP-5 Phase 5: Record for escalation report persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationReportRecord {
    pub session_id: String,
    pub node_id: String,
    /// Serialized EscalationCategory
    pub category: String,
    /// JSON-serialized RewriteAction
    pub action: String,
    /// JSON-serialized EnergyComponents
    pub energy_snapshot: String,
    /// JSON-serialized `Vec<StageOutcome>`
    pub stage_outcomes: String,
    /// Human-readable evidence
    pub evidence: String,
    /// JSON-serialized `Vec<String>`
    pub affected_node_ids: String,
}

/// PSP-5 Phase 5: Record for local graph rewrite persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewriteRecordRow {
    pub session_id: String,
    pub node_id: String,
    /// JSON-serialized RewriteAction
    pub action: String,
    /// Serialized EscalationCategory
    pub category: String,
    /// JSON-serialized `Vec<String>`
    pub requeued_nodes: String,
    /// JSON-serialized `Vec<String>`
    pub inserted_nodes: String,
}

/// PSP-5 Phase 5: Record for sheaf validation result persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheafValidationRow {
    pub session_id: String,
    pub node_id: String,
    pub validator_class: String,
    pub plugin_source: Option<String>,
    pub passed: bool,
    pub evidence_summary: String,
    /// JSON-serialized `Vec<String>`
    pub affected_files: String,
    pub v_sheaf_contribution: f32,
    /// JSON-serialized `Vec<String>`
    pub requeue_targets: String,
}

// =============================================================================
// PSP-5 Phase 6: Provisional Branch, Interface Seal, Branch Flush Records
// =============================================================================

/// PSP-5 Phase 6: Record for provisional branch persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionalBranchRow {
    pub branch_id: String,
    pub session_id: String,
    pub node_id: String,
    pub parent_node_id: String,
    pub state: String,
    pub parent_seal_hash: Option<Vec<u8>>,
    pub sandbox_dir: Option<String>,
}

/// PSP-5 Phase 6: Record for branch lineage persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchLineageRow {
    pub lineage_id: String,
    pub parent_branch_id: String,
    pub child_branch_id: String,
    pub depends_on_seal: bool,
}

/// PSP-5 Phase 6: Record for interface seal persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceSealRow {
    pub seal_id: String,
    pub session_id: String,
    pub node_id: String,
    pub sealed_path: String,
    pub artifact_kind: String,
    pub seal_hash: Vec<u8>,
    pub version: i32,
}

/// PSP-5 Phase 6: Record for branch flush decision persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchFlushRow {
    pub flush_id: String,
    pub session_id: String,
    pub parent_node_id: String,
    /// JSON-serialized `Vec<String>`
    pub flushed_branch_ids: String,
    /// JSON-serialized `Vec<String>`
    pub requeue_node_ids: String,
    pub reason: String,
}

// =============================================================================
// PSP-5 Phase 8: Task Graph and Review Outcome Records
// =============================================================================

/// PSP-5 Phase 8: Record for task graph edges (DAG reconstruction on resume)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraphEdgeRow {
    pub session_id: String,
    pub parent_node_id: String,
    pub child_node_id: String,
    pub edge_type: String,
}

/// PSP-5 Phase 8: Record for review outcome persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewOutcomeRow {
    pub session_id: String,
    pub node_id: String,
    /// One of: "approved", "rejected", "edit_requested", "correction_requested", "skipped"
    pub outcome: String,
    pub reviewer_note: Option<String>,
    /// Energy at time of review decision
    pub energy_at_review: Option<f64>,
    /// Whether verification was degraded when decision was made
    pub degraded: Option<bool>,
    /// Escalation category if the node had been classified
    pub escalation_category: Option<String>,
}

/// PSP-5 Phase 8: Record for verification result snapshot persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResultRow {
    pub session_id: String,
    pub node_id: String,
    /// JSON-serialized VerificationResult (full data for resume reconstruction)
    pub result_json: String,
    // Query-friendly summary fields
    pub syntax_ok: bool,
    pub build_ok: bool,
    pub tests_ok: bool,
    pub lint_ok: bool,
    pub diagnostics_count: i32,
    pub tests_passed: i32,
    pub tests_failed: i32,
    pub degraded: bool,
    pub degraded_reason: Option<String>,
}

/// PSP-5 Phase 8: Record for artifact bundle snapshot persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactBundleRow {
    pub session_id: String,
    pub node_id: String,
    /// JSON-serialized ArtifactBundle (full data for resume reconstruction)
    pub bundle_json: String,
    pub artifact_count: i32,
    pub command_count: i32,
    /// JSON-serialized `Vec<String>` of touched file paths
    pub touched_files: String,
}

// =========================================================================
// Plan Revision, Feature Charter, and Repair Footprint Row Types
// =========================================================================

/// Row type for feature_charters table
#[derive(Debug, Clone)]
pub struct FeatureCharterRow {
    pub charter_id: String,
    pub session_id: String,
    pub scope_description: String,
    pub max_modules: Option<i32>,
    pub max_files: Option<i32>,
    pub max_revisions: Option<i32>,
    pub language_constraint: Option<String>,
}

/// Row type for plan_revisions table
#[derive(Debug, Clone)]
pub struct PlanRevisionRow {
    pub revision_id: String,
    pub session_id: String,
    pub sequence: i32,
    pub plan_json: String,
    pub reason: String,
    pub supersedes: Option<String>,
    pub status: String,
}

/// Row type for repair_footprints table
#[derive(Debug, Clone)]
pub struct RepairFootprintRow {
    pub footprint_id: String,
    pub session_id: String,
    pub node_id: String,
    pub revision_id: String,
    pub attempt: i32,
    pub affected_files: String,
    pub bundle_json: String,
    pub diagnosis: String,
    pub resolved: bool,
}

/// Row type for budget_envelopes table
#[derive(Debug, Clone)]
pub struct BudgetEnvelopeRow {
    pub session_id: String,
    pub max_steps: Option<i32>,
    pub steps_used: i32,
    pub max_revisions: Option<i32>,
    pub revisions_used: i32,
    pub max_cost_usd: Option<f64>,
    pub cost_used_usd: f64,
}

// =========================================================================
// PSP-7: SRBN Step Records and Correction Attempt Row Types
// =========================================================================

/// PSP-7: Record for a single orchestration step transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrbnStepRecord {
    pub session_id: String,
    pub node_id: String,
    /// Pipeline stage name (e.g. "speculate", "verify", "converge", "commit").
    pub step: String,
    /// Outcome of the step (e.g. "ok", "retry", "escalated", "failed").
    pub outcome: String,
    /// JSON-serialized EnergyComponents snapshot (if available).
    pub energy_json: Option<String>,
    /// ParseResultState as string (if this step involved parsing).
    pub parse_state: Option<String>,
    /// RetryClassification as string (if this step triggered a retry).
    pub retry_classification: Option<String>,
    /// Attempt count at the time of recording.
    pub attempt_count: i32,
    /// Wall-clock duration of the step in milliseconds.
    pub duration_ms: i32,
}

/// PSP-7: Record for a single correction attempt within a convergence loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionAttemptRow {
    pub session_id: String,
    pub node_id: String,
    pub attempt: i32,
    pub parse_state: String,
    pub retry_classification: Option<String>,
    pub response_fingerprint: String,
    pub response_length: i32,
    /// JSON-serialized EnergyComponents snapshot (if available).
    pub energy_json: Option<String>,
    pub accepted: bool,
    pub rejection_reason: Option<String>,
    /// Epoch seconds.
    pub created_at: i64,
}
