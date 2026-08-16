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
        artifacts: vec![ArtifactOperation::Write {
            path: "src/main.rs".to_string(),
            content: "fn main() {}".to_string(),
        }],
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

// =========================================================================
// PSP-5 Phase 2: Ownership Manifest Tests
// =========================================================================

#[test]
fn test_ownership_manifest_assign_and_lookup() {
    let mut manifest = OwnershipManifest::new();
    manifest.assign("src/main.rs", "node_1", "rust", NodeClass::Implementation);
    manifest.assign("src/lib.rs", "node_1", "rust", NodeClass::Implementation);
    manifest.assign("tests/test.rs", "node_2", "rust", NodeClass::Integration);

    // owner_of
    let entry = manifest.owner_of("src/main.rs").unwrap();
    assert_eq!(entry.owner_node_id, "node_1");
    assert_eq!(entry.owner_plugin, "rust");
    assert_eq!(entry.node_class, NodeClass::Implementation);

    assert!(manifest.owner_of("nonexistent.rs").is_none());

    // files_owned_by
    let mut files = manifest.files_owned_by("node_1");
    files.sort();
    assert_eq!(files, vec!["src/lib.rs", "src/main.rs"]);

    let files_2 = manifest.files_owned_by("node_2");
    assert_eq!(files_2, vec!["tests/test.rs"]);

    assert_eq!(manifest.len(), 3);
    assert!(!manifest.is_empty());
}

#[test]
fn test_ownership_manifest_validate_bundle_ok() {
    let mut manifest = OwnershipManifest::new();
    manifest.assign("src/main.rs", "node_1", "rust", NodeClass::Implementation);
    manifest.assign("src/lib.rs", "node_1", "rust", NodeClass::Implementation);

    let bundle = ArtifactBundle {
        artifacts: vec![
            ArtifactOperation::Write {
                path: "src/main.rs".to_string(),
                content: "fn main() {}".to_string(),
            },
            ArtifactOperation::Write {
                path: "src/lib.rs".to_string(),
                content: "pub fn lib() {}".to_string(),
            },
        ],
        commands: vec![],
    };

    // node_1 owns both files → should pass
    assert!(manifest
        .validate_bundle(&bundle, "node_1", NodeClass::Implementation)
        .is_ok());
}

#[test]
fn test_ownership_manifest_validate_bundle_cross_owner_rejected() {
    let mut manifest = OwnershipManifest::new();
    manifest.assign("src/main.rs", "node_1", "rust", NodeClass::Implementation);
    manifest.assign("src/other.rs", "node_2", "rust", NodeClass::Implementation);

    let bundle = ArtifactBundle {
        artifacts: vec![
            ArtifactOperation::Write {
                path: "src/main.rs".to_string(),
                content: "fn main() {}".to_string(),
            },
            ArtifactOperation::Write {
                path: "src/other.rs".to_string(),
                content: "fn other() {}".to_string(),
            },
        ],
        commands: vec![],
    };

    // node_1 tries to modify node_2's file → rejected
    let result = manifest.validate_bundle(&bundle, "node_1", NodeClass::Implementation);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Ownership violation"));
}

#[test]
fn test_ownership_manifest_validate_integration_cross_owner_ok() {
    let mut manifest = OwnershipManifest::new();
    manifest.assign("src/main.rs", "node_1", "rust", NodeClass::Implementation);
    manifest.assign("src/other.rs", "node_2", "rust", NodeClass::Implementation);

    let bundle = ArtifactBundle {
        artifacts: vec![
            ArtifactOperation::Write {
                path: "src/main.rs".to_string(),
                content: "fn main() {}".to_string(),
            },
            ArtifactOperation::Write {
                path: "src/other.rs".to_string(),
                content: "fn other() {}".to_string(),
            },
        ],
        commands: vec![],
    };

    // Integration node can cross ownership boundaries
    let result = manifest.validate_bundle(&bundle, "node_3", NodeClass::Integration);
    assert!(result.is_ok());
}

#[test]
fn test_ownership_manifest_fanout_limit() {
    let manifest = OwnershipManifest::with_fanout_limit(2);

    let bundle = ArtifactBundle {
        artifacts: vec![
            ArtifactOperation::Write {
                path: "a.rs".to_string(),
                content: "a".to_string(),
            },
            ArtifactOperation::Write {
                path: "b.rs".to_string(),
                content: "b".to_string(),
            },
            ArtifactOperation::Write {
                path: "c.rs".to_string(),
                content: "c".to_string(),
            },
        ],
        commands: vec![],
    };

    // 3 artifacts exceeds fanout limit of 2
    let result = manifest.validate_bundle(&bundle, "node_1", NodeClass::Implementation);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("fanout limit"));

    // Exactly at the limit should pass
    let small_bundle = ArtifactBundle {
        artifacts: vec![
            ArtifactOperation::Write {
                path: "a.rs".to_string(),
                content: "a".to_string(),
            },
            ArtifactOperation::Write {
                path: "b.rs".to_string(),
                content: "b".to_string(),
            },
        ],
        commands: vec![],
    };
    assert!(manifest
        .validate_bundle(&small_bundle, "node_1", NodeClass::Implementation)
        .is_ok());
}

