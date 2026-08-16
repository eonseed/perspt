use super::verification::verification_stages_for_node;
use super::*;
use std::path::PathBuf;

#[tokio::test]
async fn test_orchestrator_creation() {
    let orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    assert_eq!(orch.node_count(), 0);
}

#[tokio::test]
async fn test_add_nodes() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));

    let node1 = SRBNNode::new(
        "node1".to_string(),
        "Test task 1".to_string(),
        ModelTier::Architect,
    );
    let node2 = SRBNNode::new(
        "node2".to_string(),
        "Test task 2".to_string(),
        ModelTier::Actuator,
    );

    orch.add_node(node1);
    orch.add_node(node2);
    orch.add_dependency("node1", "node2", "depends_on").unwrap();

    assert_eq!(orch.node_count(), 2);
}
#[tokio::test]
async fn test_lsp_key_for_file_resolves_by_plugin() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    // Insert a dummy LSP client key so the lookup has something to match
    orch.lsp_clients.insert(
        "rust".to_string(),
        crate::lsp::LspClient::new("rust-analyzer"),
    );
    orch.lsp_clients
        .insert("python".to_string(), crate::lsp::LspClient::new("pylsp"));

    // Rust plugin owns .rs files
    assert_eq!(
        orch.lsp_key_for_file("src/main.rs"),
        Some("rust".to_string())
    );
    // Python plugin owns .py files
    assert_eq!(orch.lsp_key_for_file("app.py"), Some("python".to_string()));
    // Unknown extension falls back to first available client
    let key = orch.lsp_key_for_file("data.csv");
    assert!(key.is_some()); // Falls back to first available
}

// =========================================================================
// PSP-8: Goal-presence sensor (false-stability guard) tests
// =========================================================================

fn goal_presence_tmpdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("perspt-gp-{}-{}", tag, uuid::Uuid::new_v4()));
    std::fs::create_dir_all(dir.join("src")).unwrap();
    dir
}

#[tokio::test]
async fn goal_presence_raises_energy_for_unwritten_symbol() {
    // A placeholder output file compiles and has no diagnostics, but the
    // goal's required symbol is absent — the sensor must raise V_str so the
    // node is not declared falsely stable.
    let dir = goal_presence_tmpdir("missing");
    std::fs::write(dir.join("src/lib.rs"), "// implement here\n").unwrap();

    let mut orch = SRBNOrchestrator::new_for_testing(dir.clone());
    orch.context.defer_tests = true;
    let mut node = SRBNNode::new(
        "n".into(),
        "Add a public function `is_even(n: i32) -> bool` returning true for even n.".into(),
        ModelTier::Actuator,
    );
    node.output_targets = vec![PathBuf::from("src/lib.rs")];
    node.contract.interface_signature = "pub fn is_even(n: i32) -> bool".into();
    node.owner_plugin = "rust".into();
    orch.add_node(node);
    let idx = orch.node_indices["n"];

    let energy = orch.step_verify(idx).await.unwrap();
    // PSP-8 quadratic energy: a missing required symbol yields a SymbolMismatch
    // residual that rolls up into the structural component (V_str > 0), and the
    // total must exceed ε so the node cannot be declared falsely stable.
    assert!(
        energy.v_str > 0.0,
        "missing required symbol must raise structural energy (got {})",
        energy.v_str
    );
    assert!(
        energy.total() > orch.graph[idx].monitor.stability_epsilon,
        "energy must exceed epsilon so the node is not falsely stable"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn goal_presence_silent_when_symbol_present() {
    // Once the required symbol is actually defined, the sensor adds nothing.
    let dir = goal_presence_tmpdir("present");
    std::fs::write(
        dir.join("src/lib.rs"),
        "pub fn is_even(n: i32) -> bool { n % 2 == 0 }\n",
    )
    .unwrap();

    let mut orch = SRBNOrchestrator::new_for_testing(dir.clone());
    orch.context.defer_tests = true;
    let mut node = SRBNNode::new("n".into(), "Add `is_even`.".into(), ModelTier::Actuator);
    node.output_targets = vec![PathBuf::from("src/lib.rs")];
    node.contract.interface_signature = "pub fn is_even(n: i32) -> bool".into();
    node.owner_plugin = "rust".into();
    orch.add_node(node);
    let idx = orch.node_indices["n"];

    let energy = orch.step_verify(idx).await.unwrap();
    assert_eq!(energy.v_str, 0.0, "satisfied goal must not raise V_str");

    std::fs::remove_dir_all(&dir).ok();
}

// =========================================================================
// PSP-8: Closed-loop control (ready-queue scheduler + re-plan) tests
// =========================================================================

#[tokio::test]
async fn next_ready_node_respects_dependencies() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    orch.add_node(SRBNNode::new("a".into(), "g".into(), ModelTier::Actuator));
    orch.add_node(SRBNNode::new("b".into(), "g".into(), ModelTier::Actuator));
    orch.add_dependency("a", "b", "depends_on").unwrap();

    // Only 'a' (no deps) is ready while both are queued.
    let idx = orch.next_ready_node().unwrap();
    assert_eq!(orch.graph[idx].node_id, "a");

    // Completing 'a' makes 'b' ready.
    let a_idx = orch.node_indices["a"];
    orch.graph[a_idx].state = NodeState::Completed;
    let idx2 = orch.next_ready_node().unwrap();
    assert_eq!(orch.graph[idx2].node_id, "b");
}

#[tokio::test]
async fn reworked_retry_node_is_ready_again() {
    // A node set back to Retry by a repair must be re-picked by the loop.
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    orch.add_node(SRBNNode::new("a".into(), "g".into(), ModelTier::Actuator));
    let a = orch.node_indices["a"];
    orch.graph[a].state = NodeState::Retry;
    assert_eq!(orch.next_ready_node(), Some(a));

    // Completed/Escalated nodes are never ready.
    orch.graph[a].state = NodeState::Completed;
    assert_eq!(orch.next_ready_node(), None);
    orch.graph[a].state = NodeState::Escalated;
    assert_eq!(orch.next_ready_node(), None);
}

#[tokio::test]
async fn deterministic_goal_gate_requires_all_completed() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    orch.add_node(SRBNNode::new("a".into(), "g".into(), ModelTier::Actuator));
    assert!(!orch.deterministic_goal_gate()); // queued
    let a = orch.node_indices["a"];
    orch.graph[a].state = NodeState::Completed;
    assert!(orch.deterministic_goal_gate());

    // An empty graph is never "achieved".
    let empty = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    assert!(!empty.deterministic_goal_gate());
}

