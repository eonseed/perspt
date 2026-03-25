//! DuckDB Merkle Ledger
//!
//! Persistent storage for session history, commits, and Merkle proofs.

use anyhow::{Context, Result};
pub use perspt_store::{LlmRequestRecord, NodeStateRecord, SessionRecord, SessionStore};
use std::path::{Path, PathBuf};

/// Full commit payload collected by the orchestrator at commit time.
///
/// Bundles graph-structural fields, retry/error metadata, and merkle
/// material so that `commit_node_snapshot()` can persist a complete
/// node record in a single call.
#[derive(Debug, Clone)]
pub struct NodeCommitPayload {
    pub node_id: String,
    pub state: String,
    pub v_total: f32,
    pub merkle_hash: Option<Vec<u8>>,
    pub attempt_count: i32,
    pub node_class: Option<String>,
    pub owner_plugin: Option<String>,
    pub goal: Option<String>,
    pub parent_id: Option<String>,
    /// JSON-serialized `Vec<String>` of child node IDs
    pub children: Option<String>,
    pub last_error_type: Option<String>,
}

/// Merkle commit record (Legacy wrapper for compatibility)
#[derive(Debug, Clone)]
pub struct MerkleCommit {
    pub commit_id: String,
    pub session_id: String,
    pub node_id: String,
    pub merkle_root: [u8; 32],
    pub parent_hash: Option<[u8; 32]>,
    pub timestamp: i64,
    pub energy: f32,
    pub stable: bool,
}

/// Session record (Legacy wrapper for compatibility)
#[derive(Debug, Clone)]
pub struct SessionRecordLegacy {
    pub session_id: String,
    pub task: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub status: String,
    pub total_nodes: usize,
    pub completed_nodes: usize,
}

/// Merkle Ledger using DuckDB for persistence
pub struct MerkleLedger {
    /// Session store from perspt-store
    store: SessionStore,
    /// Current session metadata (legacy cache)
    pub(crate) current_session: Option<SessionRecordLegacy>,
    /// Session artifact directory
    session_dir: Option<PathBuf>,
}

impl MerkleLedger {
    /// Create a new ledger (opens or creates database)
    pub fn new() -> Result<Self> {
        let store = SessionStore::new().context("Failed to initialize session store")?;
        Ok(Self {
            store,
            current_session: None,
            session_dir: None,
        })
    }

    /// Create an in-memory ledger (for testing)
    pub fn in_memory() -> Result<Self> {
        // Use a unique temp db for testing to avoid collisions
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join(format!("perspt_test_{}.db", uuid::Uuid::new_v4()));
        let store = SessionStore::open(&db_path)?;
        Ok(Self {
            store,
            current_session: None,
            session_dir: None,
        })
    }

    /// Start a new session
    pub fn start_session(&mut self, session_id: &str, task: &str, working_dir: &str) -> Result<()> {
        let record = SessionRecord {
            session_id: session_id.to_string(),
            task: task.to_string(),
            working_dir: working_dir.to_string(),
            merkle_root: None,
            detected_toolchain: None,
            status: "RUNNING".to_string(),
        };

        self.store.create_session(&record)?;

        // Create physical artifact directory
        let dir = self.store.create_session_dir(session_id)?;
        self.session_dir = Some(dir);

        let legacy_record = SessionRecordLegacy {
            session_id: session_id.to_string(),
            task: task.to_string(),
            started_at: chrono_timestamp(),
            ended_at: None,
            status: "RUNNING".to_string(),
            total_nodes: 0,
            completed_nodes: 0,
        };
        self.current_session = Some(legacy_record);

        log::info!("Started persistent session: {}", session_id);
        Ok(())
    }

    /// Record energy measurement
    pub fn record_energy(
        &self,
        node_id: &str,
        energy: &crate::types::EnergyComponents,
        total_energy: f32,
    ) -> Result<()> {
        let session_id = self
            .current_session
            .as_ref()
            .map(|s| s.session_id.clone())
            .context("No active session to record energy")?;

        let record = perspt_store::EnergyRecord {
            node_id: node_id.to_string(),
            session_id,
            v_syn: energy.v_syn,
            v_str: energy.v_str,
            v_log: energy.v_log,
            v_boot: energy.v_boot,
            v_sheaf: energy.v_sheaf,
            v_total: total_energy,
        };

        self.store.record_energy(&record)?;
        Ok(())
    }

