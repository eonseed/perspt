//! Goal-presence sensor (PSP-8): the verifier that refuses false stability.
//!
//! The SRBN gate accepts a state once its energy `V` drops below the declared
//! floor. But an empty or placeholder file *compiles*, has no failing tests, and
//! no diagnostics — so every code-quality sensor reports `r_e = 0` and the gate
//! would declare it stable even though the requested work was never done. That
//! is *false stability*: convergence to a fixed point that does not satisfy the
//! goal.
//!
//! PSP-8 closes this with a goal-presence sensor: a node carries a [`GoalSpec`]
//! naming the symbols its work must bring into existence, and the sensor emits a
//! **blocking** [`ResidualClass::SymbolMismatch`] residual for every expected
//! symbol that is absent from the observed workspace. A blocking residual is a
//! hard-gate failure (`hard(y)` is false), so the node cannot be accepted while
//! its goal artifact is missing, regardless of how low the other energy
//! components are.
//!
//! This module is domain-neutral: it compares *names*. Extracting the expected
//! names from a task contract and the observed names from source is the domain
//! package's job (see `perspt-coding`), keeping the mechanism/domain split of
//! PSP-8 intact.

use std::collections::BTreeSet;

use crate::error::Result;
use crate::residual::{
    CorrectionDirection, IndependenceRoute, ResidualClass, ResidualEvent, ResidualSeverity,
    SensorRef, SymbolRef,
};

/// The set of symbols a node's work is required to bring into existence.
///
/// A node with an empty spec has no goal-presence obligation: the sensor stays
/// silent rather than guessing, so it never penalizes work whose success cannot
/// be expressed as "these named symbols must exist".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GoalSpec {
    pub node_id: String,
    /// Symbol names that MUST be present once the node is done.
    pub expected_symbols: Vec<String>,
}

impl GoalSpec {
    pub fn new(node_id: impl Into<String>, expected_symbols: Vec<String>) -> Self {
        Self {
            node_id: node_id.into(),
            expected_symbols,
        }
    }

    /// No declared obligation — the sensor MUST NOT fire.
    pub fn is_empty(&self) -> bool {
        self.expected_symbols.is_empty()
    }
}

/// The expected symbols absent from `observed`, preserving declared order and
/// de-duplicating. Comparison is exact on the name string; the domain is
/// responsible for normalizing names (e.g. stripping a `module::` prefix)
/// before building the [`GoalSpec`] and the observed set.
pub fn missing_symbols(spec: &GoalSpec, observed: &BTreeSet<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    spec.expected_symbols
        .iter()
        .filter(|name| !observed.contains(*name))
        .filter(|name| seen.insert((*name).clone()))
        .cloned()
        .collect()
}

/// The goal-presence sensor identifier and its independence route. It is a
/// deterministic AST/source query, not a model critique, so it is full-weight
/// eligible.
pub fn goal_presence_sensor() -> SensorRef {
    SensorRef::new("goal-presence", IndependenceRoute::DeterministicTool)
}

/// Emit a single blocking goal-presence residual when any expected symbol is
/// absent from `observed`. Returns `None` when the spec is empty or the goal is
/// already satisfied.
///
/// The residual's raw magnitude is the number of missing symbols, so a node
/// that delivered nothing scores higher than one that is only partially short.
/// Because the severity is [`ResidualSeverity::Blocking`], the gate treats it as
/// a hard failure: `V` may be otherwise zero, but `hard(y)` is false, so the
/// node is not accepted.
pub fn goal_presence_residual(
    spec: &GoalSpec,
    generation: u32,
    observed: &BTreeSet<String>,
) -> Result<Option<ResidualEvent>> {
    if spec.is_empty() {
        return Ok(None);
    }
    let missing = missing_symbols(spec, observed);
    if missing.is_empty() {
        return Ok(None);
    }

    let summary = format!(
        "goal not satisfied: {} expected symbol(s) absent from the workspace: {}",
        missing.len(),
        missing.join(", ")
    );

    let mut residual = ResidualEvent::new(
        &spec.node_id,
        generation,
        ResidualClass::SymbolMismatch,
        ResidualSeverity::Blocking,
        missing.len() as f64,
        goal_presence_sensor(),
    )?;
    residual.evidence.summary = summary;
    residual.affected_symbols = missing
        .iter()
        .map(|name| SymbolRef {
            name: name.clone(),
            container: None,
        })
        .collect();
    residual = residual.with_correction(
        CorrectionDirection::new(
            ResidualClass::SymbolMismatch,
            format!(
                "the requested work is missing: define {} so the goal is satisfied; \
                 do not stop until each named symbol exists",
                missing.join(", ")
            ),
        )
        .with_rationale("an empty or placeholder file compiles but does not satisfy the goal"),
    );

    Ok(Some(residual))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observed(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_spec_never_fires() {
        let spec = GoalSpec::new("n1", vec![]);
        assert!(goal_presence_residual(&spec, 0, &observed(&[]))
            .unwrap()
            .is_none());
    }

    #[test]
    fn satisfied_goal_produces_no_residual() {
        let spec = GoalSpec::new("n1", vec!["multiply".into()]);
        let r = goal_presence_residual(&spec, 0, &observed(&["multiply", "helper"])).unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn missing_symbol_is_blocking_residual() {
        let spec = GoalSpec::new("n1", vec!["is_even".into()]);
        let r = goal_presence_residual(&spec, 0, &observed(&["unrelated"]))
            .unwrap()
            .expect("missing symbol must produce a residual");
        assert_eq!(r.class, ResidualClass::SymbolMismatch);
        assert_eq!(r.severity, ResidualSeverity::Blocking);
        assert_eq!(r.score, 1.0);
        assert_eq!(r.affected_symbols.len(), 1);
        assert_eq!(r.affected_symbols[0].name, "is_even");
        assert_eq!(r.correction_directions.len(), 1);
    }

    #[test]
    fn score_counts_all_missing_symbols() {
        let spec = GoalSpec::new("n1", vec!["a".into(), "b".into(), "c".into()]);
        let r = goal_presence_residual(&spec, 0, &observed(&["b"]))
            .unwrap()
            .unwrap();
        assert_eq!(r.score, 2.0); // a and c missing
    }

    #[test]
    fn missing_symbols_dedup_and_order() {
        let spec = GoalSpec::new("n1", vec!["a".into(), "a".into(), "b".into()]);
        assert_eq!(missing_symbols(&spec, &observed(&[])), vec!["a", "b"]);
    }

    #[test]
    fn symbol_mismatch_routes_to_structural_component() {
        // A goal-presence failure is structural, not a syntax/test failure.
        assert_eq!(
            ResidualClass::SymbolMismatch.default_component(),
            crate::residual::EnergyComponent::Str
        );
    }
}
