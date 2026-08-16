//! Measured validator dependence (PSP-9 system 8, Paper III Lemma 5.1 and
//! Corollary 5.2).
//!
//! *"Correlation among redundant validators is measured, never assumed
//! away."* This module computes, from matched labeled verdicts, each
//! validator's miss rate `q_i`, the pairwise miss correlation `κ_ij`, the
//! empirical pairwise joint miss, and the conservative conjunctive-ensemble
//! bound `ρ_eff = min_{i<j} u_ij` over pairwise joint-miss **upper
//! confidence bounds**.
//!
//! Statistics discipline (Gates R and T):
//!
//! * **Matched strata.** `q_i`, `q_j`, `κ_ij`, and the joint miss for a pair
//!   are all computed over the same jointly evaluated candidate set.
//! * **Degenerate marginals** (`q ∈ {0, 1}`) use the empirical joint miss
//!   directly, as Lemma 5.1 requires; they are never assigned `κ = 0`.
//! * **Exact only at n = 2.** For `n > 2` validators the bound is
//!   deliberately conservative; it is never tightened by inferring
//!   higher-order joints from pairwise correlations, and `n` is reported
//!   beside it. A three-validator bound tighter than its best pair would be
//!   a defect, not a better certificate.
//! * **Point estimates are diagnostics.** A certified `ρ_eff` uses upper
//!   confidence bounds and a minimum matched sample count; below it the
//!   status is `insufficient evidence`, not a number.
//! * **No energy attenuation.** Correlation controls the reported risk
//!   bound; it never rewrites the fixed quadratic energy weights. The former
//!   `attenuate_weight` helper is removed for that reason.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{Result, SdkError};

/// One validator's verdict on one candidate: did it *miss* an unsafe state?
///
/// `missed == true` means the validator accepted a candidate later found
/// unsafe — a false negative. Labels come from the delayed oracle (rollback
/// boundary, regression residuals), never from contemporaneous agreement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerdictRecord {
    pub validator_id: String,
    pub candidate_id: String,
    pub missed: bool,
}

impl VerdictRecord {
    pub fn new(
        validator_id: impl Into<String>,
        candidate_id: impl Into<String>,
        missed: bool,
    ) -> Self {
        Self {
            validator_id: validator_id.into(),
            candidate_id: candidate_id.into(),
            missed,
        }
    }
}

/// Matched-stratum statistics for one validator pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairStats {
    /// Miss rates over the *matched* candidate set.
    pub q_i: f64,
    pub q_j: f64,
    /// Phi correlation over matched misses; `None` for degenerate marginals,
    /// which use the joint miss directly rather than a fabricated zero.
    pub kappa: Option<f64>,
    /// Empirical joint miss rate over the matched set (point diagnostic).
    pub joint_miss: f64,
    /// Conservative upper confidence bound on the joint miss (Hoeffding at
    /// 95%): the value a certificate may use.
    pub joint_miss_upper: f64,
    /// Matched sample count.
    pub samples: usize,
}

/// Certification status of the ensemble bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnsembleCertification {
    /// Every pair met the matched-sample floor; `rho_eff` uses upper bounds.
    Certified,
    /// Point diagnostics only; no risk certificate may be displayed.
    InsufficientEvidence,
}

/// Independence statistics computed from the verdict ledger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndependenceStats {
    /// Per-validator miss rate over all of its verdicts (diagnostic only).
    pub miss_rates: BTreeMap<String, f64>,
    /// Matched-stratum pair statistics, keyed by ordered pair.
    pub pairs: BTreeMap<(String, String), PairStats>,
    /// `min_{i<j} u_ij` over pairwise joint-miss upper bounds, when
    /// certified; the paper's conservative conjunctive bound.
    pub rho_eff: Option<f64>,
    /// Number of validators in the ensemble — reported beside the bound
    /// because Corollary 5.2 is exact only at `n = 2`.
    pub validators: usize,
    pub certification: EnsembleCertification,
}

/// Standard deviation of a Bernoulli miss indicator: `σ = sqrt(q(1-q))`.
pub fn miss_std(q: f64) -> f64 {
    (q * (1.0 - q)).max(0.0).sqrt()
}