    /// Commit a stable node state
    pub fn commit_node(
        &mut self,
        node_id: &str,
        merkle_root: [u8; 32],
        _parent_hash: Option<[u8; 32]>,
        energy: f32,
        state_json: String,
    ) -> Result<String> {
        let session_id = self
            .current_session
            .as_ref()
            .map(|s| s.session_id.clone())
            .context("No active session to commit")?;

        let commit_id = generate_commit_id();

        let record = NodeStateRecord {
            node_id: node_id.to_string(),
            session_id: session_id.clone(),
            state: state_json,
            v_total: energy,
            merkle_hash: Some(merkle_root.to_vec()),
            attempt_count: 1, // Placeholder
            // Phase 8 fields — populated properly via commit_node_snapshot
            node_class: None,
            owner_plugin: None,
            goal: None,
            parent_id: None,
            children: None,
            last_error_type: None,
            committed_at: None,
        };

        self.store.record_node_state(&record)?;
        self.store.update_merkle_root(&session_id, &merkle_root)?;

        log::info!("Committed node {} to store", node_id);

        // Update session progress
        if let Some(ref mut session) = self.current_session {
            session.completed_nodes += 1;
        }

        Ok(commit_id)
    }

    /// Commit a full node snapshot with all Phase 8 metadata.
    ///
    /// This is the preferred commit API for the orchestrator. It records the
    /// complete node state, graph-structural fields, retry/error metadata,
    /// and merkle material in a single durable write. Returns the commit ID.
    pub fn commit_node_snapshot(&mut self, payload: &NodeCommitPayload) -> Result<String> {
        let session_id = self
            .current_session
            .as_ref()
            .map(|s| s.session_id.clone())
            .context("No active session to commit")?;

        let commit_id = generate_commit_id();

        let record = NodeStateRecord {
            node_id: payload.node_id.clone(),
            session_id: session_id.clone(),
            state: payload.state.clone(),
            v_total: payload.v_total,
            merkle_hash: payload.merkle_hash.clone(),
            attempt_count: payload.attempt_count,
            node_class: payload.node_class.clone(),
            owner_plugin: payload.owner_plugin.clone(),
            goal: payload.goal.clone(),
            parent_id: payload.parent_id.clone(),
            children: payload.children.clone(),
            last_error_type: payload.last_error_type.clone(),
            committed_at: Some(chrono_iso_now()),
        };

        self.store.record_node_state(&record)?;

        // Update merkle root if hash is present
        if let Some(ref hash) = payload.merkle_hash {
            if hash.len() == 32 {
                let mut root = [0u8; 32];
                root.copy_from_slice(hash);
                self.store.update_merkle_root(&session_id, &root)?;
            }
        }

        log::info!(
            "Committed node snapshot '{}' (state={}, attempts={})",
            payload.node_id,
            payload.state,
            payload.attempt_count
        );

        if let Some(ref mut session) = self.current_session {
            session.completed_nodes += 1;
        }

        Ok(commit_id)
    }

    /// End the current session
    pub fn end_session(&mut self, status: &str) -> Result<()> {
        if let Some(ref mut session) = self.current_session {
            session.ended_at = Some(chrono_timestamp());
            session.status = status.to_string();
            log::info!(
                "Ended session {} with status: {}",
                session.session_id,
                status
            );
        }
        Ok(())
    }

    /// Get artifacts directory
    pub fn artifacts_dir(&self) -> Option<&Path> {
        self.session_dir.as_deref()
    }

    /// Get session statistics (legacy facade)
    pub fn get_stats(&self) -> LedgerStats {
        LedgerStats {
            total_sessions: 0, // Would query store.count_sessions()
            total_commits: 0,
            db_size_bytes: 0,
        }
    }

    /// Get the current merkle root (legacy facade)
    pub fn current_merkle_root(&self) -> [u8; 32] {
        [0u8; 32] // Placeholder
    }

    /// Record an LLM request/response for debugging and cost tracking
    pub fn record_llm_request(
        &self,
        model: &str,
        prompt: &str,
        response: &str,
        node_id: Option<&str>,
        latency_ms: i32,
    ) -> Result<()> {
        let session_id = self
            .current_session
            .as_ref()
            .map(|s| s.session_id.clone())
            .context("No active session to record LLM request")?;

        let record = LlmRequestRecord {
            session_id,
            node_id: node_id.map(|s| s.to_string()),
            model: model.to_string(),
            prompt: prompt.to_string(),
            response: response.to_string(),
            tokens_in: 0, // TODO: Extract from provider response if available
            tokens_out: 0,
            latency_ms,
        };

        self.store.record_llm_request(&record)?;
        log::debug!(
            "Recorded LLM request: model={}, prompt_len={}, response_len={}",
            model,
            prompt.len(),
            response.len()
        );
        Ok(())
    }

