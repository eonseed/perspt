//! The tool loop's ports and durable event vocabulary: executors,
//! measurers, recorded events, and the rolling event log.

use super::*;

/// One exported file of an accepted candidate. `content` is the accepted
/// overlay state (`None` = deleted); `source_preimage` is the workspace state
/// from which the first mutation was derived. Resume must preserve both so a
/// later promotion cannot overwrite user edits made while the session slept.
#[derive(Debug, Clone)]
pub struct SeedFile {
    pub path: String,
    pub content: Option<Vec<u8>>,
    pub source_preimage: Option<Vec<u8>>,
}

/// Content-addressed form persisted in a durable candidate checkpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DurableSeedFile {
    pub path: String,
    pub content_artifact: Option<String>,
    pub source_preimage_artifact: Option<String>,
}

/// What applying one admitted call did to the candidate overlay.
#[derive(Debug, Clone)]
pub struct EffectOutcome {
    /// Tool output returned to the model as the tool response.
    pub output: String,
    /// Whether the overlay was mutated.
    pub mutated: bool,
}

/// Opaque executor checkpoint plus its measured state witness.
#[derive(Debug, Clone)]
pub struct CandidateCheckpoint {
    pub id: String,
    pub witness: CandidateStateWitness,
}

/// Applies admitted calls to the candidate overlay (the existing sandboxed
/// executors are the first driver).
#[async_trait::async_trait]
pub trait EffectExecutor: Send + Sync {
    /// Snapshot the reversible candidate overlay.
    async fn checkpoint(&self, scope: &[String]) -> Result<CandidateCheckpoint>;

    async fn apply(&self, call: &ProviderToolCall, entry: &ToolEntry) -> Result<EffectOutcome>;

    /// Restore an earlier candidate snapshot exactly.
    async fn restore(&self, checkpoint: &CandidateCheckpoint) -> Result<()>;

    /// Re-read the materialized candidate after an effect.
    async fn state_witness(&self) -> Result<CandidateStateWitness>;

    /// The accepted candidate's mutated paths with their current contents
    /// (`None` = deleted), for durable mid-loop checkpoints. The default is
    /// empty: fixtures without a filesystem have nothing durable to export.
    async fn export_accepted(&self) -> Result<Vec<SeedFile>> {
        Ok(Vec::new())
    }
}

/// One measured evaluation of the realized candidate.
#[derive(Debug, Clone, Default)]
pub struct Measured {
    pub hard_pass: bool,
    pub energy: f64,
    pub residuals: Vec<ResidualEvent>,
    /// The domain's directed correction for the dominant residual.
    pub correction: Option<CorrectionDirection>,
    /// The typed correction packet folded from all residuals (PSP-10
    /// system 26). `None` on domains that have not opted in; an empty
    /// packet never causes a blind retry.
    pub packet: Option<perspt_sdk::CorrectionPacket>,
}

/// Re-reads the candidate overlay and runs the declared verifier suite
/// against it (Paper I Def. 12.2: measurement runs on the realized state).
#[async_trait::async_trait]
pub trait CandidateMeasurer: Send + Sync {
    async fn measure(&self) -> Result<Measured>;

    /// Cheapest realized-state boundary available after one mutation. The
    /// default is the complete suite, which is conservative; domain drivers
    /// may provide an incremental parser without weakening the gate suite.
    async fn measure_incremental(&self) -> Result<Measured> {
        self.measure().await
    }
}

/// The compiled prompt binding for one actor (PSP-10 systems 23–24). The
/// runtime compiles it through the SDK compiler for the resolved route and
/// dialect; the loop seeds the text, stamps the digests into every control
/// frame, and — holding the stage — recompiles whenever the offered tool
/// surface or the failover route changes mid-loop, so each call's exact
/// program is ledgered.
#[derive(Debug, Clone, Default)]
pub struct PromptEnvelope {
    pub text: String,
    pub invocation_digest: String,
    pub manifest_digest: String,
    /// The platform stage sections, kept for per-call recompilation.
    pub stage: Option<perspt_core::prompts::PlatformStage>,
    /// The domain's same-stage sections, rendered once at assembly.
    pub domain_sections: Vec<perspt_sdk::prompt::RenderedSection>,
    /// The invocation compiled at seed time (initial tool surface).
    pub invocation: Option<perspt_sdk::prompt::CompiledPromptInvocation>,
}

