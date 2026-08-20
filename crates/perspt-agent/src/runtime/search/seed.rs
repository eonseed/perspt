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
}