    /// Get access to the underlying store (for direct queries)
    pub fn store(&self) -> &SessionStore {
        &self.store
    }

    // =========================================================================
    // PSP-5 Phase 3: Structural Digests & Context Provenance
    // =========================================================================

    /// Record a structural digest for a node
    pub fn record_structural_digest(
        &self,
        node_id: &str,
        source_path: &str,
        artifact_kind: &str,
        hash: &[u8],
        version: i32,
    ) -> Result<()> {
        let session_id = self
            .current_session
            .as_ref()
            .map(|s| s.session_id.clone())
            .context("No active session to record structural digest")?;

        let record = perspt_store::StructuralDigestRecord {
            digest_id: format!("sd-{}-{}", node_id, uuid::Uuid::new_v4()),
            session_id,
            node_id: node_id.to_string(),
            source_path: source_path.to_string(),
            artifact_kind: artifact_kind.to_string(),
            hash: hash.to_vec(),
            version,
        };

        self.store.record_structural_digest(&record)?;
        log::debug!(
            "Recorded structural digest for {} at {}",
            node_id,
            source_path
        );
        Ok(())
    }

    /// Get structural digests for a specific node in the current session
    pub fn get_structural_digests(
        &self,
        node_id: &str,
    ) -> Result<Vec<perspt_store::StructuralDigestRecord>> {
        let session_id = self
            .current_session
            .as_ref()
            .map(|s| s.session_id.clone())
            .context("No active session to query structural digests")?;

        self.store.get_structural_digests(&session_id, node_id)
    }

    /// Record context provenance for a node
    pub fn record_context_provenance(
        &self,
        provenance: &perspt_core::types::ContextProvenance,
    ) -> Result<()> {
        let session_id = self
            .current_session
            .as_ref()
            .map(|s| s.session_id.clone())
            .context("No active session to record context provenance")?;

        let to_hex_32 =
            |bytes: &[u8; 32]| -> String { bytes.iter().map(|b| format!("{:02x}", b)).collect() };
        let to_hex_vec =
            |bytes: &[u8]| -> String { bytes.iter().map(|b| format!("{:02x}", b)).collect() };
        let structural_hashes: Vec<String> = provenance
            .structural_digest_hashes
            .iter()
            .map(|(id, hash)| format!("{}:{}", id, to_hex_32(hash)))
            .collect();
        let summary_hashes: Vec<String> = provenance
            .summary_digest_hashes
            .iter()
            .map(|(id, hash)| format!("{}:{}", id, to_hex_32(hash)))
            .collect();
        let dep_hashes: Vec<String> = provenance
            .dependency_commit_hashes
            .iter()
            .map(|(id, hash)| format!("{}:{}", id, to_hex_vec(hash)))
            .collect();

        let record = perspt_store::ContextProvenanceRecord {
            session_id,
            node_id: provenance.node_id.clone(),
            context_package_id: provenance.context_package_id.clone(),
            structural_hashes: serde_json::to_string(&structural_hashes).unwrap_or_default(),
            summary_hashes: serde_json::to_string(&summary_hashes).unwrap_or_default(),
            dependency_hashes: serde_json::to_string(&dep_hashes).unwrap_or_default(),
            included_file_count: provenance.included_file_count as i32,
            total_bytes: provenance.total_bytes as i32,
        };

        self.store.record_context_provenance(&record)?;
        log::debug!(
            "Recorded context provenance for node '{}' (package '{}')",
            provenance.node_id,
            provenance.context_package_id
        );
        Ok(())
    }

    /// Get context provenance for a specific node in the current session
    pub fn get_context_provenance(
        &self,
        node_id: &str,
    ) -> Result<Option<perspt_store::ContextProvenanceRecord>> {
        let session_id = self
            .current_session
            .as_ref()
            .map(|s| s.session_id.clone())
            .context("No active session to query context provenance")?;

        self.store.get_context_provenance(&session_id, node_id)
    }

    // =========================================================================
    // PSP-5 Phase 5: Escalation and Rewrite Persistence
    // =========================================================================

    /// Record an escalation report for a non-convergent node
    pub fn record_escalation_report(
        &self,
        report: &perspt_core::types::EscalationReport,
    ) -> Result<()> {
        let session_id = self
            .current_session
            .as_ref()
            .map(|s| s.session_id.clone())
            .context("No active session to record escalation report")?;

        let record = perspt_store::EscalationReportRecord {
            session_id,
            node_id: report.node_id.clone(),
            category: report.category.to_string(),
            action: serde_json::to_string(&report.action).unwrap_or_default(),
            energy_snapshot: serde_json::to_string(&report.energy_snapshot).unwrap_or_default(),
            stage_outcomes: serde_json::to_string(&report.stage_outcomes).unwrap_or_default(),
            evidence: report.evidence.clone(),
            affected_node_ids: serde_json::to_string(&report.affected_node_ids).unwrap_or_default(),
        };

        self.store.record_escalation_report(&record)?;
        log::debug!(
            "Recorded escalation report for node '{}': {} → {}",
            report.node_id,
            report.category,
            report.action
        );
        Ok(())
    }

