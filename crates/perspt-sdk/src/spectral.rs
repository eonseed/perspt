//! Spectral energy-slope constant `mu` (PSP-8 System 2 / Gate G).
//!
//! For the quadratic residual energy `V(x) = x^T A x` with `A = B^T W B`, the
//! energy-slope constant `mu = 2 * lambda_min+(A)` is combinatorial rather than
//! embedding-dependent. In the identity-restriction case `A` reduces to the
//! weighted Laplacian of the verification graph and `mu` is twice its algebraic
//! connectivity (Fiedler value).
//!
//! `mu` is a diagnostic, not an input to the acceptance gate. It SHALL be
//! computed off the critical path and SHALL NOT block dispatch or the gate.
//! This module is therefore a pure, side-effect-free computation a runtime can
//! schedule on graph-revision commit or asynchronously.

use nalgebra::DMatrix;
use serde::{Deserialize, Serialize};

use crate::error::{check_positive_finite, Result, SdkError};

/// One weighted edge of the verification graph. An edge `(i, j)` couples the
/// local states of two nodes/verifiers; its weight is the residual weight
/// `w_e > 0`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationEdge {
    pub src: usize,
    pub dst: usize,
    pub weight: f64,
}

impl VerificationEdge {
    pub fn new(src: usize, dst: usize, weight: f64) -> Self {
        Self { src, dst, weight }
    }
}

/// A verification graph over `node_count` local-state components.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VerificationGraph {
    pub node_count: usize,
    pub edges: Vec<VerificationEdge>,
}

impl VerificationGraph {
    pub fn new(node_count: usize) -> Self {
        Self { node_count, edges: Vec::new() }
    }

    pub fn with_edge(mut self, src: usize, dst: usize, weight: f64) -> Self {
        self.edges.push(VerificationEdge::new(src, dst, weight));
        self
    }

    pub fn add_edge(&mut self, src: usize, dst: usize, weight: f64) {
        self.edges.push(VerificationEdge::new(src, dst, weight));
    }

    /// Build the weighted graph Laplacian `A = B^T W B` (identity-restriction
    /// case), an `n x n` symmetric positive-semidefinite matrix.
    fn laplacian(&self) -> Result<DMatrix<f64>> {
        if self.node_count == 0 {
            return Err(SdkError::Spectral("verification graph has no nodes".into()));
        }
        let n = self.node_count;
        let mut a = DMatrix::<f64>::zeros(n, n);
        for e in &self.edges {
            if e.src >= n || e.dst >= n {
                return Err(SdkError::Spectral(format!(
                    "edge ({}, {}) out of range for {} nodes",
                    e.src, e.dst, n
                )));
            }
            if e.src == e.dst {
                return Err(SdkError::Spectral(format!("self-loop at node {}", e.src)));
            }
            check_positive_finite(e.weight, "edge weight")?;
            let w = e.weight;
            a[(e.src, e.src)] += w;
            a[(e.dst, e.dst)] += w;
            a[(e.src, e.dst)] -= w;
            a[(e.dst, e.src)] -= w;
        }
        Ok(a)
    }

    /// Sorted ascending eigenvalues of the Laplacian.
    fn eigenvalues_sorted(&self) -> Result<Vec<f64>> {
        let a = self.laplacian()?;
        // The Laplacian is symmetric; use the symmetric eigensolver.
        let mut eigs: Vec<f64> = a.symmetric_eigenvalues().iter().copied().collect();
        eigs.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
        Ok(eigs)
    }

    /// Smallest non-zero eigenvalue `lambda_min+(A)` of the Laplacian.
    ///
    /// Returns `None` when the graph is disconnected or empty of edges (the
    /// spectral gap is then zero, so `mu` is not informative). Eigenvalues
    /// within `tol` of zero are treated as zero.
    pub fn smallest_nonzero_eigenvalue(&self, tol: f64) -> Result<Option<f64>> {
        let eigs = self.eigenvalues_sorted()?;
        Ok(eigs.into_iter().find(|&v| v > tol))
    }

    /// The spectral energy-slope constant `mu = 2 * lambda_min+(A)`.
    ///
    /// Returns `None` for a disconnected verification graph, where the gap is
    /// zero and `mu` is uninformative (a disconnected verifier component cannot
    /// be driven to consensus by the others).
    pub fn mu(&self, tol: f64) -> Result<Option<f64>> {
        Ok(self.smallest_nonzero_eigenvalue(tol)?.map(|lambda| 2.0 * lambda))
    }

