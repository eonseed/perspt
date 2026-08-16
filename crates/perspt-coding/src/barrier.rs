//! The coding operational safety barrier (PSP-9 system 12).
//!
//! A narrow, useful barrier rather than a pretend solution to semantic
//! correctness: versioned policy channels `h_j(x) ∈ [0, ∞)` whose unsafe
//! boundary is 1, with `h(x) = max_j h_j(x)` and the exact deterministic
//! increment `c_t = max(0, h(x') − h(x))`. Correctness evidence — compiler
//! errors, test failures — stays in `V` and is never duplicated into `h`.
//!
//! The old shortcut "deterministic transition, therefore `c_c = 0`" is
//! prohibited: read-only effects get `c_t = 0` by *measurement* (no channel
//! moves), not by declaration.

use perspt_sdk::{
    BarrierEvaluator, BarrierWitness, CandidateTransition, EffectKind, EffectProposal, SdkError,
};

/// Channel verdict: the channel's value for a candidate transition.
#[derive(Debug, Clone, PartialEq)]
struct ChannelReading {
    name: &'static str,
    h_after: f64,
}

/// The versioned channel set for the coding domain.
#[derive(Debug, Clone)]
pub struct OperationalSafetyBarrier {
    /// Path prefixes that must never be modified.
    pub protected_paths: Vec<String>,
    /// Filename fragments that indicate secret material.
    pub secret_markers: Vec<String>,
    /// Allow-listed network hosts; empty means every target is unapproved.
    pub network_allowlist: Vec<String>,
}

impl Default for OperationalSafetyBarrier {
    fn default() -> Self {
        Self {
            protected_paths: vec![".git/".into(), ".github/workflows/".into()],
            secret_markers: vec![
                ".env".into(),
                ".pem".into(),
                "id_rsa".into(),
                "credentials".into(),
                ".aws/".into(),
                ".ssh/".into(),
            ],
            network_allowlist: Vec::new(),
        }
    }
}

impl OperationalSafetyBarrier {
    /// Evaluate every channel against the proposal's candidate transition.
    fn readings(&self, proposal: &EffectProposal) -> Vec<ChannelReading> {
        let path = proposal.path.as_deref().unwrap_or("");
        let mutating = !proposal.effect.is_read_only();
        let touches =
            |marker: &str| !path.is_empty() && path.contains(marker.trim_end_matches('/'));

        let protected = mutating && self.protected_paths.iter().any(|p| touches(p));
        // Sandbox escape: a mutating effect addressing an absolute path or a
        // parent traversal is outside the candidate overlay by construction.
        let escape = mutating && (path.starts_with('/') || path.split('/').any(|c| c == ".."));
        let secret = mutating && self.secret_markers.iter().any(|m| touches(m));
        let network = match proposal.network_target.as_deref() {
            None => false,
            Some(target) => !self
                .network_allowlist
                .iter()
                .any(|allowed| target.contains(allowed.as_str())),
        };
        let dependency =
            proposal.effect == EffectKind::MutateDependencies && proposal.command.is_none();

        let boolean = |unsafe_now: bool| if unsafe_now { 1.0 } else { 0.0 };
        vec![
            ChannelReading {
                name: "protected-path-modification",
                h_after: boolean(protected),
            },
            ChannelReading {
                name: "sandbox-escape",
                h_after: boolean(escape),
            },
            ChannelReading {
                name: "secret-exposure",
                h_after: boolean(secret),
            },
            ChannelReading {
                name: "unapproved-network-reachability",
                h_after: boolean(network),
            },
            ChannelReading {
                name: "dependency-policy-violation",
                h_after: boolean(dependency),
            },
            // Resource limits are enforced by the sandbox at execution; the
            // channel reports safe unless a limit is already breached.
            ChannelReading {
                name: "resource-limits",
                h_after: 0.0,
            },
        ]
    }
}

impl BarrierEvaluator for OperationalSafetyBarrier {
    fn evaluate(&self, transition: &CandidateTransition) -> Result<BarrierWitness, SdkError> {
        let proposal = &transition.proposal;
        let readings = self.readings(proposal);
        let h_before = transition
            .before
            .barrier_channels
            .values()
            .copied()
            .fold(0.0, f64::max);
        let measured_after = transition
            .after
            .barrier_channels
            .values()
            .copied()
            .fold(0.0, f64::max);
        let h_after = readings
            .iter()
            .map(|r| r.h_after)
            .fold(measured_after, f64::max);
        let c_t = (h_after - h_before).max(0.0);
        Ok(BarrierWitness {
            h_before,
            expected_h_after_upper: h_after,
            certified_increment: c_t,
            unsafe_threshold: 1.0,
            evidence_refs: readings
                .iter()
                .map(|r| format!("channel:{}@1={}", r.name, r.h_after))
                .chain([
                    format!("state-before:{}", transition.before.state_root),
                    format!("state-after:{}", transition.after.state_root),
                ])
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perspt_sdk::{ActorId, EffectProposal};

    fn proposal(effect: EffectKind, path: Option<&str>) -> EffectProposal {
        let mut p = EffectProposal::new(ActorId::new("worker"), "n1", effect);
        if let Some(path) = path {
            p = p.with_path(path);
        }
        p
    }

    fn transition(effect: EffectKind, path: Option<&str>) -> CandidateTransition {
        CandidateTransition::unmeasured(proposal(effect, path))
    }

    #[test]
    fn a_workspace_edit_has_zero_increment_by_measurement() {
        let barrier = OperationalSafetyBarrier::default();
        let witness = barrier
            .evaluate(&transition(EffectKind::ApplyPatch, Some("src/lib.rs")))
            .unwrap();
        assert_eq!(witness.certified_increment, 0.0);
        assert!(witness.clause_holds(0.0));
    }

    #[test]
    fn protected_path_modification_crosses_the_boundary() {
        let barrier = OperationalSafetyBarrier::default();
        let witness = barrier
            .evaluate(&transition(EffectKind::ApplyPatch, Some(".git/config")))
            .unwrap();
        // h(x') = 1 is at the unsafe boundary: the clause fails whatever the
        // budget, because promotion requires h(x') < 1.
        assert!(!witness.clause_holds(1.0));
    }

    #[test]
    fn reading_a_protected_path_is_not_a_barrier_event() {
        let barrier = OperationalSafetyBarrier::default();
        let witness = barrier
            .evaluate(&transition(EffectKind::ReadFile, Some(".git/config")))
            .unwrap();
        assert_eq!(witness.certified_increment, 0.0);
    }

    #[test]
    fn parent_traversal_is_a_sandbox_escape() {
        let barrier = OperationalSafetyBarrier::default();
        let witness = barrier
            .evaluate(&transition(
                EffectKind::WriteArtifact,
                Some("../outside.rs"),
            ))
            .unwrap();
        assert!(!witness.clause_holds(1.0));
    }

    #[test]
    fn secret_material_is_a_channel_violation() {
        let barrier = OperationalSafetyBarrier::default();
        let witness = barrier
            .evaluate(&transition(EffectKind::WriteArtifact, Some(".env")))
            .unwrap();
        assert!(!witness.clause_holds(1.0));
    }

    #[test]
    fn unallowlisted_network_target_violates() {
        let barrier = OperationalSafetyBarrier::default();
        let mut p = proposal(EffectKind::NetworkFetch, None);
        p = p.with_network_target("https://example.com/payload");
        let witness = barrier
            .evaluate(&CandidateTransition::unmeasured(p))
            .unwrap();
        assert!(!witness.clause_holds(1.0));
    }
}