#[test]
fn test_ownership_manifest_assign_new_paths() {
    let mut manifest = OwnershipManifest::new();
    manifest.assign("src/main.rs", "node_1", "rust", NodeClass::Implementation);

    let bundle = ArtifactBundle {
        artifacts: vec![
            ArtifactOperation::Write {
                path: "src/main.rs".to_string(),
                content: "existing".to_string(),
            },
            ArtifactOperation::Write {
                path: "src/new_file.rs".to_string(),
                content: "new".to_string(),
            },
        ],
        commands: vec![],
    };

    manifest.assign_new_paths(&bundle, "node_1", "rust", NodeClass::Implementation);

    // Existing entry unchanged
    assert_eq!(
        manifest.owner_of("src/main.rs").unwrap().owner_node_id,
        "node_1"
    );
    // New path auto-assigned
    let new_entry = manifest.owner_of("src/new_file.rs").unwrap();
    assert_eq!(new_entry.owner_node_id, "node_1");
    assert_eq!(new_entry.owner_plugin, "rust");
    assert_eq!(manifest.len(), 2);
}

// =========================================================================
// PSP-5: Plan Ownership Closure Validation Tests
// =========================================================================

#[test]
fn test_plan_validate_duplicate_output_files_rejected() {
    let plan = TaskPlan {
        tasks: vec![
            PlannedTask {
                id: "task_1".into(),
                goal: "Create math module".into(),
                output_files: vec!["src/math.py".into(), "tests/test_math.py".into()],
                ..PlannedTask::new("task_1", "Create math module")
            },
            PlannedTask {
                id: "task_2".into(),
                goal: "Create tests".into(),
                output_files: vec!["tests/test_math.py".into()],
                ..PlannedTask::new("task_2", "Create tests")
            },
        ],
    };
    let result = plan.validate();
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("tests/test_math.py"),
        "Error should mention the duplicate file: {}",
        err
    );
    assert!(
        err.contains("Ownership violation"),
        "Error should mention ownership: {}",
        err
    );
}

#[test]
fn test_plan_validate_unique_output_files_ok() {
    let plan = TaskPlan {
        tasks: vec![
            PlannedTask {
                id: "task_1".into(),
                goal: "Create math module".into(),
                output_files: vec!["src/math.py".into()],
                ..PlannedTask::new("task_1", "Create math module")
            },
            PlannedTask {
                id: "test_1".into(),
                goal: "Tests for math".into(),
                output_files: vec!["tests/test_math.py".into()],
                dependencies: vec!["task_1".into()],
                ..PlannedTask::new("test_1", "Tests for math")
            },
        ],
    };
    assert!(plan.validate().is_ok());
}

#[test]
fn test_plan_validate_context_files_do_not_conflict_with_output_files() {
    // Reading another task's file via context_files is fine
    let plan = TaskPlan {
        tasks: vec![
            PlannedTask {
                id: "task_1".into(),
                goal: "Create math module".into(),
                output_files: vec!["src/math.py".into()],
                ..PlannedTask::new("task_1", "Create math module")
            },
            PlannedTask {
                id: "test_1".into(),
                goal: "Tests for math".into(),
                context_files: vec!["src/math.py".into()], // reading, not owning
                output_files: vec!["tests/test_math.py".into()],
                dependencies: vec!["task_1".into()],
                ..PlannedTask::new("test_1", "Tests for math")
            },
        ],
    };
    assert!(plan.validate().is_ok());
}

// =========================================================================
// PSP-5 Phase 3: Structural Digests, Context Packages, Provenance Tests
// =========================================================================

#[test]
fn test_structural_digest_from_content() {
    let digest = StructuralDigest::from_content(
        "node_1",
        "src/main.rs",
        ArtifactKind::Signature,
        b"fn main() {}",
    );

    assert_eq!(digest.source_node_id, "node_1");
    assert_eq!(digest.source_path, "src/main.rs");
    assert_eq!(digest.artifact_kind, ArtifactKind::Signature);
    assert_eq!(digest.version, 1);
    assert!(!digest.digest_id.is_empty());
    // Hash must be non-zero
    assert_ne!(digest.hash, [0u8; 32]);
}

#[test]
fn test_structural_digest_matches() {
    let d1 = StructuralDigest::from_content(
        "node_1",
        "src/main.rs",
        ArtifactKind::Signature,
        b"fn main() {}",
    );
    let d2 = StructuralDigest::from_content(
        "node_1",
        "src/main.rs",
        ArtifactKind::Signature,
        b"fn main() {}",
    );
    let d3 = StructuralDigest::from_content(
        "node_1",
        "src/main.rs",
        ArtifactKind::Signature,
        b"fn main() { println!(); }",
    );

    assert!(d1.matches(&d2));
    assert!(!d1.matches(&d3));
}

#[test]
fn test_context_budget_default() {
    let budget = ContextBudget::default();
    assert_eq!(budget.byte_limit, 100 * 1024); // 100KB
    assert_eq!(budget.file_count_limit, 20);
}

#[test]
fn test_restriction_map_for_node() {
    let map = RestrictionMap::for_node("node_1".to_string());
    assert_eq!(map.node_id, "node_1");
    assert!(map.owned_files.is_empty());
    assert!(map.sealed_interfaces.is_empty());
    assert_eq!(map.budget, ContextBudget::default());
}

