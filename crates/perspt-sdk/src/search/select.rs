//! Deterministic selection and frontier ordering (PSP-10 Proposition 5,
//! system 19).
//!
//! No model confidence or vote appears in either rule. Every float
//! comparison uses `total_cmp`; the final key is the unique branch id, so
//! the order is total and the selection unique.

use serde::{Deserialize, Serialize};

use super::domain_types::BranchMeasurement;

/// One eligible candidate offered to the selection rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchCandidate {
    pub measurement: BranchMeasurement,
    /// Measured improvement in the targeted residual cluster (selection
    /// key 3); higher is better.
    pub targeted_improvement: f64,
}

/// Proposition 5: hard pass, lower normalized energy, greater targeted
/// improvement, lower recorded cost, lexicographically smaller branch id.
/// Candidates whose sensor profile differs from the (unique) profile of
/// the eligible set are excluded before comparison — a profile mismatch is
/// never normalized after the fact.
pub fn select_branch(eligible: &[BranchCandidate]) -> Option<&BranchCandidate> {
    let profile = &eligible.first()?.measurement.sensor_profile;
    let comparable: Vec<&BranchCandidate> = eligible
        .iter()
        .filter(|candidate| candidate.measurement.sensor_profile == *profile)
        .collect();
    comparable.into_iter().min_by(|a, b| {
        b.measurement
            .hard_pass
            .cmp(&a.measurement.hard_pass)
            .then_with(|| a.measurement.energy.total_cmp(&b.measurement.energy))
            .then_with(|| b.targeted_improvement.total_cmp(&a.targeted_improvement))
            .then_with(|| a.measurement.cost.total_cmp(&b.measurement.cost))
            .then_with(|| a.measurement.branch_id.cmp(&b.measurement.branch_id))
    })
}

/// One live frontier entry awaiting its quantum (system 19).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrontierEntry {
    pub branch_id: String,
    /// Whether an operational barrier currently holds against this branch.
    pub barrier_blocked: bool,
    pub unresolved_obligations: u32,
    /// Dominant residual magnitude under the common sensor profile.
    pub dominant_residual: f64,
    pub cumulative_cost: f64,
    /// Partial-checkpoint chain depth from the accepted root.
    pub checkpoint_depth: u32,
}

/// The deterministic epoch scheduling key (system 19's six-key order).
/// A heuristic for fairness — not an energy function, an acceptance rule,
/// or a safety certificate.
pub fn frontier_order(a: &FrontierEntry, b: &FrontierEntry) -> std::cmp::Ordering {
    a.barrier_blocked
        .cmp(&b.barrier_blocked)
        .then_with(|| a.unresolved_obligations.cmp(&b.unresolved_obligations))
        .then_with(|| a.dominant_residual.total_cmp(&b.dominant_residual))
        .then_with(|| a.cumulative_cost.total_cmp(&b.cumulative_cost))
        .then_with(|| a.checkpoint_depth.cmp(&b.checkpoint_depth))
        .then_with(|| a.branch_id.cmp(&b.branch_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        branch: &str,
        hard: bool,
        energy: f64,
        improve: f64,
        cost: f64,
    ) -> BranchCandidate {
        BranchCandidate {
            measurement: BranchMeasurement {
                branch_id: branch.into(),
                candidate_id: format!("{branch}/c1"),
                energy,
                hard_pass: hard,
                residuals: vec![],
                sensor_profile: "profile-1".into(),
                cost,
            },
            targeted_improvement: improve,
        }
    }

    fn winner(candidates: Vec<BranchCandidate>) -> String {
        select_branch(&candidates)
            .unwrap()
            .measurement
            .branch_id
            .clone()
    }

    #[test]
    fn the_five_keys_apply_in_order() {
        // Key 1: hard pass beats lower energy.
        assert_eq!(
            winner(vec![
                candidate("a", false, 0.1, 9.0, 0.0),
                candidate("b", true, 5.0, 0.0, 9.0),
            ]),
            "b"
        );
        // Key 2: lower energy.
        assert_eq!(
            winner(vec![
                candidate("a", false, 2.0, 9.0, 0.0),
                candidate("b", false, 1.0, 0.0, 9.0),
            ]),
            "b"
        );
        // Key 3: greater targeted improvement at equal energy.
        assert_eq!(
            winner(vec![
                candidate("a", false, 1.0, 0.5, 0.0),
                candidate("b", false, 1.0, 0.9, 9.0),
            ]),
            "b"
        );
        // Key 4: lower cost.
        assert_eq!(
            winner(vec![
                candidate("a", false, 1.0, 0.5, 3.0),
                candidate("b", false, 1.0, 0.5, 2.0),
            ]),
            "b"
        );
        // Key 5: branch id breaks every remaining tie.
        assert_eq!(
            winner(vec![
                candidate("b", false, 1.0, 0.5, 2.0),
                candidate("a", false, 1.0, 0.5, 2.0),
            ]),
            "a"
        );
    }

    #[test]
    fn profile_mismatches_are_excluded_not_normalized() {
        let mut other = candidate("z", false, 0.0, 9.0, 0.0);
        other.measurement.sensor_profile = "profile-2".into();
        let candidates = vec![candidate("a", false, 1.0, 0.0, 0.0), other];
        let picked = select_branch(&candidates).unwrap();
        assert_eq!(picked.measurement.branch_id, "a");
    }

    #[test]
    fn frontier_order_is_total_and_fair() {
        let entry = |id: &str, blocked, obligations, residual, cost, depth| FrontierEntry {
            branch_id: id.into(),
            barrier_blocked: blocked,
            unresolved_obligations: obligations,
            dominant_residual: residual,
            cumulative_cost: cost,
            checkpoint_depth: depth,
        };
        let mut entries = [
            entry("c", false, 1, 2.0, 1.0, 1),
            entry("a", true, 0, 0.0, 0.0, 0),
            entry("b", false, 1, 2.0, 1.0, 1),
            entry("d", false, 0, 5.0, 9.0, 3),
        ];
        entries.sort_by(frontier_order);
        let order: Vec<&str> = entries.iter().map(|e| e.branch_id.as_str()).collect();
        // Unblocked first; fewer obligations; then residual/cost/depth/id.
        assert_eq!(order, ["d", "b", "c", "a"]);
    }
}
