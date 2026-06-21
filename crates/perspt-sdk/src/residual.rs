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
            SensorUnavailable | ToolFailure => Boot,
            SheafInconsistency | ContextDrift => Sheaf,
            // Admissibility outcomes are routed to the blocked channel and are
            // never summed into V; they are reported here for completeness only.
            Policy | CapabilityDenied | BudgetExhausted => Str,
        }
    }

    /// `CapabilityDenied` and `BudgetExhausted` are admissibility outcomes that
    /// SHALL be recorded on a separate blocked channel and SHALL NOT be summed
    /// into the Lyapunov energy `V` (PSP-8 System 6).
    pub fn is_admissibility_outcome(self) -> bool {
        matches!(self, ResidualClass::CapabilityDenied | ResidualClass::BudgetExhausted)
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
}

impl SensorRef {
    pub fn new(id: impl Into<String>, route: IndependenceRoute) -> Self {
        Self { id: id.into(), route }
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
        let err = ResidualEvent::new("n1", 0, ResidualClass::Type, ResidualSeverity::Error, -1.0, sensor());
        assert!(err.is_err());
    }

    #[test]
    fn rejects_nan_and_inf_score() {
        assert!(ResidualEvent::new("n1", 0, ResidualClass::Type, ResidualSeverity::Error, f64::NAN, sensor()).is_err());
        assert!(ResidualEvent::new("n1", 0, ResidualClass::Type, ResidualSeverity::Error, f64::INFINITY, sensor()).is_err());
    }

    #[test]
    fn class_maps_to_default_component() {
        assert_eq!(ResidualClass::Type.default_component(), EnergyComponent::Syn);
        assert_eq!(ResidualClass::TestFailure.default_component(), EnergyComponent::Log);
        assert_eq!(ResidualClass::ImportGraph.default_component(), EnergyComponent::Str);
        assert_eq!(ResidualClass::ToolFailure.default_component(), EnergyComponent::Boot);
        assert_eq!(ResidualClass::SheafInconsistency.default_component(), EnergyComponent::Sheaf);
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
