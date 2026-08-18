//! Resident context as paged memory (PSP-10 Definition 6, system 24).
//!
//! The conversation projection is the backing store, not the prompt. Pages
//! are immutable and content-addressed; the mandatory dependency closure is
//! pinned; optional synopsis frames are chosen by deterministic greedy
//! weighted coverage (submodular, so greedy earns the `1 - 1/e` factor on
//! the declared labels — a claim about labels, never about sufficiency).
//! An infeasible mandatory closure makes no model call.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::canon::CanonicalEncoder;
use crate::error::{Result, SdkError};

use super::section::PROMPT_DIGEST_TAG;

/// The explicit state a page depends on. The assembler checks it against
/// the current branch before making the page resident; a state change
/// invalidates the page, it never silently rewrites it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateDependency {
    SessionInvariant,
    AcceptedRoot(String),
    PartialCheckpointRoot(String),
    PathBlob { path: String, blob_hash: String },
}

/// One immutable, content-addressed context page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextPage {
    /// Content address of the page bytes.
    pub page_id: String,
    /// Page kind (task contract, tool pair, synopsis, source excerpt, …).
    pub kind: String,
    /// Hashes of the exact sources a summary names; empty for exact pages.
    pub source_hashes: Vec<String>,
    pub dependency: StateDependency,
    pub bytes: u64,
    /// Token cost under the route's named accountant.
    pub tokens: u64,
    /// Typed obligation labels this page covers. Labels derive from typed
    /// task/path/symbol/diagnostic/test/capability/provenance fields —
    /// never from model prose.
    pub obligations: Vec<String>,
    /// The turn this page was last referenced (working-set recency).
    pub freshness_turn: u64,
    /// Pages this page requires resident with it (e.g. a tool call and its
    /// result form one atomic pair).
    pub depends_on: Vec<String>,
}

/// The live state pages are validated against.
#[derive(Debug, Clone, Default)]
pub struct DependencyEnv {
    pub accepted_root: String,
    pub partial_root: Option<String>,
    /// Current blob hash per path.
    pub path_blobs: BTreeMap<String, String>,
}

impl ContextPage {
    /// Whether this page's declared dependency still holds.
    pub fn dependency_holds(&self, env: &DependencyEnv) -> bool {
        match &self.dependency {
            StateDependency::SessionInvariant => true,
            StateDependency::AcceptedRoot(root) => *root == env.accepted_root,
            StateDependency::PartialCheckpointRoot(root) => {
                env.partial_root.as_deref() == Some(root.as_str())
            }
            StateDependency::PathBlob { path, blob_hash } => {
                env.path_blobs.get(path) == Some(blob_hash)
            }
        }
    }
}

/// Definition 6's reserves over one route's context window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextBudget {
    pub window_tokens: u64,
    pub output_reserve: u64,
    pub tool_reserve: u64,
    pub guard_reserve: u64,
}

impl ContextBudget {
    /// `β_in = β_win − β_out − β_tool − β_guard`; fails when the reserves
    /// alone exceed the window.
    pub fn input_allowance(&self) -> Result<u64> {
        self.window_tokens
            .checked_sub(self.output_reserve)
            .and_then(|rest| rest.checked_sub(self.tool_reserve))
            .and_then(|rest| rest.checked_sub(self.guard_reserve))
            .ok_or_else(|| SdkError::Domain("context reserves exceed the window".into()))
    }
}

/// The assembled resident set for one model request, or the recorded
/// refusal when the mandatory closure cannot fit.
#[derive(Debug, Clone, PartialEq)]
pub enum ResidentOutcome {
    Assembled(ResidentContext),
    /// `ContextBudgetInfeasible`: no model call is made.
    Infeasible {
        required: u64,
        allowance: u64,
    },
}

/// The resident pages of one request, in their ledgered order.
#[derive(Debug, Clone, PartialEq)]
pub struct ResidentContext {
    /// Mandatory closure first, then selected optional frames.
    pub pages: Vec<ContextPage>,
    /// Count of mandatory pages at the head of `pages`.
    pub mandatory_len: usize,
    pub resident_digest: String,
}

/// Compute the mandatory dependency closure over `pages` starting from the
/// `mandatory` ids, following `depends_on` transitively. Order: the input
/// page order, restricted to closure members (deterministic).
pub fn mandatory_closure(pages: &[ContextPage], mandatory: &[String]) -> Vec<ContextPage> {
    let by_id: BTreeMap<&str, &ContextPage> = pages
        .iter()
        .map(|page| (page.page_id.as_str(), page))
        .collect();
    let mut wanted: BTreeSet<String> = mandatory.iter().cloned().collect();
    let mut frontier: Vec<String> = mandatory.to_vec();
    while let Some(id) = frontier.pop() {
        if let Some(page) = by_id.get(id.as_str()) {
            for dependency in &page.depends_on {
                if wanted.insert(dependency.clone()) {
                    frontier.push(dependency.clone());
                }
            }
        }
    }
    pages
        .iter()
        .filter(|page| wanted.contains(&page.page_id))
        .cloned()
        .collect()
}

