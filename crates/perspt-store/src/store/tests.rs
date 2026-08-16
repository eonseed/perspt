use super::*;

/// Create an in-memory store for testing
fn test_store() -> SessionStore {
    let temp_dir = std::env::temp_dir();
    let db_path = temp_dir.join(format!("perspt_test_{}.db", uuid::Uuid::new_v4()));
    SessionStore::open(&db_path).expect("Failed to create test store")
}

fn seed_session(store: &SessionStore, session_id: &str) {
    let record = SessionRecord {
        session_id: session_id.to_string(),
        task: "test task".to_string(),
        working_dir: "/tmp/test".to_string(),
        merkle_root: None,
        detected_toolchain: None,
        status: "RUNNING".to_string(),
    };
    store.create_session(&record).unwrap();
}

#[test]
fn test_node_state_phase8_roundtrip() {
    let store = test_store();
    let sid = "test-sess-1";
    seed_session(&store, sid);

    let record = NodeStateRecord {
        node_id: "node-1".to_string(),
        session_id: sid.to_string(),
        state: "Completed".to_string(),
        v_total: 0.42,
        merkle_hash: Some(vec![0xab; 32]),
        attempt_count: 3,
        node_class: Some("Interface".to_string()),
        owner_plugin: Some("rust".to_string()),
        goal: Some("Implement API".to_string()),
        parent_id: Some("root".to_string()),
        children: Some(r#"["child-a","child-b"]"#.to_string()),
        last_error_type: Some("CompilationError".to_string()),
        committed_at: Some("2025-01-01T00:00:00Z".to_string()),
    };

    store.record_node_state(&record).unwrap();

    let states = store.get_latest_node_states(sid).unwrap();
    assert_eq!(states.len(), 1);
    let r = &states[0];
    assert_eq!(r.node_id, "node-1");
    assert_eq!(r.state, "Completed");
    assert_eq!(r.attempt_count, 3);
    assert_eq!(r.node_class.as_deref(), Some("Interface"));
    assert_eq!(r.owner_plugin.as_deref(), Some("rust"));
    assert_eq!(r.goal.as_deref(), Some("Implement API"));
    assert_eq!(r.parent_id.as_deref(), Some("root"));
    assert!(r.children.is_some());
    assert_eq!(r.last_error_type.as_deref(), Some("CompilationError"));
    assert_eq!(r.committed_at.as_deref(), Some("2025-01-01T00:00:00Z"));
}

#[test]
fn test_task_graph_edge_roundtrip() {
    let store = test_store();
    let sid = "test-graph-1";
    seed_session(&store, sid);

    let edge = TaskGraphEdgeRow {
        session_id: sid.to_string(),
        parent_node_id: "parent-1".to_string(),
        child_node_id: "child-1".to_string(),
        edge_type: "depends_on".to_string(),
    };
    store.record_task_graph_edge(&edge).unwrap();

    let edges = store.get_task_graph_edges(sid).unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].parent_node_id, "parent-1");
    assert_eq!(edges[0].child_node_id, "child-1");
    assert_eq!(edges[0].edge_type, "depends_on");
}

#[test]
fn test_verification_result_roundtrip() {
    let store = test_store();
    let sid = "test-vr-1";
    seed_session(&store, sid);

    let row = VerificationResultRow {
        session_id: sid.to_string(),
        node_id: "node-v".to_string(),
        result_json: r#"{"syntax_ok":true}"#.to_string(),
        syntax_ok: true,
        build_ok: true,
        tests_ok: false,
        lint_ok: true,
        diagnostics_count: 2,
        tests_passed: 5,
        tests_failed: 1,
        degraded: false,
        degraded_reason: None,
    };
    store.record_verification_result(&row).unwrap();

    let got = store.get_verification_result(sid, "node-v").unwrap();
    assert!(got.is_some());
    let got = got.unwrap();
    assert!(got.syntax_ok);
    assert!(got.build_ok);
    assert!(!got.tests_ok);
    assert_eq!(got.tests_passed, 5);
    assert_eq!(got.tests_failed, 1);
    assert!(!got.degraded);
}

