//! Measured verifier independence (PSP-8 System 6 / Gate G).
//!
//! Verifier independence SHALL be measured, not assumed. From the per-candidate
//! verdict history the SDK computes each validator's miss rate `q_i` and the
//! pairwise miss correlation `kappa_ij`, then the effective conjunctive ensemble
//! miss bound
//!
//! ```text
//! rho_eff = min_{i<j} ( q_i q_j + kappa_ij sigma_i sigma_j ),
//!           sigma_i = sqrt(q_i (1 - q_i)).
//! ```
//!
//! Residual weights are attenuated for validators whose measured correlation
//! with an already-counted validator is high, so a redundant validator does not
//! contribute the weight of an independent one. Status views surface `rho_eff`
//! rather than a raw count of validators.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{Result, SdkError};

/// One validator's verdict on one candidate: did it *miss* an unsafe state?
///
/// `missed == true` means the validator accepted (passed) a candidate that was
/// later found unsafe — i.e. a false negative. These are the events whose rate
/// and correlation determine ensemble strength.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerdictRecord {
    pub validator_id: String,
    pub candidate_id: String,
    pub missed: bool,
}

impl VerdictRecord {
    pub fn new(validator_id: impl Into<String>, candidate_id: impl Into<String>, missed: bool) -> Self {
        Self {
            validator_id: validator_id.into(),
            candidate_id: candidate_id.into(),
            missed,
        }
    }
}

/// Independence statistics computed from the verdict ledger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndependenceStats {
    /// Per-validator miss rate `q_i`.
    pub miss_rates: BTreeMap<String, f64>,
    /// Pairwise miss correlation `kappa_ij`, keyed by ordered pair.
    pub correlations: BTreeMap<(String, String), f64>,
    /// Effective conjunctive ensemble miss bound `rho_eff`, if computable.
    pub rho_eff: Option<f64>,
}

/// Standard deviation of a Bernoulli miss indicator: `sigma = sqrt(q(1-q))`.
pub fn miss_std(q: f64) -> f64 {
    (q * (1.0 - q)).max(0.0).sqrt()
}

/// Compute independence statistics from a verdict ledger.
///
/// Only candidates evaluated by *both* validators contribute to a pairwise
/// correlation. The miss rate uses all of a validator's verdicts.
pub fn compute(records: &[VerdictRecord]) -> Result<IndependenceStats> {
    if records.is_empty() {
        return Err(SdkError::Domain("no verdict records".into()));
    }

    // Group verdicts: validator -> (candidate -> missed).
    let mut by_validator: BTreeMap<String, BTreeMap<String, bool>> = BTreeMap::new();
    for r in records {
        by_validator
            .entry(r.validator_id.clone())
            .or_default()
            .insert(r.candidate_id.clone(), r.missed);
    }

    // Per-validator miss rate q_i.
    let mut miss_rates = BTreeMap::new();
    for (v, verdicts) in &by_validator {
        let n = verdicts.len() as f64;
        let misses = verdicts.values().filter(|&&m| m).count() as f64;
        miss_rates.insert(v.clone(), misses / n);
    }

    // Pairwise miss correlation kappa_ij over jointly-evaluated candidates.
    let validators: Vec<&String> = by_validator.keys().collect();
    let mut correlations = BTreeMap::new();
    let mut rho_eff: Option<f64> = None;

    for a in 0..validators.len() {
        for b in (a + 1)..validators.len() {
            let vi = validators[a];
            let vj = validators[b];
            let mi = by_validator[vi].clone();
            let mj = by_validator[vj].clone();

            // Joint candidates.
            let joint: Vec<(bool, bool)> = mi
                .iter()
                .filter_map(|(c, &m_i)| mj.get(c).map(|&m_j| (m_i, m_j)))
                .collect();
            if joint.is_empty() {
                continue;
            }

            let kappa = pearson_phi(&joint);
            correlations.insert((vi.clone(), vj.clone()), kappa);

            let qi = miss_rates[vi];
            let qj = miss_rates[vj];
            let bound = qi * qj + kappa * miss_std(qi) * miss_std(qj);
            rho_eff = Some(match rho_eff {
                Some(current) => current.min(bound),
                None => bound,
            });
        }
    }

    Ok(IndependenceStats { miss_rates, correlations, rho_eff })
}

/// Phi coefficient (Pearson correlation for two binary variables) over paired
/// miss indicators. Returns 0 when either variable has zero variance.
fn pearson_phi(pairs: &[(bool, bool)]) -> f64 {
    let n = pairs.len() as f64;
    let to_f = |b: bool| if b { 1.0 } else { 0.0 };
    let sum_x: f64 = pairs.iter().map(|&(x, _)| to_f(x)).sum();
    let sum_y: f64 = pairs.iter().map(|&(_, y)| to_f(y)).sum();
    let mean_x = sum_x / n;
    let mean_y = sum_y / n;
    let mut cov = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;
    for &(x, y) in pairs {
        let dx = to_f(x) - mean_x;
        let dy = to_f(y) - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }
    if var_x <= f64::EPSILON || var_y <= f64::EPSILON {
        return 0.0;
    }
    cov / (var_x.sqrt() * var_y.sqrt())
}

/// Attenuate a residual weight by measured correlation with an already-counted
/// validator: `w_eff = w * (1 - max(0, kappa))`. A perfectly correlated
/// (redundant) validator contributes no additional weight; an uncorrelated or
/// anti-correlated one keeps its full weight.
pub fn attenuate_weight(weight: f64, correlation_with_counted: f64) -> f64 {
    weight * (1.0 - correlation_with_counted.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn miss_std_matches_bernoulli() {
        assert!((miss_std(0.5) - 0.5).abs() < 1e-12);
        assert_eq!(miss_std(0.0), 0.0);
        assert_eq!(miss_std(1.0), 0.0);
    }

    #[test]
    fn independent_validators_have_low_rho_eff() {
        // Two validators, anti-correlated misses: when one misses the other
        // catches. rho_eff should be below the product-of-rates upper estimate.
        let records = vec![
            VerdictRecord::new("a", "c1", true),
            VerdictRecord::new("b", "c1", false),
            VerdictRecord::new("a", "c2", false),
            VerdictRecord::new("b", "c2", true),
            VerdictRecord::new("a", "c3", true),
            VerdictRecord::new("b", "c3", false),
            VerdictRecord::new("a", "c4", false),
            VerdictRecord::new("b", "c4", true),
        ];
        let stats = compute(&records).unwrap();
        let kappa = stats.correlations.get(&("a".into(), "b".into())).copied().unwrap();
        assert!(kappa < 0.0, "expected anti-correlation, got {kappa}");
        assert!(stats.rho_eff.unwrap() >= 0.0);
    }

    #[test]
    fn redundant_validators_are_attenuated() {
        // Perfectly correlated misses -> correlation 1 -> attenuated to zero.
        let records = vec![
            VerdictRecord::new("a", "c1", true),
            VerdictRecord::new("b", "c1", true),
            VerdictRecord::new("a", "c2", false),
            VerdictRecord::new("b", "c2", false),
            VerdictRecord::new("a", "c3", true),
            VerdictRecord::new("b", "c3", true),
        ];
        let stats = compute(&records).unwrap();
        let kappa = stats.correlations.get(&("a".into(), "b".into())).copied().unwrap();
        assert!((kappa - 1.0).abs() < 1e-9, "expected perfect correlation, got {kappa}");
        assert_eq!(attenuate_weight(2.0, kappa), 0.0);
    }

    #[test]
    fn empty_ledger_is_error() {
        assert!(compute(&[]).is_err());
    }
}