#[test]
fn test_restriction_map_structural_bytes() {
    let mut map = RestrictionMap::for_node("node_1".to_string());
    let d =
        StructuralDigest::from_content("n1", "src/a.rs", ArtifactKind::InterfaceSeal, b"content");
    map.structural_digests.push(d);
    // structural_bytes = source_path.len() + 64 per digest + sealed_interfaces * 128
    assert!(map.structural_bytes() > 0);
}

#[test]
fn test_context_package_add_file_within_budget() {
    let mut pkg = ContextPackage::new("node_1".to_string());
    pkg.restriction_map.budget.byte_limit = 1024;

    assert!(pkg.add_file("a.rs", "hello world".to_string()));
    assert_eq!(pkg.included_files.len(), 1);
    assert_eq!(pkg.total_bytes, 11);
    assert!(!pkg.budget_exceeded);
}

#[test]
fn test_context_package_add_file_exceeds_budget() {
    let mut pkg = ContextPackage::new("node_1".to_string());
    pkg.restriction_map.budget.byte_limit = 10;

    let result = pkg.add_file("big.rs", "this is more than ten bytes".to_string());
    assert!(!result);
    assert!(pkg.budget_exceeded);
    // File should not have been added
    assert!(pkg.included_files.is_empty());
}

#[test]
fn test_context_package_provenance() {
    let mut pkg = ContextPackage::new("node_1".to_string());
    pkg.add_file("a.rs", "content".to_string());

    let d = StructuralDigest::from_content("n1", "src/a.rs", ArtifactKind::Signature, b"data");
    pkg.add_structural_digest(d);

    let prov = pkg.provenance();
    assert_eq!(prov.node_id, "node_1");
    assert_eq!(prov.context_package_id, pkg.package_id);
    assert_eq!(prov.included_file_count, 1);
    assert_eq!(prov.structural_digest_hashes.len(), 1);
    assert!(prov.total_bytes > 0);
}

#[test]
fn test_context_provenance_default() {
    let prov = ContextProvenance::default();
    assert!(prov.node_id.is_empty());
    assert!(prov.structural_digest_hashes.is_empty());
    assert_eq!(prov.included_file_count, 0);
}

#[test]
fn test_artifact_kind_display() {
    assert_eq!(format!("{}", ArtifactKind::Signature), "signature");
    assert_eq!(format!("{}", ArtifactKind::InterfaceSeal), "interface_seal");
}

#[test]
fn test_sensor_status_display() {
    assert_eq!(format!("{}", SensorStatus::Available), "available");
    assert_eq!(
        format!(
            "{}",
            SensorStatus::Fallback {
                actual: "ruff".into(),
                reason: "primary not found".into()
            }
        ),
        "fallback(ruff)"
    );
    assert_eq!(
        format!(
            "{}",
            SensorStatus::Unavailable {
                reason: "not installed".into()
            }
        ),
        "unavailable(not installed)"
    );
}

#[test]
fn test_verification_result_no_degraded_stages() {
    let result = VerificationResult {
        syntax_ok: true,
        build_ok: true,
        tests_ok: true,
        lint_ok: true,
        stage_outcomes: vec![StageOutcome {
            stage: "syntax_check".into(),
            passed: true,
            sensor_status: SensorStatus::Available,
            output: None,
        }],
        ..Default::default()
    };
    assert!(result.all_passed());
    assert!(!result.has_degraded_stages());
    assert!(result.degraded_stage_reasons().is_empty());
}

#[test]
fn test_verification_result_with_fallback_blocks_stability() {
    let result = VerificationResult {
        syntax_ok: true,
        build_ok: true,
        tests_ok: true,
        lint_ok: true,
        stage_outcomes: vec![
            StageOutcome {
                stage: "syntax_check".into(),
                passed: true,
                sensor_status: SensorStatus::Available,
                output: None,
            },
            StageOutcome {
                stage: "test".into(),
                passed: true,
                sensor_status: SensorStatus::Fallback {
                    actual: "python -m pytest".into(),
                    reason: "uv not found".into(),
                },
                output: None,
            },
        ],
        ..Default::default()
    };
    // All tools passed but a fallback was used — should flag degraded
    assert!(result.has_degraded_stages());
    let reasons = result.degraded_stage_reasons();
    assert_eq!(reasons.len(), 1);
    assert!(reasons[0].contains("test"));
    assert!(reasons[0].contains("fallback"));
}

#[test]
fn test_verification_result_unavailable_stage() {
    let result = VerificationResult {
        syntax_ok: false,
        stage_outcomes: vec![StageOutcome {
            stage: "lint".into(),
            passed: false,
            sensor_status: SensorStatus::Unavailable {
                reason: "clippy not installed".into(),
            },
            output: None,
        }],
        ..Default::default()
    };
    assert!(result.has_degraded_stages());
    let reasons = result.degraded_stage_reasons();
    assert!(reasons[0].contains("clippy not installed"));
}