/// Lemma 5.1's exact pairwise joint miss: `q_i q_j + κ σ_i σ_j`.
pub fn pairwise_joint_miss(q_i: f64, q_j: f64, kappa: f64) -> f64 {
    q_i * q_j + kappa * miss_std(q_i) * miss_std(q_j)
}

/// Minimum matched samples per pair before a certificate may be issued.
pub const DEFAULT_MIN_PAIR_SAMPLES: usize = 20;

/// Compute independence statistics from a verdict ledger.
pub fn compute(records: &[VerdictRecord]) -> Result<IndependenceStats> {
    compute_with_floor(records, DEFAULT_MIN_PAIR_SAMPLES)
}

/// Compute with an explicit matched-sample floor for certification.
pub fn compute_with_floor(
    records: &[VerdictRecord],
    min_pair_samples: usize,
) -> Result<IndependenceStats> {
    if records.is_empty() {
        return Err(SdkError::Domain("no verdict records".into()));
    }

    let mut by_validator: BTreeMap<String, BTreeMap<String, bool>> = BTreeMap::new();
    for r in records {
        by_validator
            .entry(r.validator_id.clone())
            .or_default()
            .insert(r.candidate_id.clone(), r.missed);
    }

    // Diagnostic-only marginal rates over each validator's full history.
    let mut miss_rates = BTreeMap::new();
    for (v, verdicts) in &by_validator {
        let n = verdicts.len() as f64;
        let misses = verdicts.values().filter(|&&m| m).count() as f64;
        miss_rates.insert(v.clone(), misses / n);
    }

    let validators: Vec<&String> = by_validator.keys().collect();
    let mut pairs = BTreeMap::new();
    let mut rho_eff: Option<f64> = None;
    let mut all_pairs_certified = !validators.is_empty() && validators.len() > 1;

    for a in 0..validators.len() {
        for b in (a + 1)..validators.len() {
            let (vi, vj) = (validators[a], validators[b]);
            let joint: Vec<(bool, bool)> = by_validator[vi]
                .iter()
                .filter_map(|(c, &m_i)| by_validator[vj].get(c).map(|&m_j| (m_i, m_j)))
                .collect();
            if joint.is_empty() {
                all_pairs_certified = false;
                continue;
            }
            let stats = pair_stats(&joint);
            if stats.samples < min_pair_samples {
                all_pairs_certified = false;
            }
            rho_eff = Some(match rho_eff {
                Some(current) => current.min(stats.joint_miss_upper),
                None => stats.joint_miss_upper,
            });
            pairs.insert((vi.clone(), vj.clone()), stats);
        }
    }

    let certification = if all_pairs_certified && !pairs.is_empty() {
        EnsembleCertification::Certified
    } else {
        EnsembleCertification::InsufficientEvidence
    };
    Ok(IndependenceStats {
        miss_rates,
        pairs,
        // A bound may only be *displayed* when certified; the point value is
        // still computed for diagnostics but gated here.
        rho_eff: match certification {
            EnsembleCertification::Certified => rho_eff,
            EnsembleCertification::InsufficientEvidence => None,
        },
        validators: validators.len(),
        certification,
    })
}

/// Matched-stratum statistics for one pair's joint verdicts.
fn pair_stats(joint: &[(bool, bool)]) -> PairStats {
    let n = joint.len() as f64;
    let q_i = joint.iter().filter(|&&(x, _)| x).count() as f64 / n;
    let q_j = joint.iter().filter(|&&(_, y)| y).count() as f64 / n;
    let joint_miss = joint.iter().filter(|&&(x, y)| x && y).count() as f64 / n;
    let degenerate = q_i == 0.0 || q_i == 1.0 || q_j == 0.0 || q_j == 1.0;
    let kappa = if degenerate {
        None
    } else {
        Some(pearson_phi(joint))
    };
    // Hoeffding one-sided 95% upper bound: p̂ + sqrt(ln(1/0.05) / (2n)).
    let joint_miss_upper = (joint_miss + ((1.0 / 0.05f64).ln() / (2.0 * n)).sqrt()).min(1.0);
    PairStats {
        q_i,
        q_j,
        kappa,
        joint_miss,
        joint_miss_upper,
        samples: joint.len(),
    }
}

