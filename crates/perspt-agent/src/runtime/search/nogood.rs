//! Exact no-goods (PSP-10 system 21, Gate AB).
//!
//! A no-good suppresses only an exact repeated attempt: the lookup key
//! `K_ng` hashes every component under the canonical-bytes discipline, and
//! a lookup matches only when every component is equal. No path prefix,
//! operator class, residual class, model interpretation, or
//! natural-language similarity may create or match a no-good.

use std::collections::BTreeMap;

use perspt_sdk::canon::CanonicalEncoder;

/// The components of one exact no-good key (spec: `K_ng = H(tag, r, d, s,
/// q, p, c, t, v, b)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoGoodComponents {
    /// `r`: the accepted root.
    pub accepted_root: String,
    /// `d`: the domain-manifest digest.
    pub domain_digest: String,
    /// `s`: the strategy digest.
    pub strategy_digest: String,
    /// `q`: the prompt-program digest.
    pub prompt_digest: String,
    /// `p`: the canonical proposal digest (goal + correction state).
    pub proposal_digest: String,
    /// `c`: the capability-grant digest.
    pub grant_digest: String,
    /// `t`: the effective tool-catalog digest.
    pub catalog_digest: String,
    /// `v`: the required-sensor fingerprint.
    pub sensor_fingerprint: String,
    /// `b`: the runtime-build digest.
    pub build_digest: String,
}

impl NoGoodComponents {
    /// The exact key `K_ng`.
    pub fn key(&self) -> String {
        let mut encoder = CanonicalEncoder::new(b"perspt.no-good.v1");
        encoder
            .text(&self.accepted_root)
            .text(&self.domain_digest)
            .text(&self.strategy_digest)
            .text(&self.prompt_digest)
            .text(&self.proposal_digest)
            .text(&self.grant_digest)
            .text(&self.catalog_digest)
            .text(&self.sensor_fingerprint)
            .text(&self.build_digest);
        encoder.digest()
    }
}

/// Deterministic support classes admitted into the store (Gate AB). Model
/// interpretation is only an observation; it cannot enter.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // ContractDiagnostic/DeniedCapability arrive with richer triggers.
pub(crate) enum NoGoodSupport {
    CompilerCode(String),
    FailedTest(String),
    ContractDiagnostic(String),
    UnchangedStateHash(String),
    DeniedCapability(String),
}

impl NoGoodSupport {
    /// The Gate AB support-class label, ledgered beside the evidence hash.
    pub fn kind(&self) -> &'static str {
        match self {
            NoGoodSupport::CompilerCode(_) => "compiler-code",
            NoGoodSupport::FailedTest(_) => "failed-test",
            NoGoodSupport::ContractDiagnostic(_) => "contract-diagnostic",
            NoGoodSupport::UnchangedStateHash(_) => "unchanged-state",
            NoGoodSupport::DeniedCapability(_) => "denied-capability",
        }
    }

    pub fn evidence_hash(&self) -> String {
        let (kind, value) = match self {
            NoGoodSupport::CompilerCode(value) => ("compiler-code", value),
            NoGoodSupport::FailedTest(value) => ("failed-test", value),
            NoGoodSupport::ContractDiagnostic(value) => ("contract-diagnostic", value),
            NoGoodSupport::UnchangedStateHash(value) => ("unchanged-state", value),
            NoGoodSupport::DeniedCapability(value) => ("denied-capability", value),
        };
        let mut encoder = CanonicalEncoder::new(b"perspt.no-good.v1");
        encoder.text("evidence").text(kind).text(value);
        encoder.digest()
    }
}

/// The in-memory exact-key store, folded from `no_good_recorded` events.
#[derive(Debug, Default)]
pub(crate) struct NoGoodStore {
    entries: BTreeMap<String, String>,
}

impl NoGoodStore {
    /// Record one no-good with deterministic support.
    pub fn record(&mut self, components: &NoGoodComponents, support: &NoGoodSupport) -> String {
        let key = components.key();
        self.entries.insert(key.clone(), support.evidence_hash());
        key
    }

    /// Fold one ledgered `no_good_recorded` entry back into the store
    /// (resume). Stale-root entries are harmless: `K_ng` embeds the
    /// accepted root, so they simply never match.
    pub fn fold_entry(&mut self, key: String, evidence_hash: String) {
        self.entries.insert(key, evidence_hash);
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Exact-equality lookup: any changed component permits a new attempt.
    pub fn suppresses(&self, components: &NoGoodComponents) -> bool {
        self.entries.contains_key(&components.key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn components() -> NoGoodComponents {
        NoGoodComponents {
            accepted_root: "root-a".into(),
            domain_digest: "d1".into(),
            strategy_digest: "s1".into(),
            prompt_digest: "q1".into(),
            proposal_digest: "p1".into(),
            grant_digest: "c1".into(),
            catalog_digest: "t1".into(),
            sensor_fingerprint: "v1".into(),
            build_digest: "b1".into(),
        }
    }

    #[test]
    fn suppression_requires_equality_of_every_component() {
        let mut store = NoGoodStore::default();
        store.record(&components(), &NoGoodSupport::CompilerCode("E0432".into()));
        assert!(store.suppresses(&components()));
        // Any single changed component permits a new attempt.
        for change in 0..9 {
            let mut changed = components();
            match change {
                0 => changed.accepted_root = "root-b".into(),
                1 => changed.domain_digest = "d2".into(),
                2 => changed.strategy_digest = "s2".into(),
                3 => changed.prompt_digest = "q2".into(),
                4 => changed.proposal_digest = "p2".into(),
                5 => changed.grant_digest = "c2".into(),
                6 => changed.catalog_digest = "t2".into(),
                7 => changed.sensor_fingerprint = "v2".into(),
                _ => changed.build_digest = "b2".into(),
            }
            assert!(!store.suppresses(&changed), "component {change}");
        }
    }
}