    /// Record a local graph rewrite
    pub fn record_rewrite(&self, record: &perspt_core::types::RewriteRecord) -> Result<()> {
        let session_id = self
            .current_session
            .as_ref()
            .map(|s| s.session_id.clone())
            .context("No active session to record rewrite")?;

        let row = perspt_store::RewriteRecordRow {
            session_id,
            node_id: record.node_id.clone(),
            action: serde_json::to_string(&record.action).unwrap_or_default(),
            category: record.category.to_string(),
            requeued_nodes: serde_json::to_string(&record.requeued_nodes).unwrap_or_default(),
            inserted_nodes: serde_json::to_string(&record.inserted_nodes).unwrap_or_default(),
        };

        self.store.record_rewrite(&row)?;
        log::debug!(
            "Recorded rewrite for node '{}': {} ({} requeued, {} inserted)",
            record.node_id,
            record.action,
            record.requeued_nodes.len(),
            record.inserted_nodes.len()
        );
        Ok(())
    }

    /// Record a sheaf validation result
    pub fn record_sheaf_validation(
        &self,
        node_id: &str,
        result: &perspt_core::types::SheafValidationResult,
    ) -> Result<()> {
        let session_id = self
            .current_session
            .as_ref()
            .map(|s| s.session_id.clone())
            .context("No active session to record sheaf validation")?;

        let row = perspt_store::SheafValidationRow {
            session_id,
            node_id: node_id.to_string(),
            validator_class: result.validator_class.to_string(),
            plugin_source: result.plugin_source.clone(),
            passed: result.passed,
            evidence_summary: result.evidence_summary.clone(),
            affected_files: serde_json::to_string(&result.affected_files).unwrap_or_default(),
            v_sheaf_contribution: result.v_sheaf_contribution,
            requeue_targets: serde_json::to_string(&result.requeue_targets).unwrap_or_default(),
        };

        self.store.record_sheaf_validation(&row)?;
        log::debug!(
            "Recorded sheaf validation for node '{}': {} → {}",
            node_id,
            result.validator_class,
            if result.passed { "pass" } else { "fail" }
        );
        Ok(())
    }

    /// Get escalation reports for the current session
    pub fn get_escalation_reports(&self) -> Result<Vec<perspt_store::EscalationReportRecord>> {
        let session_id = self
            .current_session
            .as_ref()
            .map(|s| s.session_id.clone())
            .context("No active session to query escalation reports")?;

        self.store.get_escalation_reports(&session_id)
    }

    // =========================================================================
    // PSP-5 Phase 8: Verification Result and Artifact Bundle Facades
    // =========================================================================

    /// Record a verification result snapshot for a node
    pub fn record_verification_result(
        &self,
        node_id: &str,
        result: &perspt_core::types::VerificationResult,
    ) -> Result<()> {
        let session_id = self.session_id()?;

        let result_json = serde_json::to_string(result).unwrap_or_default();
        let row = perspt_store::VerificationResultRow {
            session_id,
            node_id: node_id.to_string(),
            result_json,
            syntax_ok: result.syntax_ok,
            build_ok: result.build_ok,
            tests_ok: result.tests_ok,
            lint_ok: result.lint_ok,
            diagnostics_count: result.diagnostics_count as i32,
            tests_passed: result.tests_passed as i32,
            tests_failed: result.tests_failed as i32,
            degraded: result.degraded,
            degraded_reason: result.degraded_reason.clone(),
        };

        self.store.record_verification_result(&row)?;
        log::debug!(
            "Recorded verification result for node '{}': syn={} build={} test={} degraded={}",
            node_id,
            result.syntax_ok,
            result.build_ok,
            result.tests_ok,
            result.degraded
        );
        Ok(())
    }

    /// Get the latest verification result for a node
    pub fn get_verification_result(
        &self,
        node_id: &str,
    ) -> Result<Option<perspt_store::VerificationResultRow>> {
        let session_id = self.session_id()?;
        self.store.get_verification_result(&session_id, node_id)
    }

