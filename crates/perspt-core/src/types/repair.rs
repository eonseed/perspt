use super::*;

// =============================================================================
// PSP-5 Phase 5: Escalation Semantics, Local Graph Rewrite, Sheaf Targeting
// =============================================================================

/// PSP-5 Phase 5: Category of non-convergence detected by the verifier.
///
/// When a node exceeds its retry budget or fails to decrease energy, the
/// orchestrator classifies the failure into one of these categories so the
/// runtime can choose a targeted repair action instead of only escalating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationCategory {
    /// Compilation, type, or syntax errors that remain after retries.
    ImplementationError,
    /// Node output violates its behavioral contract or interface seal.
    ContractMismatch,
    /// Model is unable to produce acceptable output for this node's tier.
    InsufficientModelCapability,
    /// Required verifier tools are missing or degraded.
    DegradedSensors,
    /// Node scope does not match ownership or dependency graph structure.
    TopologyMismatch,
}

impl std::fmt::Display for EscalationCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EscalationCategory::ImplementationError => write!(f, "implementation_error"),
            EscalationCategory::ContractMismatch => write!(f, "contract_mismatch"),
            EscalationCategory::InsufficientModelCapability => {
                write!(f, "insufficient_model_capability")
            }
            EscalationCategory::DegradedSensors => write!(f, "degraded_sensors"),
            EscalationCategory::TopologyMismatch => write!(f, "topology_mismatch"),
        }
    }
}

/// PSP-5 Phase 5: Repair action chosen by the orchestrator after classifying
/// non-convergence.
///
/// Actions are ordered from least destructive (retry with evidence) to most
/// disruptive (user escalation).  The orchestrator picks the first action
/// that is safe given the current evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewriteAction {
    /// Re-attempt the node with a correction prompt grounded in verifier output.
    GroundedRetry {
        /// Human-readable summary of the evidence fed back to the LLM.
        evidence_summary: String,
    },
    /// Refine or tighten the node's behavioral contract or interface seal.
    ContractRepair {
        /// Which contract fields need adjustment.
        fields: Vec<String>,
    },
    /// Promote the node to a higher-capability model tier.
    CapabilityPromotion {
        /// Current tier.
        from_tier: ModelTier,
        /// Proposed tier.
        to_tier: ModelTier,
    },
    /// Attempt to recover a degraded sensor or stop with explicit degradation.
    SensorRecovery {
        /// Stages that are degraded.
        degraded_stages: Vec<String>,
    },
    /// Stop the node with an explicit degraded-validation marker rather than
    /// claiming false stability.
    DegradedValidationStop {
        /// Reason the runtime is stopping without full verification.
        reason: String,
    },
    /// Split the current node by ownership closure into smaller nodes.
    NodeSplit {
        /// Proposed child node IDs after splitting.
        proposed_children: Vec<String>,
    },
    /// Insert an interface node between this node and its dependents.
    InterfaceInsertion {
        /// The boundary that motivated the insertion.
        boundary: String,
    },
    /// Re-plan a local subgraph rooted at the failing node.
    SubgraphReplan {
        /// Node IDs in the affected subgraph.
        affected_nodes: Vec<String>,
    },
    /// Escalate to the user with stored evidence (last resort).
    UserEscalation {
        /// Structured evidence for the user.
        evidence: String,
    },
}

impl std::fmt::Display for RewriteAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RewriteAction::GroundedRetry { .. } => write!(f, "grounded_retry"),
            RewriteAction::ContractRepair { .. } => write!(f, "contract_repair"),
            RewriteAction::CapabilityPromotion { .. } => write!(f, "capability_promotion"),
            RewriteAction::SensorRecovery { .. } => write!(f, "sensor_recovery"),
            RewriteAction::DegradedValidationStop { .. } => {
                write!(f, "degraded_validation_stop")
            }
            RewriteAction::NodeSplit { .. } => write!(f, "node_split"),
            RewriteAction::InterfaceInsertion { .. } => write!(f, "interface_insertion"),
            RewriteAction::SubgraphReplan { .. } => write!(f, "subgraph_replan"),
            RewriteAction::UserEscalation { .. } => write!(f, "user_escalation"),
        }
    }
}

/// PSP-5 Phase 5: Sheaf validator class.
///
/// Each class checks a different cross-node consistency property after child
/// nodes converge and before the parent node is committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SheafValidatorClass {
    /// Exported symbols, trait impls, and module imports match dependency interfaces.
    ExportImportConsistency,
    /// Repository dependency edges remain acyclic and node-local changes do not
    /// introduce invalid module or package references.
    DependencyGraphConsistency,
    /// JSON schemas, API types, and serialization contracts remain compatible.
    SchemaContractCompatibility,
    /// Plugin-selected build targets remain satisfiable for the affected subgraph.
    BuildGraphConsistency,
    /// Failing tests are attributed to the owning node or interface boundary.
    TestOwnershipConsistency,
    /// FFI layers, generated clients, and protocol bindings across plugin boundaries.
    CrossLanguageBoundary,
    /// Repository-wide invariants and forbidden patterns still hold.
    PolicyInvariantConsistency,
}