#[test]
fn test_verification_result_degraded() {
    let store = test_store();
    let sid = "test-vr-deg";
    seed_session(&store, sid);

    let row = VerificationResultRow {
        session_id: sid.to_string(),
        node_id: "node-d".to_string(),
        result_json: "{}".to_string(),
        syntax_ok: true,
        build_ok: false,
        tests_ok: false,
        lint_ok: false,
        diagnostics_count: 0,
        tests_passed: 0,
        tests_failed: 0,
        degraded: true,
        degraded_reason: Some("LSP unavailable".to_string()),
    };
    store.record_verification_result(&row).unwrap();

    let got = store
        .get_verification_result(sid, "node-d")
        .unwrap()
        .unwrap();
    assert!(got.degraded);
    assert_eq!(got.degraded_reason.as_deref(), Some("LSP unavailable"));
}

#[test]
fn test_artifact_bundle_roundtrip() {
    let store = test_store();
    let sid = "test-ab-1";
    seed_session(&store, sid);

    let row = ArtifactBundleRow {
        session_id: sid.to_string(),
        node_id: "node-a".to_string(),
        bundle_json: r#"{"artifacts":[],"commands":[]}"#.to_string(),
        artifact_count: 3,
        command_count: 1,
        touched_files: r#"["src/main.rs","src/lib.rs","tests/test.rs"]"#.to_string(),
    };
    store.record_artifact_bundle(&row).unwrap();

    let got = store.get_artifact_bundle(sid, "node-a").unwrap();
    assert!(got.is_some());
    let got = got.unwrap();
    assert_eq!(got.artifact_count, 3);
    assert_eq!(got.command_count, 1);
    assert!(got.touched_files.contains("main.rs"));
}

#[test]
fn test_latest_node_states_dedup() {
    let store = test_store();
    let sid = "test-dedup";
    seed_session(&store, sid);

    // Insert two states for the same node
    let r1 = NodeStateRecord {
        node_id: "node-x".to_string(),
        session_id: sid.to_string(),
        state: "Coding".to_string(),
        v_total: 0.5,
        merkle_hash: None,
        attempt_count: 1,
        node_class: None,
        owner_plugin: None,
        goal: None,
        parent_id: None,
        children: None,
        last_error_type: None,
        committed_at: None,
    };
    store.record_node_state(&r1).unwrap();

    let r2 = NodeStateRecord {
        node_id: "node-x".to_string(),
        session_id: sid.to_string(),
        state: "Completed".to_string(),
        v_total: 0.3,
        merkle_hash: None,
        attempt_count: 2,
        node_class: Some("Implementation".to_string()),
        owner_plugin: None,
        goal: Some("Updated goal".to_string()),
        parent_id: None,
        children: None,
        last_error_type: None,
        committed_at: Some("2025-01-02T00:00:00Z".to_string()),
    };
    store.record_node_state(&r2).unwrap();

    // get_latest should return only the last entry
    let latest = store.get_latest_node_states(sid).unwrap();
    assert_eq!(latest.len(), 1);
    assert_eq!(latest[0].state, "Completed");
    assert_eq!(latest[0].attempt_count, 2);
    assert_eq!(latest[0].goal.as_deref(), Some("Updated goal"));
}