    /// Record an artifact bundle snapshot for a node
    pub fn record_artifact_bundle(
        &self,
        node_id: &str,
        bundle: &perspt_core::types::ArtifactBundle,
    ) -> Result<()> {
        let session_id = self.session_id()?;

        let bundle_json = serde_json::to_string(bundle).unwrap_or_default();
        let touched_files: Vec<String> = bundle
            .artifacts
            .iter()
            .map(|a| a.path().to_string())
            .collect();

        let row = perspt_store::ArtifactBundleRow {
            session_id,
            node_id: node_id.to_string(),
            bundle_json,
            artifact_count: bundle.artifacts.len() as i32,
            command_count: bundle.commands.len() as i32,
            touched_files: serde_json::to_string(&touched_files).unwrap_or_default(),
        };

        self.store.record_artifact_bundle(&row)?;
        log::debug!(
            "Recorded artifact bundle for node '{}': {} artifacts, {} commands",
            node_id,
            bundle.artifacts.len(),
            bundle.commands.len()
        );
        Ok(())
    }

    /// Get the latest artifact bundle for a node
    pub fn get_artifact_bundle(
        &self,
        node_id: &str,
    ) -> Result<Option<perspt_store::ArtifactBundleRow>> {
        let session_id = self.session_id()?;
        self.store.get_artifact_bundle(&session_id, node_id)
    }

    // =========================================================================
    // PSP-5 Phase 8: Task Graph & Session Rehydration
    // =========================================================================

    /// Record a task-graph edge (parent→child dependency)
    pub fn record_task_graph_edge(
        &self,
        parent_node_id: &str,
        child_node_id: &str,
        edge_type: &str,
    ) -> Result<()> {
        let session_id = self.session_id()?;
        let row = perspt_store::TaskGraphEdgeRow {
            session_id,
            parent_node_id: parent_node_id.to_string(),
            child_node_id: child_node_id.to_string(),
            edge_type: edge_type.to_string(),
        };
        self.store.record_task_graph_edge(&row)?;
        log::debug!(
            "Recorded task graph edge: {} → {} ({})",
            parent_node_id,
            child_node_id,
            edge_type
        );
        Ok(())
    }

    /// Get all task graph edges for the current session
    pub fn get_task_graph_edges(&self) -> Result<Vec<perspt_store::TaskGraphEdgeRow>> {
        let session_id = self.session_id()?;
        self.store.get_task_graph_edges(&session_id)
    }

    /// Get sheaf validations for a specific node
    pub fn get_sheaf_validations(
        &self,
        node_id: &str,
    ) -> Result<Vec<perspt_store::SheafValidationRow>> {
        let session_id = self.session_id()?;
        self.store.get_sheaf_validations(&session_id, node_id)
    }

    /// Load a complete session snapshot for rehydration/resume.
    ///
    /// Aggregates the latest node states, graph topology, energy history,
    /// verification results, artifact bundles, sheaf validations,
    /// provisional branches, interface seals, context provenance, and
    /// escalation reports into a single `SessionSnapshot`.
    pub fn load_session_snapshot(&self) -> Result<SessionSnapshot> {
        let session_id = self.session_id()?;

        let node_states = self
            .store
            .get_latest_node_states(&session_id)
            .unwrap_or_default();

        let graph_edges = self
            .store
            .get_task_graph_edges(&session_id)
            .unwrap_or_default();

        let branches = self
            .store
            .get_provisional_branches(&session_id)
            .unwrap_or_default();

        let escalation_reports = self
            .store
            .get_escalation_reports(&session_id)
            .unwrap_or_default();

        let flushes = self
            .store
            .get_branch_flushes(&session_id)
            .unwrap_or_default();

        // Collect per-node evidence
        let mut node_details: Vec<NodeSnapshotDetail> = Vec::with_capacity(node_states.len());
        for ns in &node_states {
            let nid = &ns.node_id;

            let energy_history = self
                .store
                .get_energy_history(&session_id, nid)
                .unwrap_or_default();

            let verification = self
                .store
                .get_verification_result(&session_id, nid)
                .ok()
                .flatten();

            let artifact_bundle = self
                .store
                .get_artifact_bundle(&session_id, nid)
                .ok()
                .flatten();

            let sheaf_validations = self
                .store
                .get_sheaf_validations(&session_id, nid)
                .unwrap_or_default();

            let interface_seals = self
                .store
                .get_interface_seals(&session_id, nid)
                .unwrap_or_default();

            let context_provenance = self
                .store
                .get_context_provenance(&session_id, nid)
                .ok()
                .flatten();

            node_details.push(NodeSnapshotDetail {
                record: ns.clone(),
                energy_history,
                verification,
                artifact_bundle,
                sheaf_validations,
                interface_seals,
                context_provenance,
            });
        }

        log::info!(
            "Loaded session snapshot: {} nodes, {} edges, {} branches",
            node_details.len(),
            graph_edges.len(),
            branches.len()
        );

        Ok(SessionSnapshot {
            session_id,
            node_details,
            graph_edges,
            branches,
            escalation_reports,
            flushes,
        })
    }