impl std::fmt::Display for SheafValidatorClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SheafValidatorClass::ExportImportConsistency => write!(f, "export_import"),
            SheafValidatorClass::DependencyGraphConsistency => write!(f, "dependency_graph"),
            SheafValidatorClass::SchemaContractCompatibility => write!(f, "schema_contract"),
            SheafValidatorClass::BuildGraphConsistency => write!(f, "build_graph"),
            SheafValidatorClass::TestOwnershipConsistency => write!(f, "test_ownership"),
            SheafValidatorClass::CrossLanguageBoundary => write!(f, "cross_language"),
            SheafValidatorClass::PolicyInvariantConsistency => write!(f, "policy_invariant"),
        }
    }
}

/// PSP-5 Phase 5: Result of a single sheaf validation pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheafValidationResult {
    /// Which validator class produced this result.
    pub validator_class: SheafValidatorClass,
    /// Plugin that owns the validator (if any).
    pub plugin_source: Option<String>,
    /// Whether the validation passed.
    pub passed: bool,
    /// Boundaries that were validated.
    pub validated_boundaries: Vec<String>,
    /// Evidence summary when validation fails.
    pub evidence_summary: String,
    /// Files or interfaces affected by the failure.
    pub affected_files: Vec<String>,
    /// Energy contribution to V_sheaf.
    pub v_sheaf_contribution: f32,
    /// Node IDs recommended for requeue on failure.
    pub requeue_targets: Vec<String>,
}

impl SheafValidationResult {
    /// Create a passing result.
    pub fn passed(class: SheafValidatorClass, boundaries: Vec<String>) -> Self {
        Self {
            validator_class: class,
            plugin_source: None,
            passed: true,
            validated_boundaries: boundaries,
            evidence_summary: String::new(),
            affected_files: Vec::new(),
            v_sheaf_contribution: 0.0,
            requeue_targets: Vec::new(),
        }
    }

    /// Create a failing result with evidence.
    pub fn failed(
        class: SheafValidatorClass,
        evidence: impl Into<String>,
        affected: Vec<String>,
        requeue: Vec<String>,
        v_sheaf: f32,
    ) -> Self {
        Self {
            validator_class: class,
            plugin_source: None,
            passed: false,
            validated_boundaries: Vec::new(),
            evidence_summary: evidence.into(),
            affected_files: affected,
            v_sheaf_contribution: v_sheaf,
            requeue_targets: requeue,
        }
    }
}

/// PSP-5 Phase 5: Full escalation report assembled by the orchestrator.
///
/// Captures everything needed for persistence, user display, and later
/// resume: the failing node, the classified category, the chosen repair
/// action, verifier evidence, and energy snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationReport {
    /// Node that triggered escalation.
    pub node_id: String,
    /// Session this report belongs to.
    pub session_id: String,
    /// Classified failure category.
    pub category: EscalationCategory,
    /// Repair action chosen (or UserEscalation if none was safe).
    pub action: RewriteAction,
    /// Energy at the time of escalation.
    pub energy_snapshot: EnergyComponents,
    /// Verifier stage outcomes at the time of escalation.
    pub stage_outcomes: Vec<StageOutcome>,
    /// Human-readable evidence summary.
    pub evidence: String,
    /// Node IDs affected by the chosen action (requeue targets).
    pub affected_node_ids: Vec<String>,
    /// Timestamp (epoch seconds).
    pub timestamp: i64,
}

/// PSP-5 Phase 5: Record of a local graph rewrite applied by the orchestrator.
///
/// Stored in the ledger so Phase 8 resume can replay or audit rewrite history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewriteRecord {
    /// Node that was rewritten.
    pub node_id: String,
    /// Session this record belongs to.
    pub session_id: String,
    /// The rewrite action that was applied.
    pub action: RewriteAction,
    /// Category that triggered the rewrite.
    pub category: EscalationCategory,
    /// Node IDs that were requeued as a result.
    pub requeued_nodes: Vec<String>,
    /// Node IDs that were newly inserted (e.g. interface insertion).
    pub inserted_nodes: Vec<String>,
    /// Timestamp (epoch seconds).
    pub timestamp: i64,
}

/// PSP-5 Phase 5: Targeted requeue entry.
///
/// When a sheaf validator or escalation identifies a subset of nodes for
/// re-execution, this record tracks the targeting metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetedRequeue {
    /// Node IDs targeted for requeue.
    pub node_ids: Vec<String>,
    /// Reason for the requeue (validator class or escalation category).
    pub reason: String,
    /// Evidence that justified targeting these specific nodes.
    pub evidence: String,
    /// Sheaf validation results that triggered this requeue (if any).
    pub sheaf_results: Vec<SheafValidationResult>,
    /// Timestamp (epoch seconds).
    pub timestamp: i64,
}
