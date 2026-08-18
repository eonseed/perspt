//! Residual evidence model (PSP-8 System 6).
//!
//! A residual is the measured reason the current state is unsafe or incomplete.
//! Each [`ResidualEvent`] stores the *raw* non-negative magnitude `r_e >= 0`;
//! the SDK squares and weights it when computing the canonical energy
//! `V = sum_e w_e r_e^2` (see [`crate::energy`]). Residuals never carry a
//! pre-squared or pre-weighted value, so the energy model stays the single
//! authority over weighting.

use serde::{Deserialize, Serialize};

use crate::error::{check_non_negative_finite, Result};

/// The five SRBN energy components. These are *derived rollups* of the single
/// quadratic residual energy, grouped for telemetry; they do not carry
/// independent weights (PSP-8 System 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnergyComponent {
    /// Syntax, parser, typechecker, and compiler diagnostics.
    Syn,
    /// Structural contract, ownership, import/symbol/interface, format, lint.
    Str,
    /// Failing tests, snapshots, property checks, behavioral validators.
    Log,
    /// Toolchain, dependency, sandbox, missing binary, degraded sensors.
    Boot,
    /// Cross-node, cross-domain, cross-adapter consistency residuals.
    Sheaf,
}

/// Residual taxonomy (PSP-8 System 6). Every verifier residual is one class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResidualClass {
    Syntax,
    Type,
    Build,
    TestFailure,
    Lint,
    Format,
    Runtime,
    Dependency,
    Manifest,
    ImportGraph,
    SymbolMismatch,
    InterfaceMismatch,
    OwnershipViolation,
    ContextDrift,
    Regression,
    Policy,
    SensorUnavailable,
    ToolFailure,
    SheafInconsistency,
    /// Admissibility outcome, not a verifier consistency residual.
    CapabilityDenied,
    /// Admissibility outcome, not a verifier consistency residual.
    BudgetExhausted,
    // --- PSP-9 system 9: tool-loop and model-plane residuals ---
    /// Tool-call arguments violated the tool schema.
    ToolArgumentInvalid,
    /// A precondition hash no longer matches; the model edited a stale view.
    WitnessStale,
    /// The same proposal (identical idempotency key) repeated without an
    /// intervening evidence change — the tool-loop analogue of livelock.
    ToolThrash,
    /// One provider turn requested non-commuting calls without an explicit
    /// dependency order; no conflicting mutation was executed.
    ToolBatchConflict,
    /// A file was mutated in a turn that never read it.
    UnreadEdit,
    /// Turn/token/capability budget consumed past a declared fraction.
    BudgetPressure,
    /// An advisory model validator objected (its fixed domain weight is
    /// declared before the run; a conjunctive adjudicator is a gate verdict,
    /// never a dynamic energy weight).
    AdjudicatorObjection,
    /// A configured route failed and failover was taken; sensor-availability
    /// class matching `SensorUnavailable`'s treatment.
    ProviderUnavailable,
}

impl ResidualClass {
    /// Default SRBN energy component for this class (PSP-8 System 6 mapping).
    pub fn default_component(self) -> EnergyComponent {
        use EnergyComponent::*;
        use ResidualClass::*;
        match self {
            Syntax | Type | Build => Syn,
            Lint | Format | ImportGraph | SymbolMismatch | InterfaceMismatch
            | OwnershipViolation | Manifest | Dependency => Str,
            TestFailure | Runtime | Regression => Log,
            SensorUnavailable | ToolFailure | ProviderUnavailable => Boot,
            SheafInconsistency | ContextDrift => Sheaf,
            // Tool-loop harness-progress residuals (PSP-9 system 9).
            ToolArgumentInvalid | WitnessStale | ToolThrash | ToolBatchConflict | UnreadEdit
            | BudgetPressure => Str,
            AdjudicatorObjection => Log,
            // Admissibility outcomes are routed to the blocked channel and are
            // never summed into V; they are reported here for completeness only.
            Policy | CapabilityDenied | BudgetExhausted => Str,
        }
    }

    /// `CapabilityDenied` and `BudgetExhausted` are admissibility outcomes that
    /// SHALL be recorded on a separate blocked channel and SHALL NOT be summed
    /// into the Lyapunov energy `V` (PSP-8 System 6).
    pub fn is_admissibility_outcome(self) -> bool {
        matches!(
            self,
            ResidualClass::CapabilityDenied | ResidualClass::BudgetExhausted
        )
    }
}

/// Severity of a residual, independent of its numeric score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResidualSeverity {
    Hint,
    Warning,
    Error,
    /// Blocks acceptance regardless of energy descent (maps to a hard gate).
    Blocking,
}

/// Verifier-independence route (PSP-8 System 6). Same-model critique is the
/// weakest route and SHALL NOT contribute a full-weight descent acceptance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndependenceRoute {
    DeterministicTool,
    Compiler,
    Lsp,
    TestOracle,
    FormalSolver,
    RepoScript,
    ExternalApi,
    SeparateModel,
    SameModelCritique,
}