#[test]
fn test_backward_compat_empty_phase8_fields() {
    let store = test_store();
    let sid = "test-compat";
    seed_session(&store, sid);

    // Insert a node with all Phase 8 fields as None (pre-Phase-8 session)
    let r = NodeStateRecord {
        node_id: "old-node".to_string(),
        session_id: sid.to_string(),
        state: "COMPLETED".to_string(),
        v_total: 1.0,
        merkle_hash: None,
        attempt_count: 1,
        node_class: None,
        owner_plugin: None,
        goal: None,
        parent_id: None,
        children: None,
        last_error_type: None,
        committed_at: None,
    };
    store.record_node_state(&r).unwrap();

    let latest = store.get_latest_node_states(sid).unwrap();
    assert_eq!(latest.len(), 1);
    assert!(latest[0].node_class.is_none());
    assert!(latest[0].goal.is_none());
    assert!(latest[0].committed_at.is_none());

    // Verification and artifact lookups should return None
    let vr = store.get_verification_result(sid, "old-node").unwrap();
    assert!(vr.is_none());
    let ab = store.get_artifact_bundle(sid, "old-node").unwrap();
    assert!(ab.is_none());
}

#[test]
fn test_review_outcome_roundtrip() {
    let store = test_store();
    let sid = "test-review";
    seed_session(&store, sid);

    let row = ReviewOutcomeRow {
        session_id: sid.to_string(),
        node_id: "node-r".to_string(),
        outcome: "approved".to_string(),
        reviewer_note: Some("LGTM".to_string()),
        energy_at_review: None,
        degraded: None,
        escalation_category: None,
    };
    store.record_review_outcome(&row).unwrap();

    let outcomes = store.get_review_outcomes(sid, "node-r").unwrap();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].outcome, "approved");
    assert_eq!(outcomes[0].reviewer_note.as_deref(), Some("LGTM"));
}

#[test]
fn test_review_outcome_with_audit_fields() {
    let store = test_store();
    let sid = "test-review-audit";
    seed_session(&store, sid);

    let row = ReviewOutcomeRow {
        session_id: sid.to_string(),
        node_id: "node-a".to_string(),
        outcome: "rejected".to_string(),
        reviewer_note: Some("Needs rework".to_string()),
        energy_at_review: Some(0.42),
        degraded: Some(true),
        escalation_category: Some("complexity".to_string()),
    };
    store.record_review_outcome(&row).unwrap();

    let outcomes = store.get_review_outcomes(sid, "node-a").unwrap();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].outcome, "rejected");
    assert_eq!(outcomes[0].energy_at_review, Some(0.42));
    assert_eq!(outcomes[0].degraded, Some(true));
    assert_eq!(
        outcomes[0].escalation_category.as_deref(),
        Some("complexity")
    );
}

#[test]
fn test_get_all_review_outcomes() {
    let store = test_store();
    let sid = "test-review-all";
    seed_session(&store, sid);

    for (node, outcome) in &[("n1", "approved"), ("n2", "rejected"), ("n1", "approved")] {
        let row = ReviewOutcomeRow {
            session_id: sid.to_string(),
            node_id: node.to_string(),
            outcome: outcome.to_string(),
            reviewer_note: None,
            energy_at_review: None,
            degraded: None,
            escalation_category: None,
        };
        store.record_review_outcome(&row).unwrap();
    }

    let all = store.get_all_review_outcomes(sid).unwrap();
    assert_eq!(all.len(), 3);
}

#[test]
fn test_feature_charter_roundtrip() {
    let store = test_store();
    let sid = "test-charter";
    seed_session(&store, sid);

    let row = FeatureCharterRow {
        charter_id: "ch-1".to_string(),
        session_id: sid.to_string(),
        scope_description: "Add authentication module".to_string(),
        max_modules: Some(3),
        max_files: Some(10),
        max_revisions: Some(5),
        language_constraint: Some("rust".to_string()),
    };
    store.record_feature_charter(&row).unwrap();

    let got = store.get_feature_charter(sid).unwrap();
    assert!(got.is_some());
    let got = got.unwrap();
    assert_eq!(got.charter_id, "ch-1");
    assert_eq!(got.scope_description, "Add authentication module");
    assert_eq!(got.max_modules, Some(3));
    assert_eq!(got.language_constraint.as_deref(), Some("rust"));
}