/// Phi coefficient over paired miss indicators. Callers exclude degenerate
/// marginals before calling.
fn pearson_phi(pairs: &[(bool, bool)]) -> f64 {
    let n = pairs.len() as f64;
    let to_f = |b: bool| if b { 1.0 } else { 0.0 };
    let mean_x: f64 = pairs.iter().map(|&(x, _)| to_f(x)).sum::<f64>() / n;
    let mean_y: f64 = pairs.iter().map(|&(_, y)| to_f(y)).sum::<f64>() / n;
    let (mut cov, mut var_x, mut var_y) = (0.0, 0.0, 0.0);
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
    fn lemma_5_1_reproduces_the_papers_mc3_arithmetic() {
        // Paper III MC3: q = 0.1, kappa = 0.5 gives a joint miss of
        // 0.01 + 0.5 * 0.09 = 0.055, not the independence product 0.01.
        let joint = pairwise_joint_miss(0.1, 0.1, 0.5);
        assert!((joint - 0.055).abs() < 1e-12, "{joint}");
    }

    #[test]
    fn marginals_and_correlation_use_the_matched_stratum() {
        // Validator a saw c1..c4; validator b saw only c1, c2. The pair's
        // q_i must come from the matched {c1, c2}, not a's full history.
        let records = vec![
            VerdictRecord::new("a", "c1", true),
            VerdictRecord::new("a", "c2", false),
            VerdictRecord::new("a", "c3", false),
            VerdictRecord::new("a", "c4", false),
            VerdictRecord::new("b", "c1", true),
            VerdictRecord::new("b", "c2", false),
        ];
        let stats = compute_with_floor(&records, 2).unwrap();
        let pair = &stats.pairs[&("a".to_string(), "b".to_string())];
        assert_eq!(pair.samples, 2);
        assert!((pair.q_i - 0.5).abs() < 1e-12, "matched q_i, not 0.25");
    }

    #[test]
    fn degenerate_marginals_use_the_joint_miss_never_kappa_zero() {
        // Validator a never misses in the matched set: q_i = 0. Lemma 5.1
        // requires the direct joint miss; kappa is not fabricated.
        let records = vec![
            VerdictRecord::new("a", "c1", false),
            VerdictRecord::new("a", "c2", false),
            VerdictRecord::new("b", "c1", true),
            VerdictRecord::new("b", "c2", false),
        ];
        let stats = compute_with_floor(&records, 2).unwrap();
        let pair = &stats.pairs[&("a".to_string(), "b".to_string())];
        assert_eq!(pair.kappa, None);
        assert_eq!(pair.joint_miss, 0.0);
    }

    #[test]
    fn below_the_sample_floor_no_bound_is_displayed() {
        let records = vec![
            VerdictRecord::new("a", "c1", true),
            VerdictRecord::new("b", "c1", true),
        ];
        let stats = compute(&records).unwrap();
        assert_eq!(
            stats.certification,
            EnsembleCertification::InsufficientEvidence
        );
        assert_eq!(
            stats.rho_eff, None,
            "insufficient evidence displays no number"
        );
    }

    #[test]
    fn three_validator_bound_is_never_tighter_than_its_best_pair() {
        // MC-T companion: with three validators, rho_eff equals the minimum
        // pairwise upper bound — never anything smaller.
        let mut records = Vec::new();
        for i in 0..40 {
            let candidate = format!("c{i}");
            let a_miss = i % 10 == 0;
            let b_miss = i % 10 == 0; // b correlates with a
            let c_miss = i % 13 == 0; // c is mostly independent
            records.push(VerdictRecord::new("a", &candidate, a_miss));
            records.push(VerdictRecord::new("b", &candidate, b_miss));
            records.push(VerdictRecord::new("c", &candidate, c_miss));
        }
        let stats = compute(&records).unwrap();
        assert_eq!(stats.validators, 3);
        assert_eq!(stats.certification, EnsembleCertification::Certified);
        let best_pair = stats
            .pairs
            .values()
            .map(|p| p.joint_miss_upper)
            .fold(f64::INFINITY, f64::min);
        assert_eq!(stats.rho_eff.unwrap(), best_pair);
        // And the certified bound is conservative: at or above every pair's
        // point estimate minimum.
        let best_point = stats
            .pairs
            .values()
            .map(|p| p.joint_miss)
            .fold(f64::INFINITY, f64::min);
        assert!(stats.rho_eff.unwrap() >= best_point);
    }

    #[test]
    fn empty_ledger_is_error() {
        assert!(compute(&[]).is_err());
    }
}