fn seed_impl_plan(orch: &mut SRBNOrchestrator) {
    use perspt_core::types::PlannedTask;
    let plan = TaskPlan {
        tasks: vec![PlannedTask {
            id: "impl".into(),
            goal: "implement".into(),
            output_files: vec!["src/lib.rs".into()],
            ..PlannedTask::new("impl", "implement")
        }],
    };
    orch.create_nodes_from_plan(&plan).unwrap();
}

#[tokio::test]
async fn merge_amendment_appends_valid_tasks_and_edges() {
    use perspt_core::types::PlannedTask;
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    seed_impl_plan(&mut orch);

    let amend = TaskPlan {
        tasks: vec![PlannedTask {
            id: "tests".into(),
            goal: "write tests".into(),
            output_files: vec!["tests/test_lib.rs".into()],
            dependencies: vec!["impl".into()],
            ..PlannedTask::new("tests", "write tests")
        }],
    };
    let added = orch.merge_plan_amendment(&amend).unwrap();
    assert_eq!(added, 1);
    assert!(orch.node_indices.contains_key("tests"));
    let impl_idx = orch.node_indices["impl"];
    let test_idx = orch.node_indices["tests"];
    assert!(orch.graph.find_edge(impl_idx, test_idx).is_some());
}

#[tokio::test]
async fn merge_amendment_rejects_ownership_collision() {
    use perspt_core::types::PlannedTask;
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    seed_impl_plan(&mut orch);
    let amend = TaskPlan {
        tasks: vec![PlannedTask {
            id: "extra".into(),
            goal: "g".into(),
            output_files: vec!["src/lib.rs".into()], // already owned by impl
            ..PlannedTask::new("extra", "g")
        }],
    };
    assert!(orch.merge_plan_amendment(&amend).is_err());
}

#[tokio::test]
async fn merge_amendment_rejects_duplicate_id_and_unknown_dep() {
    use perspt_core::types::PlannedTask;
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    seed_impl_plan(&mut orch);

    let dup = TaskPlan {
        tasks: vec![PlannedTask {
            id: "impl".into(), // duplicate of existing node
            goal: "g".into(),
            output_files: vec!["src/other.rs".into()],
            ..PlannedTask::new("impl", "g")
        }],
    };
    assert!(orch.merge_plan_amendment(&dup).is_err());

    let bad_dep = TaskPlan {
        tasks: vec![PlannedTask {
            id: "more".into(),
            goal: "g".into(),
            output_files: vec!["src/other.rs".into()],
            dependencies: vec!["nonexistent".into()],
            ..PlannedTask::new("more", "g")
        }],
    };
    assert!(orch.merge_plan_amendment(&bad_dep).is_err());
}

#[test]
fn interface_node_runs_only_syntax_so_build_penalty_is_gated() {
    // Regression for the phantom-build-failure bug: an Interface node must
    // not include the Build stage, so the stage-gated energy mapping never
    // charges it for `build_ok == false` (the Default) when Build never ran.
    use perspt_core::plugin::VerifierStage;
    let mut node = SRBNNode::new("scaffold".into(), "g".into(), ModelTier::Actuator);
    node.node_class = perspt_core::types::NodeClass::Interface;
    node.output_targets = vec![PathBuf::from("pyproject.toml")];
    let stages = super::verification::verification_stages_for_node(&node);
    assert_eq!(stages, vec![VerifierStage::SyntaxCheck]);
    assert!(!stages.contains(&VerifierStage::Build));
}

#[test]
fn parse_goal_verdict_is_tolerant() {
    let v =
        super::parse_goal_verdict("sure thing: {\"achieved\": true, \"missing\": []} ok").unwrap();
    assert!(v.achieved);
    let v2 = super::parse_goal_verdict(
        "{\"achieved\": false, \"missing\": [\"add tests\"], \"next_steps\": []}",
    )
    .unwrap();
    assert!(!v2.achieved);
    assert_eq!(v2.missing, vec!["add tests"]);
    assert!(super::parse_goal_verdict("no json here").is_none());
}