/// Recorded loop events (the ledger consumes these in system 14).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum LoopEvent {
    /// The fold base for the model context (Gate O). Additive: old ledgers
    /// without seeds still replay.
    ConversationSeeded {
        seed: perspt_sdk::ConversationSeeded,
    },
    /// One digest-chained model-context change, recorded before it is
    /// applied to the live projection.
    ConversationDelta {
        record: perspt_sdk::ConversationDeltaRecord,
    },
    TurnObserved {
        turn: u32,
        /// Which actor observed this output (PSP-10 system 27). Empty on
        /// pre-PSP-10 rows; the observation is always recorded before any
        /// parse.
        #[serde(default)]
        actor: String,
        output: TurnOutput,
    },
    ToolCallObserved {
        call: ProviderToolCall,
    },
    ProposalObserved {
        call_id: String,
        proposal: EffectProposal,
    },
    ProposalChecked {
        call_id: String,
        proposal: EffectProposal,
        witness: Box<FullAdmissibilityWitness>,
    },
    EffectApplied {
        call_id: String,
        mutated: bool,
        output: String,
    },
    /// A same-turn footprint collision (Gate P): the call was returned to
    /// the model as an observation, never given an invented order.
    ToolBatchConflict {
        call_id: String,
        conflicts_with: String,
        resources: Vec<String>,
    },
    EffectDenied {
        call_id: String,
        reason: String,
        /// Typed admissibility residual (PSP-9 system 9): `CapabilityDenied`
        /// for kernel denials, `BudgetExhausted` for budget boundaries.
        #[serde(default = "default_denial_class")]
        class: perspt_sdk::ResidualClass,
    },
    CandidateMeasured {
        node_id: String,
        generation: u32,
        /// Candidate identity `"{node}/{gen}/c{seq}"` (PSP-10 Phase 2). The
        /// accepted fold keys by it; empty on pre-PSP-10 rows.
        #[serde(default)]
        candidate_id: String,
        energy: f64,
        hard_pass: bool,
        residuals: Vec<ResidualEvent>,
    },
    GateDecisionRecorded {
        node_id: String,
        generation: u32,
        /// Shares the measurement's candidate identity so the two events
        /// key together (Proposition 2). Empty on pre-PSP-10 rows.
        #[serde(default)]
        candidate_id: String,
        decision: GateDecision,
        /// Recorded from the trajectory's `GateDecisionRef`, never recovered
        /// by correlation. `None` marks a pre-PSP-10 row.
        #[serde(default)]
        observed_energy: Option<f64>,
        #[serde(default)]
        best_accepted_before: Option<f64>,
    },
    /// The loop refused to submit a candidate past the finite-decision bound
    /// `N_gate = floor(V0/rho) + B + 1` (PSP-10 Gate X). The refusal itself
    /// appends no gate decision.
    DecisionBoundRefused {
        node_id: String,
        generation: u32,
        bound: u64,
        decisions_used: u64,
    },
    CandidateRestored {
        checkpoint_id: String,
    },
    EffectBoundaryMeasured {
        call_id: String,
        node_id: String,
        generation: u32,
        energy: f64,
        hard_pass: bool,
        residuals: Vec<ResidualEvent>,
    },
    ContextCheckpointCreated {
        checkpoint: ContextCheckpoint,
    },
    /// A gate acceptance made durable: the control frame plus every mutated
    /// path's content-addressed artifact handle, sufficient to rebuild the
    /// accepted candidate and continue the loop after a crash.
    DurableCandidateCheckpoint {
        /// The accepted candidate this checkpoint makes durable (PSP-10
        /// Phase 2). Empty on pre-PSP-10 rows.
        #[serde(default)]
        candidate_id: String,
        state_root: String,
        control: ControlFrame,
        /// Exact provider-neutral projection selected for the next model turn.
        conversation: Conversation,
        /// Exact scope used to compute `state_root`, including read paths.
        canonical_scope: Vec<String>,
        files: Vec<DurableSeedFile>,
    },
    RecoveryControlGranted {
        failure: FailureKind,
        level: perspt_sdk::CascadeLevel,
        forced_escalation: bool,
        model: ModelId,
    },
    RouteFailover {
        from_model: ModelId,
        to_model: ModelId,
        cause: String,
    },
    RecoveryContained {
        reason: String,
        restored_checkpoint_id: String,
    },
    // --- PSP-10 search alphabet (system 21). Emitted by the forest
    // runtime; every event carries forest_id and branch_id where one
    // exists. Defined with the envelope so the wire shape is pinned before
    // emission begins. ---
    SearchOpened {
        forest_id: String,
        node_id: String,
        generation: u32,
        accepted_root: String,
        limits: perspt_sdk::SearchLimits,
    },
    BranchForked {
        forest_id: String,
        branch_id: String,
        #[serde(default)]
        parent_branch: Option<String>,
        seed_checkpoint: String,
        seed_witness: perspt_sdk::WitnessRef,
    },
    BranchStrategySelected {
        forest_id: String,
        branch_id: String,
        strategy_id: String,
    },
    BranchObservation {
        forest_id: String,
        branch_id: String,
        observation: String,
    },
    BranchCandidateMeasured {
        forest_id: String,
        branch_id: String,
        candidate_id: String,
        measurement: perspt_sdk::BranchMeasurement,
    },
    PartialCheckpointed {
        forest_id: String,
        branch_id: String,
        checkpoint: perspt_sdk::PartialCheckpointRef,
    },
    FrontierEpochStarted {
        forest_id: String,
        epoch: u64,
        /// Digest of the folded forest state; strict resume recomputes and
        /// compares it.
        forest_digest: String,
    },
    FrontierEntryServed {
        forest_id: String,
        branch_id: String,
        epoch: u64,
    },
    BranchIneligible {
        forest_id: String,
        branch_id: String,
        reason: String,
    },
    BranchNotSelected {
        forest_id: String,
        branch_id: String,
    },
    BranchAbandoned {
        forest_id: String,
        branch_id: String,
        reason: String,
    },
    BranchSelected {
        forest_id: String,
        branch_id: String,
        candidate_id: String,
    },
    BranchCommitted {
        forest_id: String,
        branch_id: String,
        candidate_id: String,
        decision: GateDecision,
    },
    NoGoodRecorded {
        forest_id: String,
        branch_id: String,
        /// The exact key `K_ng` (Gate AB); support evidence hashed
        /// separately.
        key: String,
        evidence_hash: String,
    },
    SearchClosed {
        forest_id: String,
        usage: perspt_sdk::SearchUsage,
    },
    // --- PSP-10 resident-context alphabet (Definition 6, Gate AF). ---
    ContextWorkingSet {
        #[serde(default)]
        forest_id: String,
        #[serde(default)]
        branch_id: String,
        turn: u32,
        page_ids: Vec<String>,
    },
    ContextPagesSelected {
        #[serde(default)]
        forest_id: String,
        #[serde(default)]
        branch_id: String,
        turn: u32,
        resident_digest: String,
        page_ids: Vec<String>,
    },
    ContextMiss {
        #[serde(default)]
        forest_id: String,
        #[serde(default)]
        branch_id: String,
        turn: u32,
        key: String,
    },
    ContextPageRecalled {
        #[serde(default)]
        forest_id: String,
        #[serde(default)]
        branch_id: String,
        turn: u32,
        page_id: String,
    },
    ContextInfeasible {
        #[serde(default)]
        forest_id: String,
        #[serde(default)]
        branch_id: String,
        turn: u32,
        required: u64,
        allowance: u64,
    },
    ContextCompacted {
        #[serde(default)]
        forest_id: String,
        #[serde(default)]
        branch_id: String,
        summary_page: String,
        source_pages: Vec<String>,
    },
    /// A prompt program recompiled mid-loop — the offered tool surface or
    /// the failover route changed, so the program identity changed
    /// (PSP-10 Gate Z).
    PromptProgramCompiled {
        turn: u32,
        program: perspt_sdk::prompt::CompiledPromptProgram,
    },
    /// The exact prompt binding of one model call: both program digests
    /// and the invocation digest (one record per call, Gate Z).
    PromptProgramInvoked {
        turn: u32,
        invocation_digest: String,
        platform_digest: String,
        domain_digest: String,
        tool_spec_hash: String,
        #[serde(default)]
        resident_context_digest: String,
    },
}

