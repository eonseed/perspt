//! PSP-8 System 13 mechanism checks, end to end across SDK modules. Each test
//! corresponds to a falsifiable claim in the PSP. A failure here means the
//! implementation has drifted from the spec.

use perspt_sdk::*;

// --- SRBN crate integration + finite energy + energy equation ---------------

#[test]
fn energy_equation_is_weighted_sum_of_squares() {
    let model = EnergyModel::new("d", 0.5)
        .with_weight(ResidualWeight::new(
            ResidualClass::Type,
            EnergyComponent::Syn,
            2.0,
        ))
        .with_weight(ResidualWeight::new(
            ResidualClass::TestFailure,
            EnergyComponent::Log,
            1.0,
        ));
    let r = |class, score| {
        ResidualEvent::new(
            "n",
            0,
            class,
            ResidualSeverity::Error,
            score,
            SensorRef::new("c", IndependenceRoute::Compiler),
        )
        .unwrap()
    };
    // 2*3^2 + 1*2^2 = 22.
    let score = score_candidate(
        &model,
        &[
            r(ResidualClass::Type, 3.0),
            r(ResidualClass::TestFailure, 2.0),
        ],
    )
    .unwrap();
    assert_eq!(score.total, 22.0);
}

#[test]
fn negative_or_nonfinite_residuals_are_rejected() {
    assert!(ResidualEvent::new(
        "n",
        0,
        ResidualClass::Type,
        ResidualSeverity::Error,
        -1.0,
        SensorRef::new("c", IndependenceRoute::Compiler)
    )
    .is_err());
    assert!(ResidualEvent::new(
        "n",
        0,
        ResidualClass::Type,
        ResidualSeverity::Error,
        f64::INFINITY,
        SensorRef::new("c", IndependenceRoute::Compiler)
    )
    .is_err());
}

// --- Acceptance gate + finite-decision bound + residual certificate ----------

#[test]
fn gate_admits_hard_pass_and_descent_rejects_stall() {
    assert!(evaluate_gate(true, 50.0, 0.0, 0.5).unwrap().is_accepted());
    assert!(evaluate_gate(false, 9.0, 10.0, 0.5).unwrap().is_accepted()); // descent
    assert!(!evaluate_gate(false, 9.9, 10.0, 0.5).unwrap().is_accepted()); // stall
}

#[test]
fn finite_decision_bound_holds() {
    let mut traj = AcceptedTrajectory::new("n", 0, 10.0, 1.0, 3).unwrap();
    let bound = traj.decision_bound().unwrap(); // floor(10)+3+1 = 14
    let mut decisions = 0u64;
    let mut v: f64 = 10.0;
    while v > 0.0 {
        v = (v - 1.0).max(0.0);
        traj.submit(false, v).unwrap();
        decisions += 1;
    }
    assert!(decisions <= bound);
}

#[test]
fn exhaustion_yields_residual_certificate() {
    let residual = ResidualEvent::new(
        "n",
        3,
        ResidualClass::ImportGraph,
        ResidualSeverity::Error,
        1.0,
        SensorRef::new("rust-analyzer", IndependenceRoute::Lsp),
    )
    .unwrap();
    let cert = ResidualCertificate::from_residuals("n", 3, "head", 1.0, vec![residual]);
    assert_eq!(cert.verifier_routes, vec![IndependenceRoute::Lsp]);
    assert_eq!(cert.final_energy, 1.0);
}

// --- Stability-claim validation ----------------------------------------------

#[test]
fn stability_claim_computes_iss_floor_only_with_constants() {
    let mut claim = StabilityClaim::not_claimed("toy");
    assert_eq!(claim.resolve_floor().unwrap(), None); // NotClaimed
    claim.alpha = Some(2.0);
    claim.beta = Some(1.0);
    claim.delta = Some(1.0);
    claim.mu = Some(0.5);
    assert!((claim.resolve_floor().unwrap().unwrap() - 1.0).abs() < 1e-12);
}

// --- Admissibility witness + attenuation + shell confinement -----------------

#[test]
fn capability_confinement_blocks_unauthorized_write() {
    let actor = ActorId::new("explorer");
    let caps = vec![Capability::new(actor.clone(), vec![EffectKind::ReadFile])];
    let proposal = EffectProposal::new(actor, "n", EffectKind::WriteArtifact).with_path("src/x.rs");
    let w = check_admissibility(&proposal, &caps, &KernelState::new());
    assert!(matches!(w.decision, AdmissibilityDecision::Deny { .. }));
}

#[test]
fn shell_confinement_denies_sed_in_place_without_capability() {
    let actor = ActorId::new("impl");
    let mut cap = Capability::new(actor.clone(), vec![EffectKind::ReadFile]);
    cap.command_scope = vec![perspt_sdk::capability::CommandPattern("*".into())];
    let proposal = EffectProposal::new(actor, "n", EffectKind::ReadFile)
        .with_command(canonicalize("sed -i s/a/b/ f", "/r"));
    let w = check_admissibility(&proposal, &[cap], &KernelState::new());
    assert!(matches!(
        w.decision,
        AdmissibilityDecision::Deny {
            reason: DenyReason::MutationNotPermitted
        }
    ));
}