/// Deterministic greedy weighted-coverage selection of at most `slots`
/// synopsis frames. Ties break by page id (the content hash), so identical
/// inputs select identically.
pub fn select_working_set(
    candidates: &[ContextPage],
    weights: &BTreeMap<String, f64>,
    slots: usize,
) -> Vec<ContextPage> {
    let mut covered: BTreeSet<&str> = BTreeSet::new();
    let mut chosen_ids: BTreeSet<&str> = BTreeSet::new();
    let mut chosen = Vec::new();
    for _ in 0..slots {
        let mut best: Option<(&ContextPage, f64)> = None;
        for page in candidates {
            if chosen_ids.contains(page.page_id.as_str()) {
                continue;
            }
            let gain: f64 = page
                .obligations
                .iter()
                .filter(|label| !covered.contains(label.as_str()))
                .map(|label| weights.get(label).copied().unwrap_or(0.0))
                .sum();
            let better = match &best {
                None => true,
                Some((current, current_gain)) => {
                    gain > *current_gain
                        || (gain == *current_gain && page.page_id < current.page_id)
                }
            };
            if better {
                best = Some((page, gain));
            }
        }
        let Some((page, gain)) = best else { break };
        if gain <= 0.0 {
            // No remaining frame improves declared coverage; unused space
            // is allowed.
            break;
        }
        chosen_ids.insert(page.page_id.as_str());
        covered.extend(page.obligations.iter().map(String::as_str));
        chosen.push(page.clone());
    }
    chosen
}