// =========================================================================
// Phase 5: Graph rewrite & sheaf validator tests
// =========================================================================

#[tokio::test]
async fn test_split_node_creates_children() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    let mut node = SRBNNode::new("parent".into(), "Do everything".into(), ModelTier::Actuator);
    node.output_targets = vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")];
    orch.add_node(node);

    let idx = orch.node_indices["parent"];
    let applied = orch.split_node(idx, &["handle a.rs".into(), "handle b.rs".into()]);
    assert!(!applied.is_empty());
    // Parent should be gone
    assert!(!orch.node_indices.contains_key("parent"));
    // Two children should exist
    assert!(orch.node_indices.contains_key("parent__split_0"));
    assert!(orch.node_indices.contains_key("parent__split_1"));
}

#[tokio::test]
async fn test_split_node_empty_children_is_noop() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    let node = SRBNNode::new("n".into(), "g".into(), ModelTier::Actuator);
    orch.add_node(node);
    let idx = orch.node_indices["n"];
    let applied = orch.split_node(idx, &[]);
    // Should not apply — return empty vec but not panic
    assert!(applied.is_empty());
}

#[tokio::test]
async fn test_insert_interface_node() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    let n1 = SRBNNode::new("a".into(), "source".into(), ModelTier::Actuator);
    let n2 = SRBNNode::new("b".into(), "dest".into(), ModelTier::Actuator);
    orch.add_node(n1);
    orch.add_node(n2);
    orch.add_dependency("a", "b", "data_flow").unwrap();

    let idx_a = orch.node_indices["a"];
    let applied = orch.insert_interface_node(idx_a, "API boundary");
    assert!(applied.is_some());
    assert!(orch.node_indices.contains_key("a__iface"));
    // Should now have 3 nodes
    assert_eq!(orch.node_count(), 3);
}

#[tokio::test]
async fn test_replan_subgraph_resets_nodes() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    let mut n1 = SRBNNode::new("trigger".into(), "g1".into(), ModelTier::Actuator);
    n1.state = NodeState::Coding;
    let mut n2 = SRBNNode::new("dep".into(), "g2".into(), ModelTier::Actuator);
    n2.state = NodeState::Completed;
    orch.add_node(n1);
    orch.add_node(n2);

    let trigger_idx = orch.node_indices["trigger"];
    let applied = orch.replan_subgraph(trigger_idx, &["dep".into()]);
    assert!(applied);

    let dep_idx = orch.node_indices["dep"];
    assert_eq!(orch.graph[dep_idx].state, NodeState::TaskQueued);
    assert_eq!(orch.graph[trigger_idx].state, NodeState::Retry);
}

#[tokio::test]
async fn test_select_validators_always_includes_dependency_graph() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    let node = SRBNNode::new("n".into(), "g".into(), ModelTier::Actuator);
    orch.add_node(node);
    let idx = orch.node_indices["n"];

    let validators = orch.select_validators(idx);
    assert!(validators.contains(&SheafValidatorClass::DependencyGraphConsistency));
}

#[tokio::test]
async fn test_select_validators_interface_node() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    let mut node = SRBNNode::new("iface".into(), "g".into(), ModelTier::Actuator);
    node.node_class = perspt_core::types::NodeClass::Interface;
    orch.add_node(node);
    let idx = orch.node_indices["iface"];

    let validators = orch.select_validators(idx);
    assert!(validators.contains(&SheafValidatorClass::ExportImportConsistency));
}

#[tokio::test]
async fn test_run_sheaf_validator_dependency_graph_no_cycles() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    let n1 = SRBNNode::new("a".into(), "g".into(), ModelTier::Actuator);
    let n2 = SRBNNode::new("b".into(), "g".into(), ModelTier::Actuator);
    orch.add_node(n1);
    orch.add_node(n2);
    orch.add_dependency("a", "b", "dep").unwrap();

    let idx = orch.node_indices["a"];
    let result = orch.run_sheaf_validator(idx, SheafValidatorClass::DependencyGraphConsistency);
    assert!(result.passed);
    assert_eq!(result.v_sheaf_contribution, 0.0);
}

#[tokio::test]
async fn test_classify_non_convergence_default() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    let node = SRBNNode::new("n".into(), "g".into(), ModelTier::Actuator);
    orch.add_node(node);
    let idx = orch.node_indices["n"];

    // With no verification results or policy failures, should default to ImplementationError
    let category = orch.classify_non_convergence(idx);
    assert_eq!(category, EscalationCategory::ImplementationError);
}

#[tokio::test]
async fn test_affected_dependents() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    let n1 = SRBNNode::new("root".into(), "g".into(), ModelTier::Actuator);
    let n2 = SRBNNode::new("child1".into(), "g".into(), ModelTier::Actuator);
    let n3 = SRBNNode::new("child2".into(), "g".into(), ModelTier::Actuator);
    orch.add_node(n1);
    orch.add_node(n2);
    orch.add_node(n3);
    orch.add_dependency("root", "child1", "dep").unwrap();
    orch.add_dependency("root", "child2", "dep").unwrap();

    let idx = orch.node_indices["root"];
    let deps = orch.affected_dependents(idx);
    assert_eq!(deps.len(), 2);
    assert!(deps.contains(&"child1".to_string()));
    assert!(deps.contains(&"child2".to_string()));
}

