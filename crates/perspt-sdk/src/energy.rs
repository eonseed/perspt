//! Canonical quadratic residual energy (PSP-8 System 2).
//!
//! The single energy that gates acceptance, feeds the finite-decision bound,
//! and is recorded for replay is
//!
//! ```text
//! V(x) = sum_{e in E} w_e * ||r_e(x)||^2,   w_e > 0.
//! ```
//!
//! Each [`ResidualEvent`] stores the raw magnitude `r_e >= 0`; the SDK squares
//! and weights it here. The component aggregates `V_syn, V_str, V_log, V_boot,
//! V_sheaf` are *derived rollups* of this same quadratic energy:
//!
//! ```text
//! V_comp = sum_{e in comp} w_e * ||r_e||^2,   V(x) = sum_comp V_comp.
//! ```
//!
//! The rollups are user-visible projections only: they carry no independent
//! weights and are never summed through a second weighting pass.

use serde::{Deserialize, Serialize};

use crate::error::{check_positive_finite, Result, SdkError};
use crate::residual::{EnergyComponent, ResidualClass, ResidualEvent, ResidualEventRef, SensorRef};

/// One residual weight entry in an [`EnergyModel`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResidualWeight {
    pub class: ResidualClass,
    /// Optional sensor specificity; `None` matches any sensor for the class.
    pub sensor: Option<String>,
    pub component: EnergyComponent,
    /// Strictly positive edge weight `w_e`.
    pub weight: f64,
    /// Optional hard-check threshold on the raw residual magnitude.
    pub hard_threshold: Option<f64>,
}

impl ResidualWeight {
    pub fn new(class: ResidualClass, component: EnergyComponent, weight: f64) -> Self {
        Self { class, sensor: None, component, weight, hard_threshold: None }
    }

    pub fn for_sensor(mut self, sensor: impl Into<String>) -> Self {
        self.sensor = Some(sensor.into());
        self
    }

    pub fn with_hard_threshold(mut self, threshold: f64) -> Self {
        self.hard_threshold = Some(threshold);
        self
    }
}

/// The declared energy model for a domain scope (PSP-8 System 5 `EnergyModel`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnergyModel {
    pub model_id: String,
    pub domain: String,
    pub residual_weights: Vec<ResidualWeight>,
    /// The single descent tolerance `rho_gate > 0`.
    pub rho_gate: f64,
    /// Energy at or below which the candidate is treated as inside tolerance.
    pub energy_tolerance: f64,
    /// Finite correction budget (number of permitted regenerations).
    pub correction_budget: u32,
    /// Optional analytic stability claim (continuous constants).
    pub stability_claim: Option<crate::stability::StabilityClaim>,
}

impl EnergyModel {
    pub fn new(domain: impl Into<String>, rho_gate: f64) -> Self {
        Self {
            model_id: uuid::Uuid::new_v4().to_string(),
            domain: domain.into(),
            residual_weights: Vec::new(),
            rho_gate,
            energy_tolerance: 0.0,
            correction_budget: 4,
            stability_claim: None,
        }
    }

    pub fn with_weight(mut self, weight: ResidualWeight) -> Self {
        self.residual_weights.push(weight);
        self
    }

    pub fn with_correction_budget(mut self, budget: u32) -> Self {
        self.correction_budget = budget;
        self
    }

    /// Validate the model: `rho_gate > 0`, finite tolerance, and every declared
    /// weight strictly positive and finite.
    pub fn validate(&self) -> Result<()> {
        check_positive_finite(self.rho_gate, "rho_gate")?;
        if !self.energy_tolerance.is_finite() || self.energy_tolerance < 0.0 {
            return Err(SdkError::InvalidGate(format!(
                "energy_tolerance must be finite and non-negative: {}",
                self.energy_tolerance
            )));
        }
        for w in &self.residual_weights {
            check_positive_finite(w.weight, "residual weight")?;
            if let Some(t) = w.hard_threshold {
                if !t.is_finite() || t < 0.0 {
                    return Err(SdkError::InvalidWeight(format!(
                        "hard_threshold must be finite and non-negative: {t}"
                    )));
                }
            }
        }
        Ok(())
    }

    /// Resolve the weight and component for a residual. A sensor-specific entry
    /// wins over a class-wide entry. Returns `None` when no weight is declared
    /// (the caller treats that as an error: PSP-8 forbids implicit weight `1`).
    pub fn resolve(&self, class: ResidualClass, sensor: &SensorRef) -> Option<&ResidualWeight> {
        self.residual_weights
            .iter()
            .find(|w| w.class == class && w.sensor.as_deref() == Some(sensor.id.as_str()))
            .or_else(|| {
                self.residual_weights
                    .iter()
                    .find(|w| w.class == class && w.sensor.is_none())
            })
    }
}

/// Derived component rollups (PSP-8 System 2). Telemetry projections only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct EnergyComponents {
    pub v_syn: f64,
    pub v_str: f64,
    pub v_log: f64,
    pub v_boot: f64,
    pub v_sheaf: f64,
}

impl EnergyComponents {
    /// Total energy `V(x) = sum_comp V_comp`. Because the components are already
    /// weighted-squared rollups, this is a plain sum with no second weighting.
    pub fn total(&self) -> f64 {
        self.v_syn + self.v_str + self.v_log + self.v_boot + self.v_sheaf
    }

    fn add(&mut self, component: EnergyComponent, energy: f64) {
        match component {
            EnergyComponent::Syn => self.v_syn += energy,
            EnergyComponent::Str => self.v_str += energy,
            EnergyComponent::Log => self.v_log += energy,
            EnergyComponent::Boot => self.v_boot += energy,
            EnergyComponent::Sheaf => self.v_sheaf += energy,
        }
    }
}