#[test]
fn test_verification_result_mixed_stages() {
    // A realistic result: syntax passed on primary, lint fell back, tests unavailable
    let result = VerificationResult {
        syntax_ok: true,
        tests_ok: false,
        lint_ok: false,
        stage_outcomes: vec![
            StageOutcome {
                stage: "syntax_check".into(),
                passed: true,
                sensor_status: SensorStatus::Available,
                output: Some("OK".into()),
            },
            StageOutcome {
                stage: "lint".into(),
                passed: true,
                sensor_status: SensorStatus::Fallback {
                    actual: "cargo check".into(),
                    reason: "clippy not found".into(),
                },
                output: Some("warnings only".into()),
            },
            StageOutcome {
                stage: "test".into(),
                passed: false,
                sensor_status: SensorStatus::Unavailable {
                    reason: "no test runner".into(),
                },
                output: None,
            },
        ],
        ..Default::default()
    };
    assert!(result.has_degraded_stages());
    let reasons = result.degraded_stage_reasons();
    // Both lint (fallback) and test (unavailable) should be degraded
    assert_eq!(reasons.len(), 2);
    assert!(reasons.iter().any(|r| r.contains("lint")));
    assert!(reasons.iter().any(|r| r.contains("test")));
}

// =========================================================================
// Phase 5: Escalation, graph rewrite, and sheaf validator types
// =========================================================================

#[test]
fn test_escalation_category_display() {
    assert_eq!(
        EscalationCategory::ImplementationError.to_string(),
        "implementation_error"
    );
    assert_eq!(
        EscalationCategory::ContractMismatch.to_string(),
        "contract_mismatch"
    );
    assert_eq!(
        EscalationCategory::DegradedSensors.to_string(),
        "degraded_sensors"
    );
}

#[test]
fn test_rewrite_action_grounded_retry() {
    let action = RewriteAction::GroundedRetry {
        evidence_summary: "build failed twice".into(),
    };
    match action {
        RewriteAction::GroundedRetry { evidence_summary } => {
            assert!(evidence_summary.contains("build failed"));
        }
        _ => panic!("Expected GroundedRetry"),
    }
}

#[test]
fn test_rewrite_action_node_split() {
    let action = RewriteAction::NodeSplit {
        proposed_children: vec!["child_a".into(), "child_b".into()],
    };
    match action {
        RewriteAction::NodeSplit { proposed_children } => {
            assert_eq!(proposed_children.len(), 2);
        }
        _ => panic!("Expected NodeSplit"),
    }
}

#[test]
fn test_sheaf_validator_class_display() {
    assert_eq!(
        SheafValidatorClass::DependencyGraphConsistency.to_string(),
        "dependency_graph"
    );
    assert_eq!(
        SheafValidatorClass::CrossLanguageBoundary.to_string(),
        "cross_language"
    );
}

#[test]
fn test_sheaf_validation_result_passed() {
    let result = SheafValidationResult::passed(
        SheafValidatorClass::DependencyGraphConsistency,
        vec!["node_1".into()],
    );
    assert!(result.passed);
    assert_eq!(result.v_sheaf_contribution, 0.0);
    assert!(result.evidence_summary.is_empty());
    assert!(result.requeue_targets.is_empty());
}

#[test]
fn test_sheaf_validation_result_failed() {
    let result = SheafValidationResult::failed(
        SheafValidatorClass::ExportImportConsistency,
        "ownership mismatch on 2 files",
        vec!["src/a.rs".into(), "src/b.rs".into()],
        vec!["node_2".into()],
        0.3,
    );
    assert!(!result.passed);
    assert_eq!(result.v_sheaf_contribution, 0.3);
    assert!(result.evidence_summary.contains("ownership mismatch"));
    assert_eq!(result.affected_files.len(), 2);
    assert_eq!(result.requeue_targets, vec!["node_2"]);
}

#[test]
fn test_escalation_report_roundtrip() {
    let report = EscalationReport {
        node_id: "test_node".into(),
        session_id: "sess_1".into(),
        category: EscalationCategory::TopologyMismatch,
        action: RewriteAction::InterfaceInsertion {
            boundary: "module_boundary".into(),
        },
        energy_snapshot: EnergyComponents::default(),
        stage_outcomes: Vec::new(),
        evidence: "violation at boundary".into(),
        affected_node_ids: vec!["dep_1".into()],
        timestamp: 12345,
    };
    let json = serde_json::to_string(&report).unwrap();
    let deser: EscalationReport = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.node_id, "test_node");
    assert_eq!(deser.category, EscalationCategory::TopologyMismatch);
    assert_eq!(deser.affected_node_ids.len(), 1);
}

#[test]
fn test_stability_monitor_reset_for_replan() {
    let mut monitor = StabilityMonitor::new();
    monitor.record_energy(0.8);
    monitor.record_energy(0.5);
    monitor.record_failure(ErrorType::Compilation);
    assert_eq!(monitor.attempt_count, 2);

    monitor.reset_for_replan();
    assert_eq!(monitor.attempt_count, 0);
    assert!(!monitor.stable);
    // History is preserved
    assert_eq!(monitor.energy_history.len(), 2);
}