#[test]
fn test_feature_charter_returns_none_for_missing() {
    let store = test_store();
    let sid = "test-charter-miss";
    seed_session(&store, sid);

    let got = store.get_feature_charter(sid).unwrap();
    assert!(got.is_none());
}

#[test]
fn test_plan_revision_roundtrip() {
    let store = test_store();
    let sid = "test-rev";
    seed_session(&store, sid);

    let row = PlanRevisionRow {
        revision_id: "rev-1".to_string(),
        session_id: sid.to_string(),
        sequence: 1,
        plan_json: r#"{"tasks":[]}"#.to_string(),
        reason: "initial plan".to_string(),
        supersedes: None,
        status: "active".to_string(),
    };
    store.record_plan_revision(&row).unwrap();

    let active = store.get_active_plan_revision(sid).unwrap();
    assert!(active.is_some());
    let active = active.unwrap();
    assert_eq!(active.revision_id, "rev-1");
    assert_eq!(active.sequence, 1);
    assert_eq!(active.status, "active");
}

#[test]
fn test_plan_revision_supersede() {
    let store = test_store();
    let sid = "test-rev-sup";
    seed_session(&store, sid);

    let r1 = PlanRevisionRow {
        revision_id: "rev-1".to_string(),
        session_id: sid.to_string(),
        sequence: 1,
        plan_json: "{}".to_string(),
        reason: "initial".to_string(),
        supersedes: None,
        status: "active".to_string(),
    };
    store.record_plan_revision(&r1).unwrap();

    // Supersede rev-1
    store.supersede_plan_revision("rev-1").unwrap();

    let r2 = PlanRevisionRow {
        revision_id: "rev-2".to_string(),
        session_id: sid.to_string(),
        sequence: 2,
        plan_json: r#"{"tasks":["a"]}"#.to_string(),
        reason: "verifier feedback".to_string(),
        supersedes: Some("rev-1".to_string()),
        status: "active".to_string(),
    };
    store.record_plan_revision(&r2).unwrap();

    // Only rev-2 should be active
    let active = store.get_active_plan_revision(sid).unwrap().unwrap();
    assert_eq!(active.revision_id, "rev-2");

    // All revisions returned in order
    let all = store.get_plan_revisions(sid).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].status, "superseded");
    assert_eq!(all[1].status, "active");
}

#[test]
fn test_repair_footprint_roundtrip() {
    let store = test_store();
    let sid = "test-repair";
    seed_session(&store, sid);

    let row = RepairFootprintRow {
        footprint_id: "fp-1".to_string(),
        session_id: sid.to_string(),
        node_id: "node-a".to_string(),
        revision_id: "rev-1".to_string(),
        attempt: 1,
        affected_files: r#"["src/main.rs"]"#.to_string(),
        bundle_json: "{}".to_string(),
        diagnosis: "missing import".to_string(),
        resolved: false,
    };
    store.record_repair_footprint(&row).unwrap();

    let footprints = store.get_repair_footprints(sid, "node-a").unwrap();
    assert_eq!(footprints.len(), 1);
    assert_eq!(footprints[0].footprint_id, "fp-1");
    assert_eq!(footprints[0].diagnosis, "missing import");
    assert!(!footprints[0].resolved);
}

#[test]
fn test_repair_footprint_resolve() {
    let store = test_store();
    let sid = "test-repair-res";
    seed_session(&store, sid);

    let row = RepairFootprintRow {
        footprint_id: "fp-2".to_string(),
        session_id: sid.to_string(),
        node_id: "node-b".to_string(),
        revision_id: "rev-1".to_string(),
        attempt: 1,
        affected_files: "[]".to_string(),
        bundle_json: "{}".to_string(),
        diagnosis: "type error".to_string(),
        resolved: false,
    };
    store.record_repair_footprint(&row).unwrap();

    store.resolve_repair_footprint("fp-2").unwrap();

    let footprints = store.get_repair_footprints(sid, "node-b").unwrap();
    assert_eq!(footprints.len(), 1);
    assert!(footprints[0].resolved);
}

