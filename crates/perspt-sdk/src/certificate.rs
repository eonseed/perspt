//! Residual certificate (PSP-8 System 2 / Gate B).
//!
//! When the system cannot reach stability it terminates with a residual
//! certificate naming the remaining residuals, verifier routes, budget state,
//! and ledger head — an honest stop rather than a success claim. A residual
//! certificate is a first-class outcome, not a discarded failure.

use serde::{Deserialize, Serialize};

use crate::residual::{CorrectionDirection, IndependenceRoute, ResidualEvent};

/// A budget that was exhausted, named in a certificate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetRef {
    pub name: String,
    pub limit: u64,
    pub used: u64,
}

/// A residual certificate (PSP-8 `ResidualCertificate`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResidualCertificate {
    pub certificate_id: String,
    pub node_id: String,
    pub generation: u32,
    /// Ledger head at the moment the certificate was issued.
    pub ledger_head: String,
    /// Final total energy `V`.
    pub final_energy: f64,
    /// Final residual vector.
    pub final_residuals: Vec<ResidualEvent>,
    /// Budgets that were exhausted.
    pub exhausted_budgets: Vec<BudgetRef>,
    /// Verifier independence routes exercised.
    pub verifier_routes: Vec<IndependenceRoute>,
    /// Identifiers of rejected (observed-only) attempts.
    pub rejected_attempts: Vec<String>,
    /// Correction directions that remain to be tried.
    pub next_correction_directions: Vec<CorrectionDirection>,
}

impl ResidualCertificate {
    /// Build a certificate from the final residual vector, deriving the verifier
    /// routes and outstanding correction directions from the residuals.
    pub fn from_residuals(
        node_id: impl Into<String>,
        generation: u32,
        ledger_head: impl Into<String>,
        final_energy: f64,
        final_residuals: Vec<ResidualEvent>,
    ) -> Self {
        let mut verifier_routes: Vec<IndependenceRoute> = Vec::new();
        let mut next_correction_directions: Vec<CorrectionDirection> = Vec::new();
        for r in &final_residuals {
            if !verifier_routes.contains(&r.sensor.route) {
                verifier_routes.push(r.sensor.route);
            }
            next_correction_directions.extend(r.correction_directions.iter().cloned());
        }
        Self {
            certificate_id: uuid::Uuid::new_v4().to_string(),
            node_id: node_id.into(),
            generation,
            ledger_head: ledger_head.into(),
            final_energy,
            final_residuals,
            exhausted_budgets: Vec::new(),
            verifier_routes,
            rejected_attempts: Vec::new(),
            next_correction_directions,
        }
    }

    pub fn with_exhausted_budget(mut self, budget: BudgetRef) -> Self {
        self.exhausted_budgets.push(budget);
        self
    }

    pub fn with_rejected_attempts(mut self, attempts: Vec<String>) -> Self {
        self.rejected_attempts = attempts;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::residual::{ResidualClass, ResidualSeverity, SensorRef};

    #[test]
    fn certificate_derives_routes_and_directions() {
        let residual = ResidualEvent::new(
            "n1",
            2,
            ResidualClass::ImportGraph,
            ResidualSeverity::Error,
            1.0,
            SensorRef::new("rust-analyzer", IndependenceRoute::Lsp),
        )
        .unwrap()
        .with_correction(CorrectionDirection::new(
            ResidualClass::ImportGraph,
            "add `use crate::foo::Bar;`",
        ));

        let cert = ResidualCertificate::from_residuals("n1", 2, "head-abc", 1.0, vec![residual])
            .with_exhausted_budget(BudgetRef {
                name: "correction".into(),
                limit: 4,
                used: 4,
            });

        assert_eq!(cert.verifier_routes, vec![IndependenceRoute::Lsp]);
        assert_eq!(cert.next_correction_directions.len(), 1);
        assert_eq!(cert.exhausted_budgets.len(), 1);
        assert_eq!(cert.node_id, "n1");
    }
}