#[test]
fn attenuation_cannot_widen_authority() {
    let parent = Capability::new(ActorId::new("a"), vec![EffectKind::ReadFile]).delegable();
    let escalated = Capability::new(ActorId::new("sub"), vec![EffectKind::UpdatePolicy]);
    assert!(parent.delegate(escalated).is_none());
}

// --- Mutable graph: inserted node executes; conflicts serialize --------------

#[test]
fn static_graph_snapshot_bug_is_fixed() {
    let mut a = WorkNode::new("a", "first", NodeClass::Implement);
    a.state = WorkNodeState::Stable;
    let rev1 = WorkGraphRevision::build(0, None, GraphRevisionReason::InitialPlan, vec![a], vec![])
        .unwrap();
    // A repair inserts node "b".
    let mut nodes = rev1.nodes.clone();
    nodes.push(WorkNode::new("b", "inserted", NodeClass::Implement));
    let rev2 = WorkGraphRevision::build(
        1,
        Some(rev1.revision_id.clone()),
        GraphRevisionReason::LocalRepair,
        nodes,
        vec![],
    )
    .unwrap();
    let sched = Scheduler::new(4);
    let fp = |n: &WorkNode| Footprint::new().write(Resource::File(format!("{}.rs", n.node_id)));
    assert!(sched
        .ready_nodes(&rev2, fp)
        .iter()
        .any(|n| n.node_id == "b"));
}

#[test]
fn conflicting_manifest_mutations_cannot_run_together() {
    let nodes = vec![
        WorkNode::new("a", "x", NodeClass::Implement),
        WorkNode::new("b", "y", NodeClass::Implement),
    ];
    let rev =
        WorkGraphRevision::build(0, None, GraphRevisionReason::InitialPlan, nodes, vec![]).unwrap();
    let sched = Scheduler::new(4);
    let fp = |_n: &WorkNode| Footprint::new().write(Resource::Manifest("Cargo.toml".into()));
    assert_eq!(sched.ready_nodes(&rev, fp).len(), 1);
}

// --- Replay determinism + kernel-refusal -------------------------------------

#[test]
fn replay_reconstructs_accepted_trajectory_and_refuses_unrecorded() {
    let mut ledger = Ledger::new();
    ledger
        .append(LedgerEvent::CandidateAccepted {
            node_id: "a".into(),
            generation: 0,
            energy: 5.0,
        })
        .unwrap();
    ledger
        .append(LedgerEvent::CandidateAccepted {
            node_id: "b".into(),
            generation: 0,
            energy: 0.0,
        })
        .unwrap();
    assert!(ledger.verify_chain().is_ok());
    assert_eq!(replay_accepted_trajectory(&ledger).len(), 2);

    let commit = LedgerEvent::EffectApplied {
        proposal_id: "p".into(),
        idempotency_key: "k".into(),
    };
    assert!(ledger
        .commit_transition(commit.clone(), &["unrecorded".into()])
        .is_err());
    let handle = ledger.record_observation(b"data").unwrap();
    assert!(ledger.commit_transition(commit, &[handle]).is_ok());
}

// --- Spectral diagnostic + verifier independence -----------------------------

#[test]
fn spectral_mu_is_positive_for_connected_graph() {
    let g = VerificationGraph::new(3)
        .with_edge(0, 1, 1.0)
        .with_edge(1, 2, 1.0);
    assert!(g.mu(1e-9).unwrap().unwrap() > 0.0);
}

// --- Conformal calibration + graceful drift back-off -------------------------

#[test]
fn conformal_bound_asserted_when_calibrated_not_when_stale() {
    let samples = vec![
        CalibrationSample::new(0.1, true),
        CalibrationSample::new(0.2, true),
        CalibrationSample::new(0.8, false),
        CalibrationSample::new(0.9, false),
    ];
    let state = CalibrationState::calibrate(&samples, 0.3).unwrap();
    assert!(state.bound_is_asserted());
    let stale = state.mark_stale();
    assert!(!stale.bound_is_asserted());
    // Stale window does not hard-halt low-risk work.
    assert_eq!(
        conformal_decide(&stale, 0.99, RiskClass::Low),
        AcceptOutcome::UncertifiedAccept
    );
    assert_eq!(
        conformal_decide(&stale, 0.99, RiskClass::High),
        AcceptOutcome::RouteToApproval
    );
}

// --- Backlog gauge -----------------------------------------------------------

#[test]
fn backlog_gauge_aggregates_potential() {
    let workflows = vec![
        WorkflowPotential::new("w1", 10.0, 0.5, 3),
        WorkflowPotential::new("w2", 0.0, 0.5, 0),
    ];
    assert_eq!(backlog_gauge(&workflows), 24.0 + 1.0);
}

// --- Durable obligations: single-assignment + write-ahead --------------------

#[test]
fn idempotency_and_write_ahead_obligations_hold() {
    let mut log = IdempotencyLog::new();
    assert_eq!(log.record("k", b"content", "applied").unwrap(), "applied");
    assert_eq!(log.record("k", b"content", "ignored").unwrap(), "applied"); // redelivery
    assert!(log.record("k", b"different", "x").is_err()); // reuse for different content

    let mut effects = ExternalEffectLog::new();
    assert!(effects.result("k2").is_err()); // result before intent
    effects.intent("k2");
    assert!(effects.result("k2").is_ok());
}
