use super::*;

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
    pub(crate) store: SessionStore,
    /// Current session metadata (legacy cache)
    pub(crate) current_session: Option<SessionRecordLegacy>,
    /// Session artifact directory
    pub(crate) session_dir: Option<PathBuf>,
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
            // Persist status to durable store
            self.store
                .update_session_status(&session.session_id, status)?;
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
    #[allow(clippy::too_many_arguments)]
    pub fn record_llm_request(
        &self,
        model: &str,
        prompt: &str,
        response: &str,
        node_id: Option<&str>,
        latency_ms: i32,
        tokens_in: i32,
        tokens_out: i32,
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
            tokens_in,
            tokens_out,
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

    /// Record lightweight LLM usage metrics (no prompt/response text).
    ///
    /// This is always called after every LLM invocation regardless of
    /// the `--log-llm` flag so that token and cost accounting is never lost.
    pub fn record_llm_usage(
        &self,
        model: &str,
        node_id: Option<&str>,
        latency_ms: i32,
        tokens_in: i32,
        tokens_out: i32,
    ) -> Result<()> {
        let session_id = self
            .current_session
            .as_ref()
            .map(|s| s.session_id.clone())
            .context("No active session to record LLM usage")?;

        // Re-use existing table with empty prompt/response to avoid schema changes.
        let record = LlmRequestRecord {
            session_id,
            node_id: node_id.map(|s| s.to_string()),
            model: model.to_string(),
            prompt: String::new(),
            response: String::new(),
            tokens_in,
            tokens_out,
            latency_ms,
        };

        self.store.record_llm_request(&record)?;
        log::debug!(
            "Recorded LLM usage: model={}, tokens_in={}, tokens_out={}, latency={}ms",
            model,
            tokens_in,
            tokens_out,
            latency_ms,
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

    /// PSP-5 Phase 5: Count rewrite records matching a lineage prefix.
    ///
    /// A lineage is identified by the base node ID (before any `__split_` or
    /// `__iface` suffixes). This count is used as a churn guardrail to prevent
    /// infinite rewrite loops.
    pub fn get_rewrite_count_for_lineage(&self, lineage_base: &str) -> Result<usize> {
        let session_id = self
            .current_session
            .as_ref()
            .map(|s| s.session_id.clone())
            .context("No active session to query rewrite count")?;

        let records = self.store.get_rewrite_records(&session_id)?;
        let count = records
            .iter()
            .filter(|r| r.node_id.starts_with(lineage_base))
            .count();
        Ok(count)
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
        let node_details: Vec<NodeSnapshotDetail> = node_states
            .iter()
            .map(|ns| self.node_snapshot_detail(&session_id, ns))
            .collect();

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

    /// Gather every recorded evidence stream for one node.
    fn node_snapshot_detail(
        &self,
        session_id: &str,
        ns: &perspt_store::NodeStateRecord,
    ) -> NodeSnapshotDetail {
        let nid = &ns.node_id;
        NodeSnapshotDetail {
            record: ns.clone(),
            energy_history: self
                .store
                .get_energy_history(session_id, nid)
                .unwrap_or_default(),
            verification: self
                .store
                .get_verification_result(session_id, nid)
                .ok()
                .flatten(),
            artifact_bundle: self
                .store
                .get_artifact_bundle(session_id, nid)
                .ok()
                .flatten(),
            sheaf_validations: self
                .store
                .get_sheaf_validations(session_id, nid)
                .unwrap_or_default(),
            interface_seals: self
                .store
                .get_interface_seals(session_id, nid)
                .unwrap_or_default(),
            context_provenance: self
                .store
                .get_context_provenance(session_id, nid)
                .ok()
                .flatten(),
        }
    }
}