// =========================================================================
// PSP-5 Phase 6: Provisional Branch Tests
// =========================================================================

#[tokio::test]
async fn test_maybe_create_provisional_branch_root_node() {
    let temp_dir =
        std::env::temp_dir().join(format!("perspt_root_branch_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let mut orch = SRBNOrchestrator::new_for_testing(temp_dir.clone());
    orch.context.session_id = "test_session".into();
    let node = SRBNNode::new("root".into(), "root goal".into(), ModelTier::Actuator);
    orch.add_node(node);

    let idx = orch.node_indices["root"];
    // Root nodes now also get a provisional branch with sandbox
    let branch = orch.maybe_create_provisional_branch(idx);
    assert!(branch.is_some());
    assert!(orch.graph[idx].provisional_branch_id.is_some());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_maybe_create_provisional_branch_child_node() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_phase6"));
    orch.context.session_id = "test_session".into();
    let parent = SRBNNode::new("parent".into(), "parent goal".into(), ModelTier::Actuator);
    let child = SRBNNode::new("child".into(), "child goal".into(), ModelTier::Actuator);
    orch.add_node(parent);
    orch.add_node(child);
    orch.add_dependency("parent", "child", "dep").unwrap();

    let idx = orch.node_indices["child"];
    let branch = orch.maybe_create_provisional_branch(idx);
    assert!(branch.is_some());
    assert!(orch.graph[idx].provisional_branch_id.is_some());
}

#[tokio::test]
async fn test_collect_descendants() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    let n1 = SRBNNode::new("a".into(), "g".into(), ModelTier::Actuator);
    let n2 = SRBNNode::new("b".into(), "g".into(), ModelTier::Actuator);
    let n3 = SRBNNode::new("c".into(), "g".into(), ModelTier::Actuator);
    let n4 = SRBNNode::new("d".into(), "g".into(), ModelTier::Actuator);
    orch.add_node(n1);
    orch.add_node(n2);
    orch.add_node(n3);
    orch.add_node(n4);
    orch.add_dependency("a", "b", "dep").unwrap();
    orch.add_dependency("b", "c", "dep").unwrap();
    orch.add_dependency("a", "d", "dep").unwrap();

    let idx_a = orch.node_indices["a"];
    let descendants = orch.collect_descendants(idx_a);
    assert_eq!(descendants.len(), 3); // b, c, d
}

#[tokio::test]
async fn test_check_seal_prerequisites_no_interface_parent() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    let parent = SRBNNode::new("parent".into(), "g".into(), ModelTier::Actuator);
    let child = SRBNNode::new("child".into(), "g".into(), ModelTier::Actuator);
    orch.add_node(parent);
    orch.add_node(child);
    orch.add_dependency("parent", "child", "dep").unwrap();

    let idx = orch.node_indices["child"];
    // Parent is Implementation (default), not Interface — should not block
    assert!(!orch.check_seal_prerequisites(idx));
    assert!(orch.blocked_dependencies.is_empty());
}

#[tokio::test]
async fn test_check_seal_prerequisites_unsealed_interface() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    let mut parent = SRBNNode::new("iface".into(), "g".into(), ModelTier::Actuator);
    parent.node_class = perspt_core::types::NodeClass::Interface;
    let child = SRBNNode::new("impl".into(), "g".into(), ModelTier::Actuator);
    orch.add_node(parent);
    orch.add_node(child);
    orch.add_dependency("iface", "impl", "dep").unwrap();

    let idx = orch.node_indices["impl"];
    // Interface parent not sealed and not completed — should block
    assert!(orch.check_seal_prerequisites(idx));
    assert_eq!(orch.blocked_dependencies.len(), 1);
    assert_eq!(orch.blocked_dependencies[0].parent_node_id, "iface");
}

#[tokio::test]
async fn test_check_seal_prerequisites_sealed_interface() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    let mut parent = SRBNNode::new("iface".into(), "g".into(), ModelTier::Actuator);
    parent.node_class = perspt_core::types::NodeClass::Interface;
    parent.interface_seal_hash = Some([1u8; 32]); // Already sealed
    let child = SRBNNode::new("impl".into(), "g".into(), ModelTier::Actuator);
    orch.add_node(parent);
    orch.add_node(child);
    orch.add_dependency("iface", "impl", "dep").unwrap();

    let idx = orch.node_indices["impl"];
    // Interface parent is sealed — should not block
    assert!(!orch.check_seal_prerequisites(idx));
    assert!(orch.blocked_dependencies.is_empty());
}