    // =========================================================================
    // PSP-5 Phase 6: Provisional Branch, Interface Seal, Branch Flush Facades
    // =========================================================================

    /// Get the current session ID (helper for Phase 6 methods)
    fn session_id(&self) -> Result<String> {
        self.current_session
            .as_ref()
            .map(|s| s.session_id.clone())
            .context("No active session")
    }

    /// Record a new provisional branch for speculative child work
    pub fn record_provisional_branch(
        &self,
        branch: &perspt_core::types::ProvisionalBranch,
    ) -> Result<()> {
        let row = perspt_store::ProvisionalBranchRow {
            branch_id: branch.branch_id.clone(),
            session_id: branch.session_id.clone(),
            node_id: branch.node_id.clone(),
            parent_node_id: branch.parent_node_id.clone(),
            state: branch.state.to_string(),
            parent_seal_hash: branch.parent_seal_hash.map(|h| h.to_vec()),
            sandbox_dir: branch.sandbox_dir.clone(),
        };

        self.store.record_provisional_branch(&row)?;
        log::debug!(
            "Recorded provisional branch '{}' for node '{}' (parent: '{}')",
            branch.branch_id,
            branch.node_id,
            branch.parent_node_id
        );
        Ok(())
    }

    /// Update a provisional branch state
    pub fn update_branch_state(&self, branch_id: &str, new_state: &str) -> Result<()> {
        self.store.update_branch_state(branch_id, new_state)?;
        log::debug!("Updated branch '{}' state to '{}'", branch_id, new_state);
        Ok(())
    }

    /// Get all provisional branches for the current session
    pub fn get_provisional_branches(&self) -> Result<Vec<perspt_store::ProvisionalBranchRow>> {
        let session_id = self.session_id()?;
        self.store.get_provisional_branches(&session_id)
    }

    /// Get live (active/sealed) branches depending on a parent node
    pub fn get_live_branches_for_parent(
        &self,
        parent_node_id: &str,
    ) -> Result<Vec<perspt_store::ProvisionalBranchRow>> {
        let session_id = self.session_id()?;
        self.store
            .get_live_branches_for_parent(&session_id, parent_node_id)
    }

    /// Flush all live branches for a parent node and return flushed branch IDs
    pub fn flush_branches_for_parent(&self, parent_node_id: &str) -> Result<Vec<String>> {
        let session_id = self.session_id()?;
        self.store
            .flush_branches_for_parent(&session_id, parent_node_id)
    }

    /// Record a branch lineage edge (parent branch → child branch)
    pub fn record_branch_lineage(&self, lineage: &perspt_core::types::BranchLineage) -> Result<()> {
        let row = perspt_store::BranchLineageRow {
            lineage_id: lineage.lineage_id.clone(),
            parent_branch_id: lineage.parent_branch_id.clone(),
            child_branch_id: lineage.child_branch_id.clone(),
            depends_on_seal: lineage.depends_on_seal,
        };

        self.store.record_branch_lineage(&row)?;
        log::debug!(
            "Recorded branch lineage: {} → {}",
            lineage.parent_branch_id,
            lineage.child_branch_id
        );
        Ok(())
    }

    /// Get child branch IDs for a parent branch
    pub fn get_child_branches(&self, parent_branch_id: &str) -> Result<Vec<String>> {
        self.store.get_child_branches(parent_branch_id)
    }

    /// Record an interface seal for a node
    pub fn record_interface_seal(
        &self,
        seal: &perspt_core::types::InterfaceSealRecord,
    ) -> Result<()> {
        let row = perspt_store::InterfaceSealRow {
            seal_id: seal.seal_id.clone(),
            session_id: seal.session_id.clone(),
            node_id: seal.node_id.clone(),
            sealed_path: seal.sealed_path.clone(),
            artifact_kind: seal.artifact_kind.to_string(),
            seal_hash: seal.seal_hash.to_vec(),
            version: seal.version as i32,
        };

        self.store.record_interface_seal(&row)?;
        log::debug!(
            "Recorded interface seal '{}' for node '{}' at '{}'",
            seal.seal_id,
            seal.node_id,
            seal.sealed_path
        );
        Ok(())
    }