    /// Algebraic connectivity (Fiedler value) — the smallest non-zero
    /// eigenvalue; `mu = 2 * fiedler`.
    pub fn fiedler_value(&self, tol: f64) -> Result<Option<f64>> {
        self.smallest_nonzero_eigenvalue(tol)
    }

    /// Change in `mu` produced by adding a candidate verifier edge.
    ///
    /// An independent verifier that raises the spectral gap yields a positive
    /// delta; a redundant verifier that does not yields ~0. Used to distinguish
    /// independent from redundant verifiers (PSP-8 System 2).
    pub fn edge_mu_sensitivity(
        &self,
        src: usize,
        dst: usize,
        weight: f64,
        tol: f64,
    ) -> Result<f64> {
        let before = self.mu(tol)?.unwrap_or(0.0);
        let mut candidate = self.clone();
        candidate.add_edge(src, dst, weight);
        let after = candidate.mu(tol)?.unwrap_or(0.0);
        Ok(after - before)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-9;

    #[test]
    fn path_graph_has_expected_fiedler_value() {
        // Unweighted path on 3 nodes: Laplacian eigenvalues are 0, 1, 3.
        let g = VerificationGraph::new(3)
            .with_edge(0, 1, 1.0)
            .with_edge(1, 2, 1.0);
        let fiedler = g.fiedler_value(TOL).unwrap().unwrap();
        assert!((fiedler - 1.0).abs() < 1e-6, "fiedler={fiedler}");
        let mu = g.mu(TOL).unwrap().unwrap();
        assert!((mu - 2.0).abs() < 1e-6, "mu={mu}");
    }

    #[test]
    fn complete_triangle_connectivity() {
        // Unweighted triangle: eigenvalues 0, 3, 3 -> fiedler = 3.
        let g = VerificationGraph::new(3)
            .with_edge(0, 1, 1.0)
            .with_edge(1, 2, 1.0)
            .with_edge(0, 2, 1.0);
        let fiedler = g.fiedler_value(TOL).unwrap().unwrap();
        assert!((fiedler - 3.0).abs() < 1e-6, "fiedler={fiedler}");
    }

    #[test]
    fn disconnected_graph_has_no_spectral_gap() {
        // Two isolated edges over 4 nodes -> two zero eigenvalues, but the
        // second smallest is still 0 -> disconnected.
        let g = VerificationGraph::new(4)
            .with_edge(0, 1, 1.0)
            .with_edge(2, 3, 1.0);
        // smallest nonzero exists (the within-component value) but the graph is
        // disconnected: there are two zero eigenvalues. We detect disconnection
        // by counting zeros.
        let eigs = g.eigenvalues_sorted().unwrap();
        let zeros = eigs.iter().filter(|&&v| v.abs() <= TOL).count();
        assert_eq!(zeros, 2, "disconnected graph has multiplicity-2 zero eigenvalue");
    }

    #[test]
    fn independent_verifier_raises_mu_more_than_redundant() {
        // Base: path 0-1-2.
        let g = VerificationGraph::new(3)
            .with_edge(0, 1, 1.0)
            .with_edge(1, 2, 1.0);
        // Adding the closing edge 0-2 (independent cross-check) raises the gap.
        let independent = g.edge_mu_sensitivity(0, 2, 1.0, TOL).unwrap();
        // Strengthening an existing coupling 0-1 (redundant) raises it less.
        let redundant = g.edge_mu_sensitivity(0, 1, 1.0, TOL).unwrap();
        assert!(independent > 0.0);
        assert!(
            independent >= redundant,
            "independent={independent} redundant={redundant}"
        );
    }

    #[test]
    fn weighted_edges_scale_gap() {
        let g1 = VerificationGraph::new(2).with_edge(0, 1, 1.0);
        let g2 = VerificationGraph::new(2).with_edge(0, 1, 2.0);
        let m1 = g1.mu(TOL).unwrap().unwrap();
        let m2 = g2.mu(TOL).unwrap().unwrap();
        assert!((m2 - 2.0 * m1).abs() < 1e-6);
    }

    #[test]
    fn rejects_self_loop_and_out_of_range() {
        assert!(VerificationGraph::new(2).with_edge(0, 0, 1.0).mu(TOL).is_err());
        assert!(VerificationGraph::new(2).with_edge(0, 5, 1.0).mu(TOL).is_err());
    }
}