#[tokio::test]
async fn test_unblock_dependents() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    let parent = SRBNNode::new("parent".into(), "g".into(), ModelTier::Actuator);
    let child = SRBNNode::new("child".into(), "g".into(), ModelTier::Actuator);
    orch.add_node(parent);
    orch.add_node(child);

    // Manually add a blocked dependency
    orch.blocked_dependencies
        .push(perspt_core::types::BlockedDependency::new(
            "child",
            "parent",
            vec!["src/api.rs".into()],
        ));
    assert_eq!(orch.blocked_dependencies.len(), 1);

    let idx = orch.node_indices["parent"];
    orch.unblock_dependents(idx);
    assert!(orch.blocked_dependencies.is_empty());
}

#[tokio::test]
async fn test_flush_descendant_branches() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_phase6_flush"));
    orch.context.session_id = "test_session".into();

    let parent = SRBNNode::new("parent".into(), "g".into(), ModelTier::Actuator);
    let mut child1 = SRBNNode::new("child1".into(), "g".into(), ModelTier::Actuator);
    child1.provisional_branch_id = Some("branch_c1".into());
    let mut child2 = SRBNNode::new("child2".into(), "g".into(), ModelTier::Actuator);
    child2.provisional_branch_id = Some("branch_c2".into());
    let grandchild = SRBNNode::new("grandchild".into(), "g".into(), ModelTier::Actuator);
    orch.add_node(parent);
    orch.add_node(child1);
    orch.add_node(child2);
    orch.add_node(grandchild);
    orch.add_dependency("parent", "child1", "dep").unwrap();
    orch.add_dependency("parent", "child2", "dep").unwrap();
    orch.add_dependency("child1", "grandchild", "dep").unwrap();

    let idx = orch.node_indices["parent"];
    // This will try to flush branches but ledger may not find them —
    // the important thing is it doesn't panic and traverses correctly
    orch.flush_descendant_branches(idx);
}

// =========================================================================
// PSP-5 Completion Tests
// =========================================================================

#[tokio::test]
async fn test_effective_working_dir_no_branch() {
    let orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/test/workspace"));
    // No nodes, but we can test the helper directly by adding one
    let mut orch = orch;
    let node = SRBNNode::new("n1".into(), "goal".into(), ModelTier::Actuator);
    orch.add_node(node);
    let idx = orch.node_indices["n1"];
    // No provisional branch → returns live workspace
    assert_eq!(
        orch.effective_working_dir(idx),
        PathBuf::from("/test/workspace")
    );
}

#[tokio::test]
async fn test_sandbox_dir_for_node_none_without_branch() {
    let orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/test/workspace"));
    let mut orch = orch;
    let node = SRBNNode::new("n1".into(), "goal".into(), ModelTier::Actuator);
    orch.add_node(node);
    let idx = orch.node_indices["n1"];
    assert!(orch.sandbox_dir_for_node(idx).is_none());
}

#[tokio::test]
async fn test_rewrite_churn_guardrail() {
    let orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_churn"));
    let mut orch = orch;
    let node = SRBNNode::new("node_a".into(), "goal".into(), ModelTier::Actuator);
    orch.add_node(node);
    // count_lineage_rewrites should return 0 for a fresh node
    let count = orch.count_lineage_rewrites("node_a");
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_run_resumed_skips_terminal_nodes() {
    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_resume"));

    let mut n1 = SRBNNode::new("done".into(), "completed".into(), ModelTier::Actuator);
    n1.state = NodeState::Completed;
    let mut n2 = SRBNNode::new("failed".into(), "failed".into(), ModelTier::Actuator);
    n2.state = NodeState::Failed;
    orch.add_node(n1);
    orch.add_node(n2);

    // Both nodes are terminal, so run_resumed should do nothing and succeed
    let result = orch.run_resumed().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_persist_review_decision_no_panic() {
    let orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_review"));
    // Should not panic even without a real ledger session —
    // it gracefully logs errors
    orch.persist_review_decision("node_x", "approved", None);
}

// =========================================================================
// PSP-5 Gap Tests
// =========================================================================

#[tokio::test]
async fn test_check_structural_dependencies_blocks_prose_only() {
    use perspt_core::types::{NodeClass, RestrictionMap};

    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_struct_dep"));

    // Parent: Interface node (no structural digests)
    let mut parent = SRBNNode::new("iface_1".into(), "Define API".into(), ModelTier::Architect);
    parent.node_class = NodeClass::Interface;

    // Child: Implementation node depending on the interface
    let mut child = SRBNNode::new("impl_1".into(), "Implement API".into(), ModelTier::Actuator);
    child.node_class = NodeClass::Implementation;

    let parent_idx = orch.add_node(parent);
    let child_idx = orch.add_node(child.clone());
    orch.graph
        .add_edge(parent_idx, child_idx, Dependency { kind: "dep".into() });

    // Empty restriction map — no structural digests at all
    let rmap = RestrictionMap::for_node("impl_1");
    let gaps = orch.check_structural_dependencies(&child, &rmap);

    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0].0, "iface_1");
    assert!(gaps[0].1.contains("no Signature/Schema/InterfaceSeal"));
}

