//! Search-plane type mechanism checks (PSP-10 Gates AC and AB
//! prerequisites, Phase 7).
//!
//! Budget reservations precede actions and usage is monotone; a closed
//! forest cannot reopen; the Proposition 5 selection order and the
//! six-key frontier order are total; witness chains reach the accepted
//! root or the branch is refused; every serde surface round-trips with
//! defaulted fields.

use perspt_sdk::search::ReservationRequest;
use perspt_sdk::{
    BranchMeasurement, PartialCheckpointRef, SearchBranch, SearchBranchState, SearchForest,
    SearchLimits, SearchStrategy, SearchUsage, WitnessRef,
};

fn limits() -> SearchLimits {
    SearchLimits {
        actions: 3,
        model_turns: 3,
        tool_calls: 6,
        mutations: 6,
        verifier_runs: 3,
        tokens: 10_000,
        elapsed_secs: 60,
        result_bytes: 10_000,
        workspace_files: 100,
        workspace_bytes: 10_000,
    }
}

fn branch(id: &str, witness: WitnessRef) -> SearchBranch {
    SearchBranch {
        branch_id: id.into(),
        parent_branch: None,
        accepted_ancestor: "root-a".into(),
        seed_checkpoint: witness.chain.first().cloned().unwrap_or_default(),
        seed_witness: witness,
        strategy: SearchStrategy {
            strategy_id: "default".into(),
            description: "primary route, default strategy".into(),
            route_preference: None,
        },
        route: perspt_sdk::ModelId::new("test", "scripted"),
        prompt_program: perspt_sdk::search::PromptProgramDigest("sha256:p".into()),
        state: SearchBranchState::Ready,
        usage: SearchUsage::default(),
    }
}

fn forest(branches: Vec<SearchBranch>) -> SearchForest {
    SearchForest {
        forest_id: "f1".into(),
        task_id: "t1".into(),
        node_id: "n1".into(),
        generation: 0,
        accepted_root: "root-a".into(),
        branches,
        limits: limits(),
        usage: SearchUsage::default(),
    }
}

/// Gate AC: reservations precede actions; refusal reserves nothing; usage
/// never decreases below consumption; a closed forest refuses everything.
#[test]
fn budget_reservations_are_monotone_and_closure_is_terminal() {
    let mut usage = SearchUsage::default();
    let ticket = usage
        .reserve(
            &limits(),
            ReservationRequest {
                workspace_files: 80,
                workspace_bytes: 8_000,
                ..Default::default()
            },
        )
        .unwrap();
    // A second fork over the cumulative workspace cap is refused whole.
    let before = usage.clone();
    assert!(usage
        .reserve(
            &limits(),
            ReservationRequest {
                workspace_files: 30,
                ..Default::default()
            },
        )
        .is_err());
    assert_eq!(usage, before);
    // Releasing returns only the unused part; the action unit stays.
    usage.release_unused(
        ticket,
        ReservationRequest {
            workspace_files: 60,
            workspace_bytes: 8_000,
            ..Default::default()
        },
    );
    assert_eq!(usage.workspace_files, 60);
    assert_eq!(usage.workspace_bytes, 8_000);
    assert_eq!(usage.actions, 1);
    usage.close();
    assert!(usage
        .reserve(&limits(), ReservationRequest::default())
        .is_err());
}

/// Witness chains: a root branch is trivially witnessed; a child inherits
/// through `extend`; a branch whose chain does not reach the forest root
/// fails validation (Definition 2).
#[test]
fn witness_chains_are_enforced_by_forest_validation() {
    let root_witness = WitnessRef::root("root-a");
    let child_witness = root_witness.extend("partial-1");
    let good = forest(vec![
        branch("b1", root_witness),
        branch("b2", child_witness.clone()),
    ]);
    good.validate().unwrap();

    let mut foreign = branch("b3", WitnessRef::root("root-b"));
    foreign.accepted_ancestor = "root-b".into();
    assert!(forest(vec![foreign]).validate().is_err());

    let broken = WitnessRef {
        accepted_root: "root-a".into(),
        chain: vec!["partial-9".into()],
    };
    assert!(forest(vec![branch("b4", broken)]).validate().is_err());
}

/// Partial checkpoints are private search state with a full parent chain.
#[test]
fn partial_checkpoints_serde_round_trip_with_defaults() {
    let checkpoint = PartialCheckpointRef {
        state_root: "partial-1".into(),
        accepted_ancestor: "root-a".into(),
        parent_witness: WitnessRef::root("root-a"),
        correction: None,
        remaining_obligations: Vec::new(),
        evidence_digest: "sha256:e".into(),
    };
    let json = serde_json::to_value(&checkpoint).unwrap();
    // Optional fields are omitted, and omitted fields deserialize.
    assert!(json.get("correction").is_none());
    let back: PartialCheckpointRef = serde_json::from_value(json).unwrap();
    assert_eq!(back, checkpoint);

    let state = SearchBranchState::PartialCheckpointed {
        checkpoint: checkpoint.clone(),
    };
    let round: SearchBranchState =
        serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
    assert_eq!(round, state);
}

/// The forest serde surface round-trips (resume folds depend on it).
#[test]
fn the_forest_round_trips() {
    let value = forest(vec![branch("b1", WitnessRef::root("root-a"))]);
    let round: SearchForest =
        serde_json::from_value(serde_json::to_value(&value).unwrap()).unwrap();
    assert_eq!(round, value);
    // Measurements embed in branch state.
    let measured = SearchBranchState::CandidateMeasured {
        measurement: BranchMeasurement {
            branch_id: "b1".into(),
            candidate_id: "n1/0/c1".into(),
            energy: 1.0,
            hard_pass: false,
            residuals: vec![],
            sensor_profile: "profile-1".into(),
            cost: 2.0,
        },
    };
    let round: SearchBranchState =
        serde_json::from_value(serde_json::to_value(&measured).unwrap()).unwrap();
    assert_eq!(round, measured);
}