#[test]
fn test_budget_envelope_upsert_and_get() {
    let store = test_store();
    let sid = "test-budget";
    seed_session(&store, sid);

    let row = BudgetEnvelopeRow {
        session_id: sid.to_string(),
        max_steps: Some(100),
        steps_used: 5,
        max_revisions: Some(10),
        revisions_used: 1,
        max_cost_usd: Some(5.0),
        cost_used_usd: 0.25,
    };
    store.upsert_budget_envelope(&row).unwrap();

    let got = store.get_budget_envelope(sid).unwrap();
    assert!(got.is_some());
    let got = got.unwrap();
    assert_eq!(got.max_steps, Some(100));
    assert_eq!(got.steps_used, 5);
    assert_eq!(got.cost_used_usd, 0.25);
}

#[test]
fn test_budget_envelope_upsert_updates() {
    let store = test_store();
    let sid = "test-budget-up";
    seed_session(&store, sid);

    let row1 = BudgetEnvelopeRow {
        session_id: sid.to_string(),
        max_steps: Some(100),
        steps_used: 0,
        max_revisions: None,
        revisions_used: 0,
        max_cost_usd: None,
        cost_used_usd: 0.0,
    };
    store.upsert_budget_envelope(&row1).unwrap();

    // Update with new values
    let row2 = BudgetEnvelopeRow {
        session_id: sid.to_string(),
        max_steps: Some(100),
        steps_used: 42,
        max_revisions: Some(5),
        revisions_used: 3,
        max_cost_usd: Some(10.0),
        cost_used_usd: 4.5,
    };
    store.upsert_budget_envelope(&row2).unwrap();

    let got = store.get_budget_envelope(sid).unwrap().unwrap();
    assert_eq!(got.steps_used, 42);
    assert_eq!(got.revisions_used, 3);
    assert_eq!(got.cost_used_usd, 4.5);
}

#[test]
fn test_budget_envelope_missing_returns_none() {
    let store = test_store();
    let sid = "test-budget-miss";
    seed_session(&store, sid);

    let got = store.get_budget_envelope(sid).unwrap();
    assert!(got.is_none());
}

#[test]
fn test_read_only_store_queries_work() {
    let temp_dir = std::env::temp_dir();
    let db_path = temp_dir.join(format!("perspt_ro_test_{}.db", uuid::Uuid::new_v4()));

    // Create and seed a normal store
    {
        let store = SessionStore::open(&db_path).unwrap();
        seed_session(&store, "ro-test");
    }

    // Open read-only and verify queries work
    let ro = SessionStore::open_read_only(&db_path).unwrap();
    let sessions = ro.list_recent_sessions(10).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, "ro-test");
}

#[test]
fn test_read_only_store_rejects_writes() {
    let temp_dir = std::env::temp_dir();
    let db_path = temp_dir.join(format!("perspt_ro_wr_{}.db", uuid::Uuid::new_v4()));

    // Create the DB first
    {
        let _store = SessionStore::open(&db_path).unwrap();
    }

    // Open read-only and verify writes fail
    let ro = SessionStore::open_read_only(&db_path).unwrap();
    let record = SessionRecord {
        session_id: "should-fail".to_string(),
        task: "test".to_string(),
        working_dir: "/tmp".to_string(),
        merkle_root: None,
        detected_toolchain: None,
        status: "RUNNING".to_string(),
    };
    assert!(ro.create_session(&record).is_err());
}

// =================================================================
// PSP-7: SRBN step records and correction attempts round-trip tests
// =================================================================