#[tokio::test]
async fn test_check_structural_dependencies_passes_with_digest() {
    use perspt_core::types::{ArtifactKind, NodeClass, RestrictionMap, StructuralDigest};

    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_struct_ok"));

    let mut parent = SRBNNode::new("iface_2".into(), "Define API".into(), ModelTier::Architect);
    parent.node_class = NodeClass::Interface;

    let mut child = SRBNNode::new("impl_2".into(), "Implement API".into(), ModelTier::Actuator);
    child.node_class = NodeClass::Implementation;

    let parent_idx = orch.add_node(parent);
    let child_idx = orch.add_node(child.clone());
    orch.graph
        .add_edge(parent_idx, child_idx, Dependency { kind: "dep".into() });

    // Restriction map with a Signature digest from the Interface node
    let mut rmap = RestrictionMap::for_node("impl_2");
    rmap.structural_digests.push(StructuralDigest::from_content(
        "iface_2",
        "api.rs",
        ArtifactKind::Signature,
        b"fn do_thing(x: i32) -> bool;",
    ));

    let gaps = orch.check_structural_dependencies(&child, &rmap);
    assert!(gaps.is_empty(), "Expected no gaps when digest present");
}

#[tokio::test]
async fn test_check_structural_dependencies_skips_non_implementation() {
    use perspt_core::types::{NodeClass, RestrictionMap};

    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_struct_skip"));

    // An Integration node should NOT be checked
    let mut node = SRBNNode::new("integ_1".into(), "Wire modules".into(), ModelTier::Actuator);
    node.node_class = NodeClass::Integration;
    orch.add_node(node.clone());

    let rmap = RestrictionMap::for_node("integ_1");
    let gaps = orch.check_structural_dependencies(&node, &rmap);
    assert!(gaps.is_empty(), "Integration nodes should skip the check");
}

#[tokio::test]
async fn test_tier_default_models_are_differentiated() {
    // PSP-5 Fix D: each tier should map to a different default model
    let arch = ModelTier::Architect.default_model();
    let act = ModelTier::Actuator.default_model();
    let spec = ModelTier::Speculator.default_model();

    // Architect and Actuator should NOT be the same tier default
    assert_ne!(arch, act, "Architect and Actuator defaults should differ");
    // Speculator should be the lightest
    assert_ne!(spec, arch, "Speculator should differ from Architect");
}

// =========================================================================
// PSP-5: Tier Wiring and Plan Validation Tests
// =========================================================================

#[tokio::test]
async fn test_orchestrator_stores_all_four_tier_models() {
    let orch = SRBNOrchestrator::new_with_models(
        PathBuf::from("/tmp/test_tiers"),
        false,
        Some("arch-model".into()),
        Some("act-model".into()),
        Some("ver-model".into()),
        Some("spec-model".into()),
        None,
        None,
        None,
        None,
    );
    assert_eq!(orch.architect_model, "arch-model");
    assert_eq!(orch.actuator_model, "act-model");
    assert_eq!(orch.verifier_model, "ver-model");
    assert_eq!(orch.speculator_model, "spec-model");
}

#[tokio::test]
async fn test_orchestrator_default_tier_models() {
    let orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_tier_defaults"));
    assert_eq!(orch.architect_model, ModelTier::Architect.default_model());
    assert_eq!(orch.actuator_model, ModelTier::Actuator.default_model());
    assert_eq!(orch.verifier_model, ModelTier::Verifier.default_model());
    assert_eq!(orch.speculator_model, ModelTier::Speculator.default_model());
}

#[tokio::test]
async fn test_deterministic_fallback_graph_is_valid_plan() {
    // Regression: the fallback plan previously had `scaffold` and `implement`
    // both claiming the main module file, violating ownership exclusivity and
    // hard-failing every greenfield run that fell back to it. Each task must
    // own a distinct file (manifest / main module / tests).
    for task in [
        "build a python RPN calculator library",
        "build a rust command-line tool",
        "build a javascript utility package",
    ] {
        let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
        let result = orch.create_deterministic_fallback_graph(task);
        assert!(
            result.is_ok(),
            "fallback graph for {task:?} must be a valid plan, got {result:?}"
        );
        assert_eq!(orch.node_count(), 3, "fallback plan should have 3 nodes");
    }
}

#[tokio::test]
async fn test_create_nodes_rejects_duplicate_output_files() {
    use perspt_core::types::PlannedTask;

    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_dup_outputs"));

    let plan = TaskPlan {
        tasks: vec![
            PlannedTask {
                id: "task_1".into(),
                goal: "Create math".into(),
                output_files: vec!["src/math.py".into(), "tests/test_math.py".into()],
                ..PlannedTask::new("task_1", "Create math")
            },
            PlannedTask {
                id: "task_2".into(),
                goal: "Create tests".into(),
                output_files: vec!["tests/test_math.py".into()],
                ..PlannedTask::new("task_2", "Create tests")
            },
        ],
    };

    let result = orch.create_nodes_from_plan(&plan);
    assert!(result.is_err(), "Should reject duplicate output_files");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("tests/test_math.py"),
        "Error should mention the duplicate file: {}",
        err
    );
}

