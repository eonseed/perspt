//! Benchmark harness and metrics (PSP-8 System 13).
//!
//! Perspt does not claim SRBN reliability from implementation alone; it ships
//! mechanism checks and benchmarks that can falsify the PSP's claims. A residual
//! certificate is a first-class outcome, not a discarded failure: **no benchmark
//! report omits failed runs**, and `false stability` (claiming success while a
//! required sensor was missing) is tracked as its own metric and must be zero.

use serde::{Deserialize, Serialize};

/// The terminal outcome of one benchmark case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkOutcome {
    /// All required verifiers passed.
    HardPass,
    /// Honest non-convergence: terminated with a residual certificate.
    ResidualCertified,
    /// A regression appeared after commit.
    Regression,
    /// Claimed success while a required sensor was missing — a correctness bug
    /// that must never occur.
    FalseStability,
}

impl BenchmarkOutcome {
    pub fn is_success(self) -> bool {
        matches!(self, BenchmarkOutcome::HardPass)
    }

    /// Whether this outcome is a correctness violation (never acceptable).
    pub fn is_correctness_violation(self) -> bool {
        matches!(self, BenchmarkOutcome::FalseStability)
    }
}

/// One benchmark case description.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkCase {
    pub case_id: String,
    pub domain: String,
    pub description: String,
}

impl BenchmarkCase {
    pub fn new(
        case_id: impl Into<String>,
        domain: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            case_id: case_id.into(),
            domain: domain.into(),
            description: description.into(),
        }
    }
}

/// The recorded result of running one case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub case_id: String,
    pub outcome: BenchmarkOutcome,
    pub gate_decisions: u32,
    pub energy_descents: u32,
    pub graph_revisions: u32,
    pub verifier_calls: u32,
    pub capability_denials: u32,
}

impl BenchmarkResult {
    pub fn new(case_id: impl Into<String>, outcome: BenchmarkOutcome) -> Self {
        Self {
            case_id: case_id.into(),
            outcome,
            gate_decisions: 0,
            energy_descents: 0,
            graph_revisions: 0,
            verifier_calls: 0,
            capability_denials: 0,
        }
    }
}

/// A full benchmark report (PSP-8 System 13 metrics).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub results: Vec<BenchmarkResult>,
}

impl BenchmarkReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, result: BenchmarkResult) {
        self.results.push(result);
    }

    pub fn total(&self) -> usize {
        self.results.len()
    }

    fn rate(&self, predicate: impl Fn(&BenchmarkResult) -> bool) -> f64 {
        if self.results.is_empty() {
            return 0.0;
        }
        self.results.iter().filter(|r| predicate(r)).count() as f64 / self.results.len() as f64
    }

    /// Final hard-pass rate.
    pub fn hard_pass_rate(&self) -> f64 {
        self.rate(|r| r.outcome == BenchmarkOutcome::HardPass)
    }

    /// Residual-certified termination rate.
    pub fn residual_certified_rate(&self) -> f64 {
        self.rate(|r| r.outcome == BenchmarkOutcome::ResidualCertified)
    }

    /// False-stability rate. This MUST be zero for a conformant implementation.
    pub fn false_stability_rate(&self) -> f64 {
        self.rate(|r| r.outcome == BenchmarkOutcome::FalseStability)
    }

    /// Regression-after-commit rate.
    pub fn regression_rate(&self) -> f64 {
        self.rate(|r| r.outcome == BenchmarkOutcome::Regression)
    }

    /// Whether the report preserves failures (a report of only successes that
    /// hides certified/failed runs would violate System 13). True if the report
    /// retains every case it was given — which it always does, since `add`
    /// appends unconditionally. This predicate exists to assert the invariant in
    /// tests and to document it.
    pub fn preserves_failures(&self) -> bool {
        // By construction nothing is filtered; the report is the source of truth.
        true
    }

    /// Whether the implementation is correctness-conformant: no false-stability
    /// outcomes occurred.
    pub fn is_correctness_conformant(&self) -> bool {
        self.results
            .iter()
            .all(|r| !r.outcome.is_correctness_violation())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_computes_rates_over_all_outcomes() {
        let mut report = BenchmarkReport::new();
        report.add(BenchmarkResult::new("c1", BenchmarkOutcome::HardPass));
        report.add(BenchmarkResult::new("c2", BenchmarkOutcome::HardPass));
        report.add(BenchmarkResult::new(
            "c3",
            BenchmarkOutcome::ResidualCertified,
        ));
        report.add(BenchmarkResult::new("c4", BenchmarkOutcome::Regression));
        assert_eq!(report.total(), 4);
        assert_eq!(report.hard_pass_rate(), 0.5);
        assert_eq!(report.residual_certified_rate(), 0.25);
        assert_eq!(report.regression_rate(), 0.25);
    }

    #[test]
    fn failed_runs_are_preserved_not_omitted() {
        let mut report = BenchmarkReport::new();
        report.add(BenchmarkResult::new("ok", BenchmarkOutcome::HardPass));
        report.add(BenchmarkResult::new(
            "certified",
            BenchmarkOutcome::ResidualCertified,
        ));
        // The certified (non-success) run is retained in the report.
        assert!(report
            .results
            .iter()
            .any(|r| r.outcome == BenchmarkOutcome::ResidualCertified));
        assert!(report.preserves_failures());
    }

    #[test]
    fn false_stability_breaks_correctness_conformance() {
        let mut report = BenchmarkReport::new();
        report.add(BenchmarkResult::new("ok", BenchmarkOutcome::HardPass));
        assert!(report.is_correctness_conformant());
        report.add(BenchmarkResult::new(
            "bad",
            BenchmarkOutcome::FalseStability,
        ));
        assert!(!report.is_correctness_conformant());
        assert!(report.false_stability_rate() > 0.0);
    }
}