    /// Get all interface seals for a node in the current session
    pub fn get_interface_seals(
        &self,
        node_id: &str,
    ) -> Result<Vec<perspt_store::InterfaceSealRow>> {
        let session_id = self.session_id()?;
        self.store.get_interface_seals(&session_id, node_id)
    }

    /// Check whether a node has any interface seals
    pub fn has_interface_seals(&self, node_id: &str) -> Result<bool> {
        let session_id = self.session_id()?;
        self.store.has_interface_seals(&session_id, node_id)
    }

    /// Record a branch flush decision
    pub fn record_branch_flush(&self, flush: &perspt_core::types::BranchFlushRecord) -> Result<()> {
        let row = perspt_store::BranchFlushRow {
            flush_id: flush.flush_id.clone(),
            session_id: flush.session_id.clone(),
            parent_node_id: flush.parent_node_id.clone(),
            flushed_branch_ids: serde_json::to_string(&flush.flushed_branch_ids)
                .unwrap_or_default(),
            requeue_node_ids: serde_json::to_string(&flush.requeue_node_ids).unwrap_or_default(),
            reason: flush.reason.clone(),
        };

        self.store.record_branch_flush(&row)?;
        log::debug!(
            "Recorded branch flush for parent '{}': {} branches flushed",
            flush.parent_node_id,
            flush.flushed_branch_ids.len()
        );
        Ok(())
    }

    /// Get all branch flush records for the current session
    pub fn get_branch_flushes(&self) -> Result<Vec<perspt_store::BranchFlushRow>> {
        let session_id = self.session_id()?;
        self.store.get_branch_flushes(&session_id)
    }

    // =========================================================================
    // PSP-5 Phase 7: Shared Review & Provenance Aggregation Helpers
    // =========================================================================

    /// Build a review-ready summary for a single node.
    ///
    /// Aggregates energy history, escalation reports, sheaf validations,
    /// context provenance, interface seals, and branch state from the store
    /// into a single struct consumable by both TUI and CLI surfaces.
    pub fn node_review_summary(&self, node_id: &str) -> Result<NodeReviewSummary> {
        let session_id = self.session_id()?;

        let energy_history = self
            .store
            .get_energy_history(&session_id, node_id)
            .unwrap_or_default();

        let latest_energy = energy_history.last().cloned();

        let escalation_reports = self
            .store
            .get_escalation_reports(&session_id)
            .unwrap_or_default()
            .into_iter()
            .filter(|r| r.node_id == node_id)
            .collect::<Vec<_>>();

        let sheaf_validations = self
            .store
            .get_sheaf_validations(&session_id, node_id)
            .unwrap_or_default();

        let interface_seals = self
            .store
            .get_interface_seals(&session_id, node_id)
            .unwrap_or_default();

        let context_provenance = self
            .store
            .get_context_provenance(&session_id, node_id)
            .ok()
            .flatten()
            .into_iter()
            .collect::<Vec<_>>();

        let branches: Vec<_> = self
            .store
            .get_provisional_branches(&session_id)
            .unwrap_or_default()
            .into_iter()
            .filter(|b| b.node_id == node_id)
            .collect();

        let attempt_count = energy_history.len().max(1) as u32;

        Ok(NodeReviewSummary {
            node_id: node_id.to_string(),
            latest_energy,
            energy_history,
            attempt_count,
            escalation_reports,
            sheaf_validations,
            interface_seals,
            context_provenance,
            branches,
        })
    }

    /// Build a session-level summary aggregating lifecycle counts, energy
    /// stats, escalation activity, and branch provenance.
    pub fn session_summary(&self) -> Result<SessionReviewSummary> {
        let session_id = self.session_id()?;

        let node_states = self.store.get_node_states(&session_id).unwrap_or_default();
        let total_nodes = node_states.len();
        let completed = node_states
            .iter()
            .filter(|n| n.state == "COMPLETED" || n.state == "STABLE")
            .count();
        let failed = node_states.iter().filter(|n| n.state == "FAILED").count();
        let escalated = node_states
            .iter()
            .filter(|n| n.state == "Escalated")
            .count();

        // Collect latest energy per node
        let mut total_energy: f32 = 0.0;
        let mut node_energies: Vec<(String, perspt_store::EnergyRecord)> = Vec::new();
        for ns in &node_states {
            if let Ok(history) = self.store.get_energy_history(&session_id, &ns.node_id) {
                if let Some(latest) = history.last() {
                    total_energy += latest.v_total;
                    node_energies.push((ns.node_id.clone(), latest.clone()));
                }
            }
        }

        let escalation_reports = self
            .store
            .get_escalation_reports(&session_id)
            .unwrap_or_default();

        let branches = self
            .store
            .get_provisional_branches(&session_id)
            .unwrap_or_default();

        let active_branches = branches.iter().filter(|b| b.state == "active").count();
        let sealed_branches = branches.iter().filter(|b| b.state == "sealed").count();
        let merged_branches = branches.iter().filter(|b| b.state == "merged").count();
        let flushed_branches = branches.iter().filter(|b| b.state == "flushed").count();

        let flushes = self
            .store
            .get_branch_flushes(&session_id)
            .unwrap_or_default();

        Ok(SessionReviewSummary {
            session_id,
            total_nodes,
            completed,
            failed,
            escalated,
            total_energy,
            node_energies,
            escalation_reports,
            branches_total: branches.len(),
            active_branches,
            sealed_branches,
            merged_branches,
            flushed_branches,
            flush_decisions: flushes,
        })
    }
}