#[tokio::test]
async fn test_create_nodes_accepts_unique_output_files() {
    use perspt_core::types::PlannedTask;

    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_unique_outputs"));

    let plan = TaskPlan {
        tasks: vec![
            PlannedTask {
                id: "task_1".into(),
                goal: "Create math".into(),
                output_files: vec!["src/math.py".into()],
                ..PlannedTask::new("task_1", "Create math")
            },
            PlannedTask {
                id: "test_1".into(),
                goal: "Test math".into(),
                output_files: vec!["tests/test_math.py".into()],
                dependencies: vec!["task_1".into()],
                ..PlannedTask::new("test_1", "Test math")
            },
        ],
    };

    let result = orch.create_nodes_from_plan(&plan);
    assert!(result.is_ok(), "Should accept unique output_files");
    assert_eq!(orch.graph.node_count(), 2);
}

#[tokio::test]
async fn test_ownership_manifest_built_with_majority_plugin_vote() {
    use perspt_core::types::PlannedTask;

    let mut orch = SRBNOrchestrator::new_for_testing(PathBuf::from("/tmp/test_plugin_vote"));

    let plan = TaskPlan {
        tasks: vec![PlannedTask {
            id: "task_1".into(),
            goal: "Create Python module".into(),
            output_files: vec![
                "src/main.py".into(),
                "src/helper.py".into(),
                "src/__init__.py".into(),
            ],
            ..PlannedTask::new("task_1", "Create Python module")
        }],
    };

    orch.create_nodes_from_plan(&plan).unwrap();

    // All three files should be in the manifest
    assert_eq!(orch.context.ownership_manifest.len(), 3);
    // The node should have the python plugin assigned
    let idx = orch.node_indices["task_1"];
    assert_eq!(orch.graph[idx].owner_plugin, "python");
}