/// The versioned runtime-event envelope (PSP-10 system 21, Gate AD). Every
/// post-cutover `tool_loop` row wraps its event in exactly this shape;
/// rows without `schema_version` decode through the strict legacy decoder;
/// unknown versions fail authoritative replay and resume closed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopEventEnvelopeV1 {
    /// Exactly 1.
    pub schema_version: u16,
    pub body: LoopEvent,
}

impl LoopEventEnvelopeV1 {
    pub fn new(body: LoopEvent) -> Self {
        Self {
            schema_version: 1,
            body,
        }
    }
}

fn default_denial_class() -> perspt_sdk::ResidualClass {
    perspt_sdk::ResidualClass::CapabilityDenied
}

/// In-loop event log. With a durable recorder attached every event is already
/// persisted before use, so only a rolling count and chain root are kept in
/// memory; without one (conformance fixtures) the events are retained so
/// tests can inspect them. This keeps long runs O(1) in event memory and
/// makes each compaction O(1) instead of re-serializing the whole history.
#[derive(Debug, Default)]
pub struct EventLog {
    retained: Vec<LoopEvent>,
    retain: bool,
    count: u64,
    chain_root: String,
}

impl EventLog {
    pub(super) fn new(retain: bool) -> Self {
        Self {
            retain,
            ..Self::default()
        }
    }