#[test]
fn test_rewrite_record_serialization() {
    let record = RewriteRecord {
        node_id: "n1".into(),
        session_id: "s1".into(),
        action: RewriteAction::SubgraphReplan {
            affected_nodes: vec!["n2".into(), "n3".into()],
        },
        category: EscalationCategory::InsufficientModelCapability,
        requeued_nodes: vec!["n2".into(), "n3".into()],
        inserted_nodes: Vec::new(),
        timestamp: 99999,
    };
    let json = serde_json::to_string(&record).unwrap();
    let deser: RewriteRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.requeued_nodes.len(), 2);
    assert!(deser.inserted_nodes.is_empty());
}

// =========================================================================
// PSP-5 Phase 6: Provisional Branch and Seal Tests
// =========================================================================

#[test]
fn test_provisional_branch_state_display() {
    assert_eq!(ProvisionalBranchState::Active.to_string(), "active");
    assert_eq!(ProvisionalBranchState::Sealed.to_string(), "sealed");
    assert_eq!(ProvisionalBranchState::Merged.to_string(), "merged");
    assert_eq!(ProvisionalBranchState::Flushed.to_string(), "flushed");
}

#[test]
fn test_provisional_branch_lifecycle() {
    let branch = ProvisionalBranch::new("b1", "s1", "node_child", "node_parent");
    assert_eq!(branch.state, ProvisionalBranchState::Active);
    assert!(branch.is_live());
    assert!(!branch.is_flushed());
    assert!(branch.parent_seal_hash.is_none());
    assert!(branch.sandbox_dir.is_none());
    assert!(branch.created_at > 0);
}

#[test]
fn test_provisional_branch_flushed_not_live() {
    let mut branch = ProvisionalBranch::new("b1", "s1", "n1", "p1");
    branch.state = ProvisionalBranchState::Flushed;
    assert!(!branch.is_live());
    assert!(branch.is_flushed());
}

#[test]
fn test_provisional_branch_sealed_is_live() {
    let mut branch = ProvisionalBranch::new("b1", "s1", "n1", "p1");
    branch.state = ProvisionalBranchState::Sealed;
    assert!(branch.is_live());
    assert!(!branch.is_flushed());
}

#[test]
fn test_provisional_branch_serialization() {
    let branch = ProvisionalBranch::new("b1", "s1", "n1", "p1");
    let json = serde_json::to_string(&branch).unwrap();
    let deser: ProvisionalBranch = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.branch_id, "b1");
    assert_eq!(deser.state, ProvisionalBranchState::Active);
}

#[test]
fn test_branch_lineage_serialization() {
    let lineage = BranchLineage {
        lineage_id: "lin_1".into(),
        parent_branch_id: "parent_b".into(),
        child_branch_id: "child_b".into(),
        depends_on_seal: true,
    };
    let json = serde_json::to_string(&lineage).unwrap();
    let deser: BranchLineage = serde_json::from_str(&json).unwrap();
    assert!(deser.depends_on_seal);
    assert_eq!(deser.parent_branch_id, "parent_b");
}

#[test]
fn test_interface_seal_from_digest() {
    let digest = StructuralDigest::from_content(
        "node_iface",
        "src/api.rs",
        ArtifactKind::InterfaceSeal,
        b"pub fn hello() -> String",
    );
    let seal = InterfaceSealRecord::from_digest("sess1", "node_iface", &digest);
    assert_eq!(seal.node_id, "node_iface");
    assert_eq!(seal.sealed_path, "src/api.rs");
    assert!(seal.matches_hash(&digest.hash));
    assert!(!seal.matches_hash(&[0u8; 32]));
}

#[test]
fn test_branch_flush_record() {
    let flush = BranchFlushRecord::new(
        "s1",
        "parent_node",
        vec!["b1".into(), "b2".into()],
        vec!["child1".into(), "child2".into()],
        "Parent failed verification",
    );
    assert!(flush.flush_id.starts_with("flush_"));
    assert_eq!(flush.flushed_branch_ids.len(), 2);
    assert_eq!(flush.requeue_node_ids.len(), 2);
    assert!(flush.created_at > 0);
}

#[test]
fn test_blocked_dependency() {
    let dep = BlockedDependency::new("child_node", "parent_node", vec!["src/api.rs".into()]);
    assert_eq!(dep.child_node_id, "child_node");
    assert_eq!(dep.parent_node_id, "parent_node");
    assert_eq!(dep.required_seal_paths.len(), 1);
    assert!(dep.blocked_at > 0);
}

#[test]
fn test_srbn_node_phase6_fields() {
    let node = SRBNNode::new("n1".into(), "goal".into(), ModelTier::Actuator);
    assert!(node.provisional_branch_id.is_none());
    assert!(node.interface_seal_hash.is_none());
}

// =========================================================================
// Plan Revision, Charter, Repair, and Budget Tests
// =========================================================================

#[test]
fn test_plan_revision_initial() {
    let plan = TaskPlan {
        tasks: vec![PlannedTask::new("t1", "Do something")],
    };
    let rev = PlanRevision::initial("session_1", plan);
    assert_eq!(rev.sequence, 1);
    assert_eq!(rev.reason, "initial");
    assert!(rev.supersedes.is_none());
    assert!(rev.is_active());
    assert_eq!(rev.status, PlanRevisionStatus::Active);
}

