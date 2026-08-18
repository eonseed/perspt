//! Deterministic prompt budget fitting (Definition 4, resolved decision 11).
//!
//! Optional sections drop in increasing priority order, ties broken by
//! section id, until the estimate fits the token budget. Required sections
//! never drop. If the required sections alone exceed the budget,
//! compilation fails closed. The dropped-section set is therefore a pure
//! function of the inputs; it enters the program digest and the ledger.

use crate::error::{Result, SdkError};

use super::accountant::TokenAccountantRef;
use super::section::{PromptSectionId, RenderedSection};

/// The outcome of one deterministic fit.
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetFit {
    /// Surviving sections, in their original declared order.
    pub kept: Vec<RenderedSection>,
    /// Dropped section ids, in drop order (ascending priority, ties by id).
    pub dropped: Vec<PromptSectionId>,
}

/// Fit sections to `token_budget` under `accountant`.
pub fn fit_budget(
    sections: Vec<RenderedSection>,
    accountant: &TokenAccountantRef,
    token_budget: u64,
) -> Result<BudgetFit> {
    let required_cost: u64 = sections
        .iter()
        .filter(|section| section.required)
        .map(|section| accountant.count_message(&section.content))
        .sum();
    if required_cost > token_budget {
        return Err(SdkError::Domain(format!(
            "required sections alone cost {required_cost} tokens over the \
             {token_budget}-token budget; compilation fails closed"
        )));
    }

    // Optional sections in drop order: ascending priority, ties by id.
    let mut drop_order: Vec<usize> = sections
        .iter()
        .enumerate()
        .filter(|(_, section)| !section.required)
        .map(|(index, _)| index)
        .collect();
    drop_order.sort_by(|&a, &b| {
        sections[a]
            .priority
            .cmp(&sections[b].priority)
            .then_with(|| sections[a].id.cmp(&sections[b].id))
    });

    let mut total: u64 = sections
        .iter()
        .map(|section| accountant.count_message(&section.content))
        .sum();
    let mut dropped_indices = Vec::new();
    let mut drop_queue = drop_order.into_iter();
    while total > token_budget {
        let Some(index) = drop_queue.next() else {
            // Unreachable: required-only cost already fit.
            return Err(SdkError::Domain(
                "budget fitting exhausted optional sections without fitting".into(),
            ));
        };
        total -= accountant.count_message(&sections[index].content);
        dropped_indices.push(index);
    }

    let dropped: Vec<PromptSectionId> = dropped_indices
        .iter()
        .map(|&index| sections[index].id.clone())
        .collect();
    let kept = sections
        .into_iter()
        .enumerate()
        .filter(|(index, _)| !dropped_indices.contains(index))
        .map(|(_, section)| section)
        .collect();
    Ok(BudgetFit { kept, dropped })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::section::{PromptMessageRole, PromptSectionVersion};

    fn section(id: &str, required: bool, priority: u16, bytes: usize) -> RenderedSection {
        RenderedSection {
            id: PromptSectionId(id.into()),
            version: PromptSectionVersion(1),
            role: PromptMessageRole::System,
            required,
            priority,
            content_hash: format!("sha256:{id}"),
            content: "x".repeat(bytes),
        }
    }

    /// The spec's worked example: at a 1,800-token budget the priority-40
    /// optional drops first and the priority-60 optional then fits.
    #[test]
    fn the_spec_worked_example_drops_only_repository_hints() {
        let accountant = TokenAccountantRef::approx_bytes_v1();
        // Bytes chosen so count_message ≈ the spec's token column.
        let sections = vec![
            section("s/role", true, 0, 336),                 // ~120
            section("s/tool_protocol", true, 0, 1_896),      // ~640
            section("s/correction", true, 0, 2_076),         // ~700
            section("s/repository_hints", false, 40, 1_356), // ~460
            section("s/style_guidance", false, 60, 636),     // ~220
            section("s/output_contract", true, 0, 306),      // ~110
        ];
        let fit = fit_budget(sections, &accountant, 1_800).unwrap();
        assert_eq!(
            fit.dropped,
            vec![PromptSectionId("s/repository_hints".into())]
        );
        let kept: Vec<&str> = fit.kept.iter().map(|s| s.id.0.as_str()).collect();
        assert_eq!(
            kept,
            [
                "s/role",
                "s/tool_protocol",
                "s/correction",
                "s/style_guidance",
                "s/output_contract"
            ]
        );
    }

    #[test]
    fn required_overflow_fails_closed_and_ties_break_by_id() {
        let accountant = TokenAccountantRef::approx_bytes_v1();
        assert!(fit_budget(vec![section("s/huge", true, 0, 30_000)], &accountant, 100).is_err());

        let sections = vec![
            section("s/b", false, 10, 3_000),
            section("s/a", false, 10, 3_000),
            section("s/base", true, 0, 30),
        ];
        let fit = fit_budget(sections, &accountant, 1_200).unwrap();
        assert_eq!(fit.dropped.first().unwrap().0, "s/a", "ties break by id");
    }

    #[test]
    fn fitting_is_deterministic() {
        let accountant = TokenAccountantRef::approx_bytes_v1();
        let sections = || {
            vec![
                section("s/base", true, 0, 300),
                section("s/opt1", false, 20, 900),
                section("s/opt2", false, 30, 900),
            ]
        };
        let first = fit_budget(sections(), &accountant, 450).unwrap();
        let second = fit_budget(sections(), &accountant, 450).unwrap();
        assert_eq!(first, second);
    }
}