#[test]
fn test_srbn_step_record_roundtrip() {
    let store = test_store();
    let sid = "step-sess";
    seed_session(&store, sid);

    let r1 = SrbnStepRecord {
        session_id: sid.to_string(),
        node_id: "n1".to_string(),
        step: "speculate".to_string(),
        outcome: "ok".to_string(),
        energy_json: Some(r#"{"v_syn":0.0}"#.to_string()),
        parse_state: Some("FullyParsed".to_string()),
        retry_classification: None,
        attempt_count: 0,
        duration_ms: 120,
    };
    let r2 = SrbnStepRecord {
        session_id: sid.to_string(),
        node_id: "n1".to_string(),
        step: "verify".to_string(),
        outcome: "retry".to_string(),
        energy_json: Some(r#"{"v_syn":5.0}"#.to_string()),
        parse_state: None,
        retry_classification: Some("MalformedResponse".to_string()),
        attempt_count: 1,
        duration_ms: 300,
    };

    store.record_step(&r1).unwrap();
    store.record_step(&r2).unwrap();

    let timeline = store.get_step_timeline(sid, "n1").unwrap();
    assert_eq!(timeline.len(), 2);
    assert_eq!(timeline[0].step, "speculate");
    assert_eq!(timeline[0].outcome, "ok");
    assert_eq!(timeline[0].duration_ms, 120);
    assert_eq!(timeline[1].step, "verify");
    assert_eq!(
        timeline[1].retry_classification.as_deref(),
        Some("MalformedResponse")
    );

    let all = store.get_session_steps(sid).unwrap();
    assert_eq!(all.len(), 2);
}

#[test]
fn test_correction_attempt_roundtrip() {
    let store = test_store();
    let sid = "corr-sess";
    seed_session(&store, sid);

    let a1 = CorrectionAttemptRow {
        session_id: sid.to_string(),
        node_id: "n1".to_string(),
        attempt: 1,
        parse_state: "FullyParsed".to_string(),
        retry_classification: None,
        response_fingerprint: "abc123".to_string(),
        response_length: 4096,
        energy_json: Some(r#"{"v_syn":2.0}"#.to_string()),
        accepted: false,
        rejection_reason: Some("v_syn too high".to_string()),
        created_at: 1700000000,
    };
    let a2 = CorrectionAttemptRow {
        session_id: sid.to_string(),
        node_id: "n1".to_string(),
        attempt: 2,
        parse_state: "FullyParsed".to_string(),
        retry_classification: None,
        response_fingerprint: "def456".to_string(),
        response_length: 3800,
        energy_json: Some(r#"{"v_syn":0.0}"#.to_string()),
        accepted: true,
        rejection_reason: None,
        created_at: 1700000010,
    };

    store.record_correction_attempt(&a1).unwrap();
    store.record_correction_attempt(&a2).unwrap();

    let attempts = store.get_correction_attempts(sid, "n1").unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].attempt, 1);
    assert!(!attempts[0].accepted);
    assert_eq!(
        attempts[0].rejection_reason.as_deref(),
        Some("v_syn too high")
    );
    assert_eq!(attempts[1].attempt, 2);
    assert!(attempts[1].accepted);
    assert!(attempts[1].rejection_reason.is_none());
}

#[test]
fn test_step_timeline_filters_by_node() {
    let store = test_store();
    let sid = "filter-sess";
    seed_session(&store, sid);

    for (node, step) in [("n1", "speculate"), ("n2", "speculate"), ("n1", "verify")] {
        store
            .record_step(&SrbnStepRecord {
                session_id: sid.to_string(),
                node_id: node.to_string(),
                step: step.to_string(),
                outcome: "ok".to_string(),
                energy_json: None,
                parse_state: None,
                retry_classification: None,
                attempt_count: 0,
                duration_ms: 0,
            })
            .unwrap();
    }

    assert_eq!(store.get_step_timeline(sid, "n1").unwrap().len(), 2);
    assert_eq!(store.get_step_timeline(sid, "n2").unwrap().len(), 1);
    assert_eq!(store.get_session_steps(sid).unwrap().len(), 3);
}