#[test]
fn test_plan_revision_successor() {
    let plan1 = TaskPlan {
        tasks: vec![PlannedTask::new("t1", "First")],
    };
    let rev1 = PlanRevision::initial("s1", plan1);

    let plan2 = TaskPlan {
        tasks: vec![PlannedTask::new("t2", "Second")],
    };
    let rev2 = PlanRevision::successor(&rev1, plan2, "verification_failure");

    assert_eq!(rev2.sequence, 2);
    assert_eq!(rev2.reason, "verification_failure");
    assert_eq!(rev2.supersedes, Some(rev1.revision_id.clone()));
    assert!(rev2.is_active());
}

#[test]
fn test_plan_revision_status_display() {
    assert_eq!(PlanRevisionStatus::Active.to_string(), "active");
    assert_eq!(PlanRevisionStatus::Superseded.to_string(), "superseded");
    assert_eq!(PlanRevisionStatus::Cancelled.to_string(), "cancelled");
}

#[test]
fn test_planning_policy_defaults_and_queries() {
    let policy = PlanningPolicy::default();
    assert_eq!(policy, PlanningPolicy::FeatureIncrement);
    assert!(policy.needs_architect());
    assert!(!policy.needs_speculator());

    assert!(!PlanningPolicy::LocalEdit.needs_architect());
    assert!(!PlanningPolicy::LocalEdit.needs_speculator());

    assert!(PlanningPolicy::LargeFeature.needs_architect());
    assert!(PlanningPolicy::LargeFeature.needs_speculator());

    assert!(PlanningPolicy::GreenfieldBuild.needs_architect());
    assert!(PlanningPolicy::GreenfieldBuild.needs_speculator());

    assert!(PlanningPolicy::ArchitecturalRevision.needs_architect());
    assert!(PlanningPolicy::ArchitecturalRevision.needs_speculator());
}

#[test]
fn test_repair_footprint_creation() {
    let bundle = ArtifactBundle {
        artifacts: vec![ArtifactOperation::Write {
            path: "src/fix.rs".into(),
            content: "fixed".into(),
        }],
        commands: vec![],
    };
    let fp = RepairFootprint::new("s1", "node1", "rev1", 1, &bundle, "Syntax error");
    assert_eq!(fp.node_id, "node1");
    assert_eq!(fp.attempt, 1);
    assert_eq!(fp.affected_files, vec!["src/fix.rs"]);
    assert!(!fp.resolved);

    let mut fp = fp;
    fp.mark_resolved();
    assert!(fp.resolved);
}

#[test]
fn test_budget_envelope_tracking() {
    let mut budget = BudgetEnvelope::new("s1");
    budget.max_steps = Some(3);
    budget.max_revisions = Some(2);
    budget.max_cost_usd = Some(1.0);

    assert!(!budget.any_exhausted());

    budget.record_step();
    budget.record_step();
    assert!(!budget.steps_exhausted());
    budget.record_step();
    assert!(budget.steps_exhausted());
    assert!(budget.any_exhausted());
}

#[test]
fn test_budget_envelope_cost_tracking() {
    let mut budget = BudgetEnvelope::new("s1");
    budget.max_cost_usd = Some(0.50);
    budget.record_cost(0.25);
    assert!(!budget.cost_exhausted());
    budget.record_cost(0.30);
    assert!(budget.cost_exhausted());
}

#[test]
fn test_artifact_operation_delete_and_move() {
    let del = ArtifactOperation::Delete {
        path: "src/old.rs".into(),
    };
    assert!(del.is_delete());
    assert!(!del.is_write());
    assert_eq!(del.path(), "src/old.rs");

    let mv = ArtifactOperation::Move {
        from: "src/old.rs".into(),
        to: "src/new.rs".into(),
    };
    assert!(mv.is_move());
    assert!(!mv.is_write());
    assert_eq!(mv.path(), "src/old.rs");
}

#[test]
fn test_artifact_bundle_with_delete_and_move() {
    let bundle = ArtifactBundle {
        artifacts: vec![
            ArtifactOperation::Write {
                path: "src/new.rs".into(),
                content: "code".into(),
            },
            ArtifactOperation::Delete {
                path: "src/old.rs".into(),
            },
            ArtifactOperation::Move {
                from: "src/a.rs".into(),
                to: "src/b.rs".into(),
            },
        ],
        commands: vec![],
    };
    assert_eq!(bundle.writes_count(), 1);
    assert_eq!(bundle.deletes_count(), 1);
    assert_eq!(bundle.moves_count(), 1);
    assert!(bundle.validate().is_ok());

    let paths = bundle.affected_paths();
    assert!(paths.contains(&"src/new.rs"));
    assert!(paths.contains(&"src/old.rs"));
    assert!(paths.contains(&"src/a.rs"));
    assert!(paths.contains(&"src/b.rs"));
}

#[test]
fn test_artifact_bundle_move_validation() {
    // Move with traversal in destination should fail
    let bundle = ArtifactBundle {
        artifacts: vec![ArtifactOperation::Move {
            from: "src/a.rs".into(),
            to: "../outside.rs".into(),
        }],
        commands: vec![],
    };
    assert!(bundle.validate().is_err());
}

#[test]
fn test_dependency_expectation_default() {
    let de = DependencyExpectation::default();
    assert!(de.required_packages.is_empty());
    assert!(de.setup_commands.is_empty());
    assert!(de.min_toolchain_version.is_none());
}