/// The full energy evaluation of a candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnergyScore {
    /// Total energy `V`.
    pub total: f64,
    /// Derived component rollups.
    pub components: EnergyComponents,
    /// Residuals sorted by weighted energy, descending: dominant first.
    pub dominant: Vec<ResidualEventRef>,
    /// Residual classes that hit a hard threshold (force a hard-fail).
    pub hard_violations: Vec<ResidualClass>,
    /// Admissibility-outcome residuals excluded from `V` (blocked channel).
    pub blocked: Vec<ResidualEventRef>,
}

/// Compute the canonical quadratic energy for a candidate's residual vector
/// against a declared [`EnergyModel`].
///
/// Admissibility-outcome residuals (`CapabilityDenied`, `BudgetExhausted`) are
/// routed to the `blocked` channel and excluded from `V`. Every consistency
/// residual SHALL have a declared weight; a missing weight is an error rather
/// than an implicit weight of `1`.
pub fn score_candidate(model: &EnergyModel, residuals: &[ResidualEvent]) -> Result<EnergyScore> {
    model.validate()?;

    let mut components = EnergyComponents::default();
    let mut dominant: Vec<ResidualEventRef> = Vec::new();
    let mut blocked: Vec<ResidualEventRef> = Vec::new();
    let mut hard_violations: Vec<ResidualClass> = Vec::new();

    for r in residuals {
        // Raw score already validated at construction, but re-check defensively
        // so a hand-built residual cannot smuggle in a non-finite score.
        crate::error::check_non_negative_finite(r.score, "residual score")?;

        if r.is_admissibility_outcome() {
            blocked.push(ResidualEventRef {
                residual_id: r.residual_id.clone(),
                class: r.class,
                component: r.component,
                weighted_energy: 0.0,
            });
            continue;
        }

        let weight = model.resolve(r.class, &r.sensor).ok_or_else(|| {
            SdkError::InvalidWeight(format!(
                "no declared weight for residual class {:?} from sensor {}",
                r.class, r.sensor.id
            ))
        })?;

        if let Some(threshold) = weight.hard_threshold {
            if r.score > threshold {
                hard_violations.push(r.class);
            }
        }

        let weighted = weight.weight * r.score * r.score;
        components.add(weight.component, weighted);
        dominant.push(ResidualEventRef {
            residual_id: r.residual_id.clone(),
            class: r.class,
            component: weight.component,
            weighted_energy: weighted,
        });
    }

    dominant.sort_by(|a, b| {
        b.weighted_energy
            .partial_cmp(&a.weighted_energy)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let total = components.total();
    // Total is a sum of finite non-negative terms; assert the invariant.
    debug_assert!(total.is_finite() && total >= 0.0);

    Ok(EnergyScore {
        total,
        components,
        dominant,
        hard_violations,
        blocked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::residual::{IndependenceRoute, ResidualSeverity};

    fn model() -> EnergyModel {
        EnergyModel::new("test", 0.5)
            .with_weight(ResidualWeight::new(ResidualClass::Type, EnergyComponent::Syn, 2.0))
            .with_weight(ResidualWeight::new(ResidualClass::TestFailure, EnergyComponent::Log, 1.0))
    }

    fn residual(class: ResidualClass, score: f64) -> ResidualEvent {
        ResidualEvent::new(
            "n1",
            0,
            class,
            ResidualSeverity::Error,
            score,
            SensorRef::new("compiler", IndependenceRoute::Compiler),
        )
        .unwrap()
    }

    #[test]
    fn energy_is_weighted_sum_of_squares() {
        // V = 2.0 * 3^2 + 1.0 * 2^2 = 18 + 4 = 22.
        let residuals = vec![
            residual(ResidualClass::Type, 3.0),
            residual(ResidualClass::TestFailure, 2.0),
        ];
        let score = score_candidate(&model(), &residuals).unwrap();
        assert_eq!(score.total, 22.0);
        assert_eq!(score.components.v_syn, 18.0);
        assert_eq!(score.components.v_log, 4.0);
        // Dominant residual is the type error (18 > 4).
        assert_eq!(score.dominant[0].class, ResidualClass::Type);
    }

    #[test]
    fn missing_weight_is_error_not_implicit_one() {
        let residuals = vec![residual(ResidualClass::Build, 1.0)];
        assert!(score_candidate(&model(), &residuals).is_err());
    }

    #[test]
    fn admissibility_outcomes_excluded_from_energy() {
        let residuals = vec![
            residual(ResidualClass::Type, 3.0),
            residual(ResidualClass::CapabilityDenied, 99.0),
        ];
        let score = score_candidate(&model(), &residuals).unwrap();
        assert_eq!(score.total, 18.0);
        assert_eq!(score.blocked.len(), 1);
    }

    #[test]
    fn hard_threshold_flags_violation() {
        let model = EnergyModel::new("test", 0.5).with_weight(
            ResidualWeight::new(ResidualClass::Type, EnergyComponent::Syn, 1.0).with_hard_threshold(0.0),
        );
        let score = score_candidate(&model, &[residual(ResidualClass::Type, 1.0)]).unwrap();
        assert_eq!(score.hard_violations, vec![ResidualClass::Type]);
    }

    #[test]
    fn rejects_non_positive_weight() {
        let model = EnergyModel::new("test", 0.5)
            .with_weight(ResidualWeight::new(ResidualClass::Type, EnergyComponent::Syn, 0.0));
        assert!(model.validate().is_err());
    }

    #[test]
    fn empty_residuals_give_zero_energy() {
        let score = score_candidate(&model(), &[]).unwrap();
        assert_eq!(score.total, 0.0);
    }
}
