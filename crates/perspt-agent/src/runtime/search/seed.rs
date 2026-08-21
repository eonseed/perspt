//! Durable search state routed to its exact owner on resume.

use super::nogood::NoGoodStore;

/// Prior state folded from the ledger: exact no-goods plus the one
/// interrupted forest's identity and consumption.
pub(crate) struct SearchSeed {
    pub no_goods: NoGoodStore,
    pub prior_usage: Option<perspt_sdk::SearchUsage>,
    pub resumed_from: Option<String>,
    pub node_id: Option<String>,
    pub generation: Option<u32>,
    pub accepted_root: Option<String>,
}

impl SearchSeed {
    pub(super) fn owns(&self, node_id: &str, generation: u32, accepted_root: &str) -> bool {
        match (&self.node_id, self.generation, &self.accepted_root) {
            (Some(node), Some(candidate_generation), Some(root)) => {
                node == node_id && candidate_generation == generation && root == accepted_root
            }
            // Without an interrupted forest, exact no-goods may seed the
            // first forest: their keys include node/generation/root state.
            (None, None, None) => true,
            _ => false,
        }
    }
}

/// A forest's opening claim on the resumed seed. The folded no-goods stay
/// in the slot and seed **every** forest in the session; the interrupted
/// forest's consumption and `resumed_from` link are claimed exactly once,
/// and only by the exact owner. The final component is the interrupted
/// forest id when this opening bypasses an unclaimed consumption (fresh
/// budget while the interrupted usage stays unabsorbed), so the caller can
/// ledger the bypass.
pub(super) fn claim(
    seed_slot: &mut Option<SearchSeed>,
    limits: &perspt_sdk::SearchLimits,
    node_id: &str,
    generation: u32,
    accepted_root: &str,
) -> (
    Option<String>,
    NoGoodStore,
    perspt_sdk::search::SharedSearchBudget,
    Option<String>,
) {
    let fresh = || perspt_sdk::search::SharedSearchBudget::new(limits.clone());
    let Some(seed) = seed_slot.as_mut() else {
        return (None, NoGoodStore::default(), fresh(), None);
    };
    let no_goods = seed.no_goods.clone();
    if !seed.owns(node_id, generation, accepted_root) {
        let bypassed = seed
            .prior_usage
            .is_some()
            .then(|| seed.resumed_from.clone())
            .flatten();
        return (None, no_goods, fresh(), bypassed);
    }
    let resumed_from = seed.resumed_from.take();
    let prior_usage = seed.prior_usage.take();
    seed.node_id = None;
    seed.generation = None;
    seed.accepted_root = None;
    let budget = match prior_usage {
        Some(usage) => perspt_sdk::search::SharedSearchBudget::with_usage(limits.clone(), usage),
        None => fresh(),
    };
    (resumed_from, no_goods, budget, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupted_usage_is_bound_to_node_generation_and_root() {
        let seed = SearchSeed {
            no_goods: NoGoodStore::default(),
            prior_usage: Some(perspt_sdk::SearchUsage::default()),
            resumed_from: Some("forest-a".into()),
            node_id: Some("node-a".into()),
            generation: Some(3),
            accepted_root: Some("root-a".into()),
        };
        assert!(seed.owns("node-a", 3, "root-a"));
        assert!(!seed.owns("node-b", 3, "root-a"));
        assert!(!seed.owns("node-a", 4, "root-a"));
        assert!(!seed.owns("node-a", 3, "root-b"));
    }

    #[test]
    fn folded_no_goods_survive_for_every_forest_after_the_claim() {
        let mut no_goods = NoGoodStore::default();
        no_goods.fold_entry("exact-key".into(), "evidence".into());
        let mut slot = Some(SearchSeed {
            no_goods,
            prior_usage: Some(perspt_sdk::SearchUsage::default()),
            resumed_from: Some("forest-a".into()),
            node_id: Some("node-a".into()),
            generation: Some(3),
            accepted_root: Some("root-a".into()),
        });
        let limits = perspt_sdk::SearchLimits::release_default();

        // A non-owner opening first: no-goods yes, usage no, bypass noted.
        let (resumed, store, _, bypassed) = claim(&mut slot, &limits, "node-b", 0, "root-b");
        assert_eq!(resumed, None);
        assert_eq!(store.len(), 1);
        assert_eq!(bypassed.as_deref(), Some("forest-a"));

        // The exact owner claims the interrupted consumption and the link.
        let (resumed, store, _, bypassed) = claim(&mut slot, &limits, "node-a", 3, "root-a");
        assert_eq!(resumed.as_deref(), Some("forest-a"));
        assert_eq!(store.len(), 1);
        assert_eq!(bypassed, None);

        // Every later forest still folds the recorded no-goods, and the
        // already-claimed consumption is neither doubled nor "bypassed".
        let (resumed, store, _, bypassed) = claim(&mut slot, &limits, "node-c", 1, "root-c");
        assert_eq!(resumed, None);
        assert_eq!(store.len(), 1);
        assert_eq!(bypassed, None);
    }
}