impl IndependenceRoute {
    /// Whether this route may contribute a full-weight descent acceptance.
    /// Same-model critique may not (PSP-8 System 6).
    pub fn is_full_weight_eligible(self) -> bool {
        !matches!(self, IndependenceRoute::SameModelCritique)
    }
}

/// A sensor that produced a residual.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensorRef {
    /// Stable sensor identifier, e.g. `"rust-analyzer"`, `"cargo-test"`.
    pub id: String,
    /// Independence route for this sensor.
    pub route: IndependenceRoute,
    /// Tool identity, version, and configuration fingerprint (PSP-10
    /// Assumption 2). Empty on pre-PSP-10 records. Sensors with different
    /// fingerprints are never pooled as one calibration stratum.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fingerprint: String,
}

impl SensorRef {
    pub fn new(id: impl Into<String>, route: IndependenceRoute) -> Self {
        Self {
            id: id.into(),
            route,
            fingerprint: String::new(),
        }
    }

    /// Attach the tool identity/version/configuration fingerprint.
    pub fn with_fingerprint(mut self, fingerprint: impl Into<String>) -> Self {
        self.fingerprint = fingerprint.into();
        self
    }
}

/// Content-addressed reference to a recorded `CorrectionPacket`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrectionPacketRef(pub String);

/// One root-cause cluster of residuals (PSP-10 system 26): two hundred
/// cascade errors from one missing import are one cluster, not two hundred
/// units of energy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResidualClusterRef {
    pub cluster_id: String,
    pub class: ResidualClass,
    /// The primary diagnostic the cascade folds into.
    pub root_cause: String,
    pub member_count: u32,
    /// Profile-normalized magnitude, not a raw diagnostic count.
    pub magnitude: f64,
}

/// One structured diagnostic preserved from a verifier's native output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredDiagnosticRef {
    /// Tool code (`E0432`, a pytest test id, a tsc `TSxxxx`), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

/// Everything a correction may touch, preserved from the diagnostics.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AffectedSet {
    pub paths: Vec<String>,
    pub symbols: Vec<SymbolRef>,
    /// `path:line:column` spans, when the tool reports them.
    pub spans: Vec<String>,
    pub tests: Vec<String>,
    pub graph_edges: Vec<String>,
}

/// One correction operator the domain proposes for the dominant cluster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrectionOperator {
    pub operator_id: String,
    pub instruction: String,
    pub addresses: ResidualClass,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub rationale: String,
}

/// A declared sensor that could not run; correction proceeds under this
/// named uncertainty instead of pretending completeness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingSensorDecl {
    pub sensor: String,
    pub reason: String,
}

/// Reference to a recorded no-good (PSP-10 system 21; the exact-key store
/// arrives with the search plane).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoGoodRef {
    pub key: String,
    pub evidence_hash: String,
}

/// The typed correction packet carried to the next search quantum
/// (PSP-10 system 26). It preserves paths, symbols, spans, and rationale —
/// the correction channel no longer flattens to one instruction string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorrectionPacket {
    pub dominant_cluster: ResidualClusterRef,
    pub diagnostics: Vec<StructuredDiagnosticRef>,
    pub affected: AffectedSet,
    pub operators: Vec<CorrectionOperator>,
    pub expected_footprint: crate::toolset::FootprintSpec,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub follow_up_stages: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub no_goods: Vec<NoGoodRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uncertainty: Vec<MissingSensorDecl>,
}

impl CorrectionPacket {
    /// A packet with no operators and no diagnostics carries no direction;
    /// the caller expands or escalates instead of blind-retrying.
    pub fn is_empty(&self) -> bool {
        self.operators.is_empty() && self.diagnostics.is_empty()
    }
}

/// Reference to a code symbol implicated by a residual.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolRef {
    pub name: String,
    /// Enclosing container (module, file, namespace), if known.
    pub container: Option<String>,
}

/// Normalized evidence payload behind a residual.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EvidencePayload {
    /// Human-readable one-line summary.
    pub summary: String,
    /// Raw tool/LSP/test output, retained for replay and prompt context.
    pub raw: Option<String>,
    /// Structured detail (diagnostic JSON, AST query result, etc.).
    pub structured: Option<serde_json::Value>,
}

/// A correction direction: the targeted instruction the controller derives from
/// a dominant residual cluster (PSP-8 System 6). Undirected retries are a bug.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrectionDirection {
    pub direction_id: String,
    /// The residual class this direction addresses.
    pub addresses: ResidualClass,
    /// What to do, in domain terms (e.g. "add `use crate::foo::Bar;`").
    pub instruction: String,
    /// Files the correction is expected to touch.
    pub target_paths: Vec<String>,
    /// Symbols the correction is expected to touch.
    pub target_symbols: Vec<SymbolRef>,
    /// Why this direction was chosen.
    pub rationale: String,
}