    pub(super) fn push(&mut self, event: &LoopEvent) -> Result<()> {
        let hashed = perspt_sdk::ledger::content_hash(&serde_json::to_vec(event)?);
        self.chain_root =
            perspt_sdk::ledger::content_hash(format!("{}:{hashed}", self.chain_root).as_bytes());
        self.count += 1;
        if self.retain {
            self.retained.push(event.clone());
        }
        Ok(())
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    /// Rolling chain root over every event pushed so far.
    pub fn chain_root(&self) -> &str {
        &self.chain_root
    }

    pub fn events(&self) -> &[LoopEvent] {
        &self.retained
    }

    pub(super) fn into_events(self) -> Vec<LoopEvent> {
        self.retained
    }
}

/// Synchronous write-ahead event sink. Implementations must durably append
/// before returning; the loop records observations before inspecting them.
pub trait LoopRecorder: Send + Sync {
    fn record(&self, event: &LoopEvent) -> Result<()>;

    /// Write-ahead bracketing for durable external effects (system 13): the
    /// intent is recorded before the effect runs and the result after, so an
    /// interrupted run shows the open bracket. Defaults are no-ops for
    /// in-memory conformance fixtures.
    fn external_intent(&self, _key: &str, _intent: &serde_json::Value) -> Result<()> {
        Ok(())
    }

    fn external_result(&self, _key: &str, _result: &serde_json::Value) -> Result<()> {
        Ok(())
    }

    /// Persist exact observation bytes and return their content handle. The
    /// default supports in-memory conformance fixtures; production recorders
    /// override it with durable content-addressed storage.
    fn record_artifact(&self, content: &[u8], _media_type: &str) -> Result<String> {
        Ok(perspt_sdk::ledger::content_hash(content))
    }

    /// Retrieve previously recorded artifact bytes by content handle. The
    /// default supports in-memory conformance fixtures, which never store.
    fn fetch_artifact(&self, _handle: &str) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }
}

/// The loop's terminal report.
#[derive(Debug)]
pub struct LoopOutcome {
    pub outcome: NodeTerminalOutcome,
    pub trajectory: AcceptedTrajectory,
    pub events: Vec<LoopEvent>,
    pub projection: ProjectionMismatch,
    pub turns_used: u32,
    /// Horizontal controls consumed from Paper III's one shared recovery pool.
    pub recovery_spent: u32,
    /// True when containment was caused by exhausted provider-transport
    /// recovery — an infrastructure outcome, not a governance anomaly.
    pub contained_by_transport: bool,
}