/// PSP-5 Phase 7: Aggregated review summary for a single node.
///
/// Consumed by both TUI review modal and CLI status/resume commands.
#[derive(Debug, Clone)]
pub struct NodeReviewSummary {
    pub node_id: String,
    pub latest_energy: Option<perspt_store::EnergyRecord>,
    pub energy_history: Vec<perspt_store::EnergyRecord>,
    pub attempt_count: u32,
    pub escalation_reports: Vec<perspt_store::EscalationReportRecord>,
    pub sheaf_validations: Vec<perspt_store::SheafValidationRow>,
    pub interface_seals: Vec<perspt_store::InterfaceSealRow>,
    pub context_provenance: Vec<perspt_store::ContextProvenanceRecord>,
    pub branches: Vec<perspt_store::ProvisionalBranchRow>,
}

/// PSP-5 Phase 7: Aggregated session-level review summary.
///
/// Consumed by both TUI dashboard and CLI status/resume commands.
#[derive(Debug, Clone)]
pub struct SessionReviewSummary {
    pub session_id: String,
    pub total_nodes: usize,
    pub completed: usize,
    pub failed: usize,
    pub escalated: usize,
    pub total_energy: f32,
    pub node_energies: Vec<(String, perspt_store::EnergyRecord)>,
    pub escalation_reports: Vec<perspt_store::EscalationReportRecord>,
    pub branches_total: usize,
    pub active_branches: usize,
    pub sealed_branches: usize,
    pub merged_branches: usize,
    pub flushed_branches: usize,
    pub flush_decisions: Vec<perspt_store::BranchFlushRow>,
}

/// Ledger statistics (Legacy)
#[derive(Debug, Clone)]
pub struct LedgerStats {
    pub total_sessions: usize,
    pub total_commits: usize,
    pub db_size_bytes: u64,
}

/// PSP-5 Phase 8: Per-node evidence bundle for session rehydration.
#[derive(Debug, Clone)]
pub struct NodeSnapshotDetail {
    pub record: NodeStateRecord,
    pub energy_history: Vec<perspt_store::EnergyRecord>,
    pub verification: Option<perspt_store::VerificationResultRow>,
    pub artifact_bundle: Option<perspt_store::ArtifactBundleRow>,
    pub sheaf_validations: Vec<perspt_store::SheafValidationRow>,
    pub interface_seals: Vec<perspt_store::InterfaceSealRow>,
    pub context_provenance: Option<perspt_store::ContextProvenanceRecord>,
}

/// PSP-5 Phase 8: Complete session snapshot for resume/rehydration.
///
/// Aggregates all persisted state needed to reconstruct the orchestrator
/// DAG, restore node states, and resume execution from the last durable
/// boundary.
#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    pub session_id: String,
    pub node_details: Vec<NodeSnapshotDetail>,
    pub graph_edges: Vec<perspt_store::TaskGraphEdgeRow>,
    pub branches: Vec<perspt_store::ProvisionalBranchRow>,
    pub escalation_reports: Vec<perspt_store::EscalationReportRecord>,
    pub flushes: Vec<perspt_store::BranchFlushRow>,
}

/// Generate a unique commit ID
fn generate_commit_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:x}", now)
}

/// Get current timestamp
fn chrono_timestamp() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// ISO-8601 timestamp for committed_at fields
fn chrono_iso_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // Simple UTC timestamp — YYYY-MM-DDTHH:MM:SSZ
    let days = secs / 86400;
    let time = secs % 86400;
    let h = time / 3600;
    let m = (time % 3600) / 60;
    let s = time % 60;
    // Days since 1970-01-01 to y/m/d (civil calendar)
    let (y, mo, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, m, s)
}

/// Convert days since Unix epoch to (year, month, day)
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Algorithm from Howard Hinnant's date library
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
