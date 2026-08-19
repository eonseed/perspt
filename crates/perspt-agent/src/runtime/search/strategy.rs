//! Deliberate branch strategies (PSP-10 system 20).
//!
//! A strategy is real behavior, never a bare label: it decides whether the
//! branch continues the witnessed partial or restarts fresh, and it
//! contributes a distinct goal-program fragment. Expansion is trigger-
//! driven — repeated failure signatures, stagnation, or measured progress
//! each select a different next strategy.

/// The four deliberate strategies of the release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BranchStrategy {
    /// Continue the current approach; smallest change that clears the
    /// remaining diagnostics.
    LocalRepair,
    /// The signature repeated: discard the approach and implement a
    /// genuinely different one from the accepted root.
    AlternativeApproach,
    /// Localize before editing: run the verifier tools and fix exactly
    /// what the evidence names, continuing the partial.
    DiagnosticProbe,
    /// Independent fresh attempt, preferably on a distinct model family.
    DistinctFamily,
}

impl BranchStrategy {
    pub(crate) fn id(&self) -> &'static str {
        match self {
            BranchStrategy::LocalRepair => "local-repair",
            BranchStrategy::AlternativeApproach => "alternative-approach",
            BranchStrategy::DiagnosticProbe => "diagnostic-probe",
            BranchStrategy::DistinctFamily => "distinct-family",
        }
    }

    /// Whether this branch seeds from the witnessed partial checkpoint
    /// (continuation) or from the forest's accepted seed (fresh).
    pub(crate) fn continues_partial(&self) -> bool {
        matches!(
            self,
            BranchStrategy::LocalRepair | BranchStrategy::DiagnosticProbe
        )
    }

    /// The strategy's goal-program fragment (typed selection, fixed text).
    pub(crate) fn goal_fragment(&self) -> &'static str {
        match self {
            BranchStrategy::LocalRepair => {
                "Continue the current approach: make the smallest change that \
                 clears the remaining diagnostics listed below."
            }
            BranchStrategy::AlternativeApproach => {
                "Earlier attempts failed with the same signature. Discard that \
                 approach entirely and implement a genuinely different \
                 algorithm or structure for the goal."
            }
            BranchStrategy::DiagnosticProbe => {
                "Before editing further, run the verifier tools (run_test, \
                 run_build) and read the failing output to localize the fault; \
                 then fix exactly what the evidence names."
            }
            BranchStrategy::DistinctFamily => {
                "Approach the goal independently from first principles; do not \
                 assume any earlier partial attempt exists."
            }
        }
    }
}

/// What the previous branch measured, folded to the trigger inputs.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct BranchSummary {
    /// The failure signature equals the previous branch's (Gate AB input).
    pub repeated_signature: bool,
    /// Fewer remaining obligations than the previous branch.
    pub obligations_decreased: bool,
    /// Energy strictly below the previous branch's.
    pub energy_improved: bool,
}

/// The trigger-driven next strategy after an ineligible branch
/// (system 20's expansion preference: operator → strategy → family).
pub(crate) fn next_strategy(previous: BranchSummary) -> BranchStrategy {
    if previous.repeated_signature {
        BranchStrategy::AlternativeApproach
    } else if previous.obligations_decreased {
        BranchStrategy::LocalRepair
    } else if !previous.energy_improved {
        BranchStrategy::DistinctFamily
    } else {
        BranchStrategy::DiagnosticProbe
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triggers_select_distinct_behaviors() {
        assert_eq!(
            next_strategy(BranchSummary {
                repeated_signature: true,
                ..Default::default()
            }),
            BranchStrategy::AlternativeApproach
        );
        assert_eq!(
            next_strategy(BranchSummary {
                obligations_decreased: true,
                ..Default::default()
            }),
            BranchStrategy::LocalRepair
        );
        assert_eq!(
            next_strategy(BranchSummary::default()),
            BranchStrategy::DistinctFamily
        );
        assert_eq!(
            next_strategy(BranchSummary {
                energy_improved: true,
                ..Default::default()
            }),
            BranchStrategy::DiagnosticProbe
        );
        assert!(BranchStrategy::LocalRepair.continues_partial());
        assert!(!BranchStrategy::AlternativeApproach.continues_partial());
    }
}