#[test]
fn test_planned_task_has_dependency_expectations() {
    let task = PlannedTask::new("t1", "Build module");
    assert!(task.dependency_expectations.required_packages.is_empty());
}

#[test]
fn test_srbn_node_carries_dependency_expectations() {
    let mut task = PlannedTask::new("t1", "Build module");
    task.dependency_expectations = DependencyExpectation {
        required_packages: vec!["serde".to_string(), "tokio".to_string()],
        setup_commands: vec!["cargo fetch".to_string()],
        min_toolchain_version: Some("1.75".to_string()),
    };
    let node = task.to_srbn_node(ModelTier::Actuator);
    assert_eq!(node.dependency_expectations.required_packages.len(), 2);
    assert_eq!(node.dependency_expectations.required_packages[0], "serde");
    assert_eq!(node.dependency_expectations.setup_commands, ["cargo fetch"]);
    assert_eq!(
        node.dependency_expectations
            .min_toolchain_version
            .as_deref(),
        Some("1.75")
    );
}

#[test]
fn test_dependency_expectations_deserialized_from_json() {
    let json = r#"{
        "id": "t1",
        "goal": "Build module",
        "dependency_expectations": {
            "required_packages": ["requests", "pydantic"],
            "setup_commands": [],
            "min_toolchain_version": "3.11"
        }
    }"#;
    let task: PlannedTask = serde_json::from_str(json).unwrap();
    assert_eq!(task.dependency_expectations.required_packages.len(), 2);
    assert_eq!(
        task.dependency_expectations
            .min_toolchain_version
            .as_deref(),
        Some("3.11")
    );
}

#[test]
fn test_dependency_expectations_default_when_omitted() {
    let json = r#"{"id": "t1", "goal": "Build module"}"#;
    let task: PlannedTask = serde_json::from_str(json).unwrap();
    assert!(task.dependency_expectations.required_packages.is_empty());
    assert!(task.dependency_expectations.setup_commands.is_empty());
    assert!(task.dependency_expectations.min_toolchain_version.is_none());
}

#[test]
fn test_node_state_from_display_str_case_insensitive() {
    assert_eq!(
        NodeState::from_display_str("Completed"),
        NodeState::Completed
    );
    assert_eq!(
        NodeState::from_display_str("COMPLETED"),
        NodeState::Completed
    );
    assert_eq!(
        NodeState::from_display_str("completed"),
        NodeState::Completed
    );
    assert_eq!(
        NodeState::from_display_str("TaskQueued"),
        NodeState::TaskQueued
    );
    assert_eq!(
        NodeState::from_display_str("TASKQUEUED"),
        NodeState::TaskQueued
    );
    assert_eq!(NodeState::from_display_str("coding"), NodeState::Coding);
    assert_eq!(NodeState::from_display_str("STABLE"), NodeState::Completed);
    assert_eq!(NodeState::from_display_str("RUNNING"), NodeState::Coding);
    // Unknown strings map to TaskQueued (default)
    assert_eq!(
        NodeState::from_display_str("garbage"),
        NodeState::TaskQueued
    );
}

#[test]
fn test_node_state_display_roundtrip() {
    let states = [
        NodeState::TaskQueued,
        NodeState::Planning,
        NodeState::Coding,
        NodeState::Verifying,
        NodeState::Retry,
        NodeState::SheafCheck,
        NodeState::Committing,
        NodeState::Escalated,
        NodeState::Completed,
        NodeState::Failed,
    ];
    for state in &states {
        let display = state.to_string();
        let parsed = NodeState::from_display_str(&display);
        assert_eq!(parsed, *state, "Roundtrip failed for {:?}", state);
    }
}

#[test]
fn test_node_state_is_success() {
    assert!(NodeState::Completed.is_success());
    assert!(!NodeState::Escalated.is_success());
    assert!(!NodeState::Failed.is_success());
    assert!(!NodeState::Coding.is_success());
}

#[test]
fn test_node_state_is_active() {
    assert!(NodeState::Coding.is_active());
    assert!(NodeState::Verifying.is_active());
    assert!(NodeState::Planning.is_active());
    assert!(NodeState::Retry.is_active());
    assert!(NodeState::SheafCheck.is_active());
    assert!(NodeState::Committing.is_active());
    assert!(!NodeState::Completed.is_active());
    assert!(!NodeState::Escalated.is_active());
    assert!(!NodeState::TaskQueued.is_active());
}

#[test]
fn test_session_outcome_equality() {
    assert_eq!(SessionOutcome::Success, SessionOutcome::Success);
    assert_ne!(SessionOutcome::Success, SessionOutcome::PartialSuccess);
    assert_ne!(SessionOutcome::Success, SessionOutcome::Failed);
    assert_ne!(SessionOutcome::PartialSuccess, SessionOutcome::Failed);
}

// PSP-7 type tests

#[test]
fn test_parse_result_state_is_ok() {
    assert!(ParseResultState::StrictJsonOk.is_ok());
    assert!(ParseResultState::TolerantRecoveryOk.is_ok());
    assert!(!ParseResultState::NoStructuredPayload.is_ok());
    assert!(!ParseResultState::SchemaInvalid.is_ok());
    assert!(!ParseResultState::SemanticallyRejected.is_ok());
    assert!(!ParseResultState::EmptyBundle.is_ok());
}