#[tokio::test]
async fn test_apply_bundle_strips_paths_outside_node_output_targets() {
    use perspt_core::types::{ArtifactBundle, ArtifactOperation, PlannedTask};

    let temp_dir = std::env::temp_dir().join(format!(
        "perspt_bundle_target_guard_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(temp_dir.join("src")).unwrap();

    let mut orch = SRBNOrchestrator::new_for_testing(temp_dir.clone());
    let plan = TaskPlan {
        tasks: vec![
            PlannedTask {
                id: "validate_module".into(),
                goal: "Create validation module".into(),
                output_files: vec!["src/validate.rs".into()],
                ..PlannedTask::new("validate_module", "Create validation module")
            },
            PlannedTask {
                id: "lib_module".into(),
                goal: "Export validation module".into(),
                output_files: vec!["src/lib.rs".into()],
                dependencies: vec!["validate_module".into()],
                ..PlannedTask::new("lib_module", "Export validation module")
            },
        ],
    };

    orch.create_nodes_from_plan(&plan).unwrap();

    let bundle = ArtifactBundle {
        artifacts: vec![
            ArtifactOperation::Write {
                path: "src/validate.rs".into(),
                content: "pub fn ok() {}".into(),
            },
            ArtifactOperation::Write {
                path: "src/lib.rs".into(),
                content: "pub mod validate;".into(),
            },
        ],
        commands: vec![],
    };

    // Should succeed — the undeclared path src/lib.rs is stripped, but
    // src/validate.rs is applied.
    orch.apply_bundle_transactionally(
        &bundle,
        "validate_module",
        perspt_core::types::NodeClass::Implementation,
    )
    .await
    .expect("Should apply valid artifacts after stripping undeclared paths");

    // The declared file should be written
    assert!(temp_dir.join("src/validate.rs").exists());
    // The undeclared file should NOT be written
    assert!(!temp_dir.join("src/lib.rs").exists());
}

#[tokio::test]
async fn test_apply_bundle_keeps_legal_support_file() {
    use perspt_core::types::{ArtifactBundle, ArtifactOperation, PlannedTask};

    let temp_dir = std::env::temp_dir().join(format!(
        "perspt_bundle_support_file_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(temp_dir.join("src")).unwrap();

    let mut orch = SRBNOrchestrator::new_for_testing(temp_dir.clone());
    let plan = TaskPlan {
        tasks: vec![PlannedTask {
            id: "main_module".into(),
            goal: "Create Rust main".into(),
            output_files: vec!["src/main.rs".into()],
            ..PlannedTask::new("main_module", "Create Rust main")
        }],
    };
    orch.create_nodes_from_plan(&plan).unwrap();

    let bundle = ArtifactBundle {
        artifacts: vec![
            ArtifactOperation::Write {
                path: "src/main.rs".into(),
                content: "fn main() {}".into(),
            },
            ArtifactOperation::Write {
                path: "build.rs".into(),
                content: "fn main() {}".into(),
            },
        ],
        commands: vec![],
    };

    orch.apply_bundle_transactionally(
        &bundle,
        "main_module",
        perspt_core::types::NodeClass::Implementation,
    )
    .await
    .expect("legal support files should survive semantic filtering");

    assert!(temp_dir.join("src/main.rs").exists());
    assert!(temp_dir.join("build.rs").exists());
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_apply_bundle_denies_root_manifest_mutation() {
    use perspt_core::types::{ArtifactBundle, ArtifactOperation, PlannedTask};

    let temp_dir = std::env::temp_dir().join(format!(
        "perspt_bundle_manifest_policy_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(temp_dir.join("src")).unwrap();

    let mut orch = SRBNOrchestrator::new_for_testing(temp_dir.clone());
    let plan = TaskPlan {
        tasks: vec![PlannedTask {
            id: "main_module".into(),
            goal: "Create Rust main".into(),
            output_files: vec!["src/main.rs".into()],
            ..PlannedTask::new("main_module", "Create Rust main")
        }],
    };
    orch.create_nodes_from_plan(&plan).unwrap();

    let bundle = ArtifactBundle {
        artifacts: vec![
            ArtifactOperation::Write {
                path: "src/main.rs".into(),
                content: "fn main() {}".into(),
            },
            ArtifactOperation::Write {
                path: "Cargo.toml".into(),
                content: "[package]\nname = \"bad\"\n".into(),
            },
        ],
        commands: vec![],
    };

    orch.apply_bundle_transactionally(
        &bundle,
        "main_module",
        perspt_core::types::NodeClass::Implementation,
    )
    .await
    .expect("declared artifact should still apply after denied manifest is stripped");

    assert!(temp_dir.join("src/main.rs").exists());
    assert!(!temp_dir.join("Cargo.toml").exists());
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_apply_bundle_writes_into_branch_sandbox() {
    use perspt_core::types::{ArtifactBundle, ArtifactOperation, PlannedTask};

    let temp_dir = std::env::temp_dir().join(format!(
        "perspt_branch_sandbox_write_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(temp_dir.join("src")).unwrap();
    std::fs::write(temp_dir.join("src/lib.rs"), "pub fn old() {}\n").unwrap();

    let mut orch = SRBNOrchestrator::new_for_testing(temp_dir.clone());
    orch.context.session_id = uuid::Uuid::new_v4().to_string();

    let plan = TaskPlan {
        tasks: vec![
            PlannedTask {
                id: "parent".into(),
                goal: "Parent node".into(),
                output_files: vec!["src/lib.rs".into()],
                ..PlannedTask::new("parent", "Parent node")
            },
            PlannedTask {
                id: "child".into(),
                goal: "Child node".into(),
                context_files: vec!["src/lib.rs".into()],
                output_files: vec!["src/child.rs".into()],
                dependencies: vec!["parent".into()],
                ..PlannedTask::new("child", "Child node")
            },
        ],
    };

    orch.create_nodes_from_plan(&plan).unwrap();
    let child_idx = orch.node_indices["child"];
    let branch_id = orch.maybe_create_provisional_branch(child_idx).unwrap();
    let sandbox_dir = orch.sandbox_dir_for_node(child_idx).unwrap();

    let bundle = ArtifactBundle {
        artifacts: vec![ArtifactOperation::Write {
            path: "src/child.rs".into(),
            content: "pub fn child() {}\n".into(),
        }],
        commands: vec![],
    };

    orch.apply_bundle_transactionally(
        &bundle,
        "child",
        perspt_core::types::NodeClass::Implementation,
    )
    .await
    .unwrap();

    assert!(sandbox_dir.join("src/child.rs").exists());
    assert!(!temp_dir.join("src/child.rs").exists());

    orch.merge_provisional_branch(&branch_id, child_idx);
}

#[test]
fn test_verification_stages_for_node_classes() {
    use perspt_core::plugin::VerifierStage;

    // Interface → SyntaxCheck only
    let interface_node = SRBNNode::new("iface".into(), "Define trait".into(), ModelTier::Actuator);
    // Default is Implementation, so override:
    let mut interface_node = interface_node;
    interface_node.node_class = perspt_core::types::NodeClass::Interface;
    let stages = verification_stages_for_node(&interface_node);
    assert_eq!(stages, vec![VerifierStage::SyntaxCheck]);

    // Implementation without tests → SyntaxCheck + Build
    let mut implementation_node = SRBNNode::new(
        "impl".into(),
        "Implement feature".into(),
        ModelTier::Actuator,
    );
    implementation_node.node_class = perspt_core::types::NodeClass::Implementation;
    let stages = verification_stages_for_node(&implementation_node);
    assert_eq!(
        stages,
        vec![VerifierStage::SyntaxCheck, VerifierStage::Build]
    );

    // Implementation with weighted tests → SyntaxCheck + Build + Test
    implementation_node
        .contract
        .weighted_tests
        .push(perspt_core::types::WeightedTest {
            test_name: "test_feature".into(),
            criticality: perspt_core::types::Criticality::High,
        });
    let stages = verification_stages_for_node(&implementation_node);
    assert_eq!(
        stages,
        vec![
            VerifierStage::SyntaxCheck,
            VerifierStage::Build,
            VerifierStage::Test
        ]
    );

    // Integration → full pipeline
    let mut integration_node =
        SRBNNode::new("test".into(), "Verify feature".into(), ModelTier::Actuator);
    integration_node.node_class = perspt_core::types::NodeClass::Integration;
    integration_node
        .contract
        .weighted_tests
        .push(perspt_core::types::WeightedTest {
            test_name: "test_feature".into(),
            criticality: perspt_core::types::Criticality::High,
        });
    let stages = verification_stages_for_node(&integration_node);
    assert_eq!(
        stages,
        vec![
            VerifierStage::SyntaxCheck,
            VerifierStage::Build,
            VerifierStage::Test,
            VerifierStage::Lint,
        ]
    );
}