impl CorrectionDirection {
    pub fn new(addresses: ResidualClass, instruction: impl Into<String>) -> Self {
        Self {
            direction_id: uuid::Uuid::new_v4().to_string(),
            addresses,
            instruction: instruction.into(),
            target_paths: Vec::new(),
            target_symbols: Vec::new(),
            rationale: String::new(),
        }
    }

    pub fn with_rationale(mut self, rationale: impl Into<String>) -> Self {
        self.rationale = rationale.into();
        self
    }

    pub fn with_paths(mut self, paths: Vec<String>) -> Self {
        self.target_paths = paths;
        self
    }
}

/// A first-class residual event (PSP-8 System 6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResidualEvent {
    pub residual_id: String,
    pub node_id: String,
    pub generation: u32,
    pub component: EnergyComponent,
    pub class: ResidualClass,
    pub severity: ResidualSeverity,
    /// Raw non-negative magnitude `r_e >= 0`. The SDK squares and weights it.
    pub score: f64,
    pub sensor: SensorRef,
    pub evidence: EvidencePayload,
    pub affected_paths: Vec<String>,
    pub affected_symbols: Vec<SymbolRef>,
    pub correction_directions: Vec<CorrectionDirection>,
}

impl ResidualEvent {
    /// Construct a residual, validating that the raw score is finite and
    /// non-negative. The component defaults to the class mapping but may be
    /// overridden afterward by a domain package.
    pub fn new(
        node_id: impl Into<String>,
        generation: u32,
        class: ResidualClass,
        severity: ResidualSeverity,
        score: f64,
        sensor: SensorRef,
    ) -> Result<Self> {
        check_non_negative_finite(score, "residual score")?;
        Ok(Self {
            residual_id: uuid::Uuid::new_v4().to_string(),
            node_id: node_id.into(),
            generation,
            component: class.default_component(),
            class,
            severity,
            score,
            sensor,
            evidence: EvidencePayload::default(),
            affected_paths: Vec::new(),
            affected_symbols: Vec::new(),
            correction_directions: Vec::new(),
        })
    }

    pub fn with_evidence(mut self, evidence: EvidencePayload) -> Self {
        self.evidence = evidence;
        self
    }

    pub fn with_component(mut self, component: EnergyComponent) -> Self {
        self.component = component;
        self
    }

    pub fn with_paths(mut self, paths: Vec<String>) -> Self {
        self.affected_paths = paths;
        self
    }

    pub fn with_correction(mut self, direction: CorrectionDirection) -> Self {
        self.correction_directions.push(direction);
        self
    }

    /// Whether this residual is an admissibility outcome (blocked channel) and
    /// therefore excluded from the Lyapunov energy.
    pub fn is_admissibility_outcome(&self) -> bool {
        self.class.is_admissibility_outcome()
    }
}

/// A lightweight reference to a residual, used in energy traces and gate
/// decisions to point at dominant residuals without copying the full payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResidualEventRef {
    pub residual_id: String,
    pub class: ResidualClass,
    pub component: EnergyComponent,
    /// Weighted energy contribution `w_e * r_e^2` of this residual.
    pub weighted_energy: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sensor() -> SensorRef {
        SensorRef::new("compiler", IndependenceRoute::Compiler)
    }

    #[test]
    fn rejects_negative_score() {
        let err = ResidualEvent::new(
            "n1",
            0,
            ResidualClass::Type,
            ResidualSeverity::Error,
            -1.0,
            sensor(),
        );
        assert!(err.is_err());
    }

    #[test]
    fn rejects_nan_and_inf_score() {
        assert!(ResidualEvent::new(
            "n1",
            0,
            ResidualClass::Type,
            ResidualSeverity::Error,
            f64::NAN,
            sensor()
        )
        .is_err());
        assert!(ResidualEvent::new(
            "n1",
            0,
            ResidualClass::Type,
            ResidualSeverity::Error,
            f64::INFINITY,
            sensor()
        )
        .is_err());
    }

    #[test]
    fn class_maps_to_default_component() {
        assert_eq!(
            ResidualClass::Type.default_component(),
            EnergyComponent::Syn
        );
        assert_eq!(
            ResidualClass::TestFailure.default_component(),
            EnergyComponent::Log
        );
        assert_eq!(
            ResidualClass::ImportGraph.default_component(),
            EnergyComponent::Str
        );
        assert_eq!(
            ResidualClass::ToolFailure.default_component(),
            EnergyComponent::Boot
        );
        assert_eq!(
            ResidualClass::SheafInconsistency.default_component(),
            EnergyComponent::Sheaf
        );
    }

    #[test]
    fn admissibility_outcomes_flagged() {
        assert!(ResidualClass::CapabilityDenied.is_admissibility_outcome());
        assert!(ResidualClass::BudgetExhausted.is_admissibility_outcome());
        assert!(!ResidualClass::Type.is_admissibility_outcome());
    }

    #[test]
    fn same_model_critique_not_full_weight() {
        assert!(!IndependenceRoute::SameModelCritique.is_full_weight_eligible());
        assert!(IndependenceRoute::Compiler.is_full_weight_eligible());
    }
}