#[test]
fn test_parse_result_state_display() {
    assert_eq!(ParseResultState::StrictJsonOk.to_string(), "strict_json_ok");
    assert_eq!(
        ParseResultState::NoStructuredPayload.to_string(),
        "no_structured_payload"
    );
    assert_eq!(
        ParseResultState::SemanticallyRejected.to_string(),
        "semantically_rejected"
    );
}

#[test]
fn test_retry_classification_display() {
    assert_eq!(
        RetryClassification::MalformedRetry.to_string(),
        "malformed_retry"
    );
    assert_eq!(RetryClassification::Retarget.to_string(), "retarget");
    assert_eq!(RetryClassification::Replan.to_string(), "replan");
    assert_eq!(
        RetryClassification::BudgetExhausted.to_string(),
        "budget_exhausted"
    );
}

#[test]
fn test_prompt_intent_serde_roundtrip() {
    let intents = [
        PromptIntent::ArchitectExisting,
        PromptIntent::ActuatorMultiOutput,
        PromptIntent::CorrectionRetry,
        PromptIntent::SoloGenerate,
    ];
    for intent in &intents {
        let json = serde_json::to_string(intent).unwrap();
        let back: PromptIntent = serde_json::from_str(&json).unwrap();
        assert_eq!(*intent, back);
    }
}

#[test]
fn test_task_plan_cycle_detection() {
    let mut a = PlannedTask::new("a", "goal a");
    a.dependencies = vec!["b".to_string()];
    let mut b = PlannedTask::new("b", "goal b");
    b.dependencies = vec!["c".to_string()];
    let mut c = PlannedTask::new("c", "goal c");
    c.dependencies = vec!["a".to_string()];
    let plan = TaskPlan {
        tasks: vec![a, b, c],
    };
    let err = plan.validate().unwrap_err();
    assert!(err.contains("cycle"), "Expected cycle error, got: {err}");
}

#[test]
fn test_task_plan_implicit_dependency_enforcement() {
    // Task B produces "src/lib.rs", Task A reads it but doesn't depend on B
    let mut a = PlannedTask::new("a", "use lib");
    a.context_files = vec!["src/lib.rs".to_string()];
    a.output_files = vec!["src/main.rs".to_string()];
    let mut b = PlannedTask::new("b", "create lib");
    b.output_files = vec!["src/lib.rs".to_string()];

    let mut plan = TaskPlan { tasks: vec![a, b] };
    let err = plan.validate().unwrap_err();
    assert!(
        err.contains("does not declare it as a dependency"),
        "Expected implicit dep error, got: {err}"
    );
    // Fix: add the dependency
    plan.tasks[0].dependencies.push("b".to_string());
    assert!(plan.validate().is_ok());
}

#[test]
fn test_task_plan_valid_acyclic() {
    let a = PlannedTask::new("a", "goal a");
    let mut b = PlannedTask::new("b", "goal b");
    b.dependencies = vec!["a".to_string()];
    let mut c = PlannedTask::new("c", "goal c");
    c.dependencies = vec!["a".to_string(), "b".to_string()];
    let plan = TaskPlan {
        tasks: vec![a, b, c],
    };
    assert!(plan.validate().is_ok());
}

#[test]
fn test_task_plan_test_file_dependency_inference() {
    // Source task produces src/lib.rs, test task produces tests/lib_test.rs
    // Test task should be required to depend on source task.
    let mut src = PlannedTask::new("src", "implement lib");
    src.output_files = vec!["src/lib.rs".to_string()];
    let mut tst = PlannedTask::new("tst", "test lib");
    tst.output_files = vec!["tests/lib_test.rs".to_string()];

    let plan = TaskPlan {
        tasks: vec![src, tst],
    };
    let err = plan.validate().unwrap_err();
    assert!(
        err.contains("Test task 'tst'") && err.contains("does not depend on source task 'src'"),
        "Expected test-dep inference error, got: {err}"
    );
}

#[test]
fn test_task_plan_test_file_dependency_satisfied() {
    let mut src = PlannedTask::new("src", "implement lib");
    src.output_files = vec!["src/lib.rs".to_string()];
    let mut tst = PlannedTask::new("tst", "test lib");
    tst.output_files = vec!["tests/lib_test.rs".to_string()];
    tst.dependencies = vec!["src".to_string()];

    let plan = TaskPlan {
        tasks: vec![src, tst],
    };
    assert!(plan.validate().is_ok());
}

#[test]
fn test_glob_matches_simple() {
    assert!(super::glob_matches_simple("tests/*.rs", "tests/foo.rs"));
    assert!(!super::glob_matches_simple("tests/*.rs", "src/foo.rs"));
    assert!(super::glob_matches_simple(
        "**/*.test.js",
        "src/utils.test.js"
    ));
    assert!(super::glob_matches_simple("test_*.py", "test_auth.py"));
    assert!(!super::glob_matches_simple("test_*.py", "auth.py"));
    assert!(super::glob_matches_simple(
        "tests/**/*.rs",
        "tests/unit/foo.rs"
    ));
}