/// Assemble the resident context per Definition 6.
///
/// `instruction_tokens` is `c(I_t)` for the compiled programs; `frame_tokens`
/// is the synopsis frame bound `q`. Every candidate offered for selection
/// must already respect `q`; pages whose dependency fails are excluded
/// before selection.
pub fn assemble_resident(
    pages: &[ContextPage],
    mandatory_ids: &[String],
    env: &DependencyEnv,
    budget: &ContextBudget,
    instruction_tokens: u64,
    frame_tokens: u64,
    weights: &BTreeMap<String, f64>,
) -> Result<ResidentOutcome> {
    let allowance = budget.input_allowance()?;
    let mandatory = mandatory_closure(pages, mandatory_ids);
    let mandatory_cost: u64 = mandatory.iter().map(|page| page.tokens).sum();
    if instruction_tokens + mandatory_cost > allowance {
        return Ok(ResidentOutcome::Infeasible {
            required: instruction_tokens + mandatory_cost,
            allowance,
        });
    }
    let remaining = allowance - instruction_tokens - mandatory_cost;
    let slots = remaining.checked_div(frame_tokens).unwrap_or(0) as usize;
    let mandatory_ids_set: BTreeSet<&str> =
        mandatory.iter().map(|page| page.page_id.as_str()).collect();
    let candidates: Vec<ContextPage> = pages
        .iter()
        .filter(|page| {
            !mandatory_ids_set.contains(page.page_id.as_str())
                && page.dependency_holds(env)
                && page.tokens <= frame_tokens
        })
        .cloned()
        .collect();
    let selected = select_working_set(&candidates, weights, slots);
    let mut resident = mandatory;
    let mandatory_len = resident.len();
    resident.extend(selected);
    let mut encoder = CanonicalEncoder::new(PROMPT_DIGEST_TAG);
    encoder.text("resident-context");
    encoder.list(resident.iter().map(|page| page.page_id.as_str()));
    let resident_digest = encoder.digest();
    Ok(ResidentOutcome::Assembled(ResidentContext {
        pages: resident,
        mandatory_len,
        resident_digest,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic SplitMix64 for seeded property tests (no external
    /// dependency; failing seeds print for reproduction).
    struct SplitMix64(u64);

    impl SplitMix64 {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
    }

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

    fn env() -> DependencyEnv {
        DependencyEnv {
            accepted_root: "root-a".into(),
            partial_root: None,
            path_blobs: BTreeMap::new(),
        }
    }

    #[test]
    fn randomized_page_sets_never_exceed_the_input_allowance() {
        for seed in 0..64u64 {
            let mut rng = SplitMix64(seed);
            let budget = ContextBudget {
                window_tokens: 2_000,
                output_reserve: 400,
                tool_reserve: 100,
                guard_reserve: 100,
            };
            let allowance = budget.input_allowance().unwrap();
            let frame = 64;
            let mut pages = vec![page(
                "m0",
                (rng.next() % 300) + 1,
                &["task"],
                StateDependency::SessionInvariant,
            )];
            for index in 0..(rng.next() % 20) {
                pages.push(page(
                    &format!("p{index}"),
                    (rng.next() % frame) + 1,
                    &["a", "b"],
                    StateDependency::AcceptedRoot("root-a".into()),
                ));
            }
            let weights: BTreeMap<String, f64> =
                [("a".to_string(), 1.0), ("b".to_string(), 0.5)].into();
            let instruction = rng.next() % 500;
            let outcome = assemble_resident(
                &pages,
                &["m0".to_string()],
                &env(),
                &budget,
                instruction,
                frame,
                &weights,
            )
            .unwrap();
            if let ResidentOutcome::Assembled(resident) = outcome {
                let total: u64 = resident.pages.iter().map(|p| p.tokens).sum();
                assert!(
                    instruction + total <= allowance,
                    "seed {seed}: {instruction} + {total} > {allowance}"
                );
            }
        }
    }

    #[test]
    fn infeasible_mandatory_closure_refuses_before_any_call() {
        let budget = ContextBudget {
            window_tokens: 500,
            output_reserve: 100,
            tool_reserve: 50,
            guard_reserve: 50,
        };
        let pages = vec![page(
            "m0",
            400,
            &["task"],
            StateDependency::SessionInvariant,
        )];
        let outcome = assemble_resident(
            &pages,
            &["m0".to_string()],
            &env(),
            &budget,
            0,
            64,
            &BTreeMap::new(),
        )
        .unwrap();
        assert!(matches!(outcome, ResidentOutcome::Infeasible { .. }));
    }

    #[test]
    fn root_dependent_pages_invalidate_on_root_change() {
        let stale = page(
            "p1",
            10,
            &["a"],
            StateDependency::AcceptedRoot("old".into()),
        );
        let fresh = page(
            "p2",
            10,
            &["a"],
            StateDependency::AcceptedRoot("root-a".into()),
        );
        let blob = page(
            "p3",
            10,
            &["b"],
            StateDependency::PathBlob {
                path: "src/lib.rs".into(),
                blob_hash: "h1".into(),
            },
        );
        let mut environment = env();
        assert!(!stale.dependency_holds(&environment));
        assert!(fresh.dependency_holds(&environment));
        assert!(!blob.dependency_holds(&environment));
        environment
            .path_blobs
            .insert("src/lib.rs".into(), "h1".into());
        assert!(blob.dependency_holds(&environment));
        environment
            .path_blobs
            .insert("src/lib.rs".into(), "h2".into());
        assert!(!blob.dependency_holds(&environment));
    }

    #[test]
    fn greedy_selection_matches_exhaustive_optimum_on_small_fixtures() {
        // Coverage is submodular; on tiny instances greedy must equal the
        // exhaustive optimum for K = 1 and be within the guarantee for
        // K = 2. We assert exact equality on a fixture where greedy is
        // optimal by construction.
        let weights: BTreeMap<String, f64> = [
            ("a".to_string(), 3.0),
            ("b".to_string(), 2.0),
            ("c".to_string(), 2.0),
        ]
        .into();
        let candidates = vec![
            page("p1", 10, &["a"], StateDependency::SessionInvariant),
            page("p2", 10, &["b", "c"], StateDependency::SessionInvariant),
            page("p3", 10, &["b"], StateDependency::SessionInvariant),
        ];
        let one = select_working_set(&candidates, &weights, 1);
        assert_eq!(one[0].page_id, "p2", "gain 4.0 beats 3.0");
        let two = select_working_set(&candidates, &weights, 2);
        let ids: Vec<&str> = two.iter().map(|p| p.page_id.as_str()).collect();
        assert_eq!(ids, ["p2", "p1"], "optimum coverage 7.0");
        // Determinism.
        assert_eq!(select_working_set(&candidates, &weights, 2), two);
    }

    #[test]
    fn tool_pairs_stay_atomic_through_the_mandatory_closure() {
        let mut call = page("call1", 10, &[], StateDependency::SessionInvariant);
        call.depends_on = vec!["result1".to_string()];
        let result = page("result1", 10, &[], StateDependency::SessionInvariant);
        let closure = mandatory_closure(&[call, result], &["call1".to_string()]);
        let ids: Vec<&str> = closure.iter().map(|p| p.page_id.as_str()).collect();
        assert_eq!(ids, ["call1", "result1"]);
    }

    #[test]
    fn resident_digest_is_deterministic_over_page_order() {
        let pages = vec![
            page("m0", 10, &["task"], StateDependency::SessionInvariant),
            page(
                "p1",
                10,
                &["a"],
                StateDependency::AcceptedRoot("root-a".into()),
            ),
        ];
        let weights: BTreeMap<String, f64> = [("a".to_string(), 1.0)].into();
        let budget = ContextBudget {
            window_tokens: 1_000,
            output_reserve: 100,
            tool_reserve: 50,
            guard_reserve: 50,
        };
        let run = || {
            assemble_resident(
                &pages,
                &["m0".to_string()],
                &env(),
                &budget,
                0,
                64,
                &weights,
            )
            .unwrap()
        };
        assert_eq!(run(), run());
    }
}
