//! Resident-context mechanism checks (PSP-10 Definition 6, Gate AF;
//! Phase 9).
//!
//! Requests never exceed the input allowance; an infeasible mandatory
//! closure makes no model call (`mc_actors` proves the zero-transport-call
//! half); tool-call/result pairs stay atomic through the mandatory
//! closure; optional selection is deterministic and matches the
//! exhaustive optimum on small weighted-coverage fixtures; root-dependent
//! pages invalidate on root change.

use std::collections::BTreeMap;

use perspt_sdk::prompt::{
    assemble_resident, mandatory_closure, select_working_set, ContextBudget, ContextPage,
    DependencyEnv, ResidentOutcome, StateDependency,
};

fn page(id: &str, tokens: u64, labels: &[&str], dependency: StateDependency) -> ContextPage {
    ContextPage {
        page_id: id.into(),
        kind: "synopsis".into(),
        source_hashes: vec![],
        dependency,
        bytes: tokens * 3,
        tokens,
        obligations: labels.iter().map(|label| label.to_string()).collect(),
        freshness_turn: 0,
        depends_on: vec![],
    }
}

fn env(root: &str) -> DependencyEnv {
    DependencyEnv {
        accepted_root: root.into(),
        partial_root: None,
        path_blobs: BTreeMap::new(),
    }
}

/// Gate AF: for every assembled outcome, instruction plus resident tokens
/// fit the allowance; assembly is deterministic (identical inputs,
/// identical page order and digest).
#[test]
fn assembly_fits_the_allowance_and_is_deterministic() {
    let budget = ContextBudget {
        window_tokens: 1_000,
        output_reserve: 200,
        tool_reserve: 100,
        guard_reserve: 100,
    };
    let allowance = budget.input_allowance().unwrap();
    let pages = vec![
        page("m0", 120, &["task"], StateDependency::SessionInvariant),
        page(
            "p1",
            60,
            &["a"],
            StateDependency::AcceptedRoot("root".into()),
        ),
        page(
            "p2",
            60,
            &["b"],
            StateDependency::AcceptedRoot("root".into()),
        ),
        page(
            "p3",
            60,
            &["a", "b"],
            StateDependency::AcceptedRoot("root".into()),
        ),
    ];
    let weights: BTreeMap<String, f64> = [("a".to_string(), 1.0), ("b".to_string(), 1.0)].into();
    let run = || {
        assemble_resident(
            &pages,
            &["m0".to_string()],
            &env("root"),
            &budget,
            100,
            64,
            &weights,
        )
        .unwrap()
    };
    let ResidentOutcome::Assembled(resident) = run() else {
        panic!("feasible closure must assemble");
    };
    let total: u64 = resident.pages.iter().map(|page| page.tokens).sum();
    assert!(100 + total <= allowance);
    assert_eq!(run(), run(), "identical inputs select identically");
    assert!(resident.resident_digest.starts_with("sha256:"));
}

/// An infeasible mandatory closure refuses before any call.
#[test]
fn infeasible_mandatory_closure_refuses() {
    let budget = ContextBudget {
        window_tokens: 400,
        output_reserve: 100,
        tool_reserve: 50,
        guard_reserve: 50,
    };
    let pages = vec![page(
        "m0",
        500,
        &["task"],
        StateDependency::SessionInvariant,
    )];
    let outcome = assemble_resident(
        &pages,
        &["m0".to_string()],
        &env("root"),
        &budget,
        0,
        64,
        &BTreeMap::new(),
    )
    .unwrap();
    assert!(matches!(
        outcome,
        ResidentOutcome::Infeasible { required: 500, .. }
    ));
}

/// Tool-call/result pairs are one atomic unit through the closure.
#[test]
fn tool_pairs_are_atomic() {
    let mut call = page("call", 10, &[], StateDependency::SessionInvariant);
    call.depends_on = vec!["result".to_string()];
    let result = page("result", 10, &[], StateDependency::SessionInvariant);
    let closure = mandatory_closure(&[call, result], &["call".to_string()]);
    assert_eq!(closure.len(), 2, "a call never appears without its result");
}

/// Greedy selection equals the exhaustive optimum on a small
/// weighted-coverage fixture, and stale root-dependent pages are excluded
/// before selection.
#[test]
fn greedy_matches_exhaustive_and_stale_pages_are_excluded() {
    let weights: BTreeMap<String, f64> = [
        ("a".to_string(), 3.0),
        ("b".to_string(), 2.0),
        ("c".to_string(), 2.0),
        ("d".to_string(), 1.0),
    ]
    .into();
    let candidates = vec![
        page("p1", 10, &["a", "d"], StateDependency::SessionInvariant),
        page("p2", 10, &["b", "c"], StateDependency::SessionInvariant),
        page("p3", 10, &["a"], StateDependency::SessionInvariant),
        page("p4", 10, &["c", "d"], StateDependency::SessionInvariant),
    ];
    // Exhaustive optimum for K = 2: {p1, p2} covers a, b, c, d = 8.0.
    let coverage = |ids: &[&str]| -> f64 {
        let mut covered = std::collections::BTreeSet::new();
        for id in ids {
            let page = candidates.iter().find(|page| page.page_id == *id).unwrap();
            covered.extend(page.obligations.iter().cloned());
        }
        covered.iter().map(|label| weights[label]).sum()
    };
    let mut best = ("", "", 0.0);
    for (i, a) in ["p1", "p2", "p3", "p4"].iter().enumerate() {
        for b in ["p1", "p2", "p3", "p4"].iter().skip(i + 1) {
            let value = coverage(&[a, b]);
            if value > best.2 {
                best = (a, b, value);
            }
        }
    }
    let selected = select_working_set(&candidates, &weights, 2);
    let ids: Vec<&str> = selected.iter().map(|page| page.page_id.as_str()).collect();
    let greedy_value = coverage(&ids);
    assert_eq!(greedy_value, best.2, "greedy equals the optimum here");

    // A stale root-dependent page never becomes resident.
    let stale = page(
        "p9",
        10,
        &["a"],
        StateDependency::AcceptedRoot("old".into()),
    );
    let pages = vec![
        page("m0", 10, &["task"], StateDependency::SessionInvariant),
        stale,
    ];
    let budget = ContextBudget {
        window_tokens: 1_000,
        output_reserve: 100,
        tool_reserve: 50,
        guard_reserve: 50,
    };
    let outcome = assemble_resident(
        &pages,
        &["m0".to_string()],
        &env("root"),
        &budget,
        0,
        64,
        &weights,
    )
    .unwrap();
    let ResidentOutcome::Assembled(resident) = outcome else {
        panic!("feasible");
    };
    assert!(
        resident.pages.iter().all(|page| page.page_id != "p9"),
        "stale page admitted"
    );
}
