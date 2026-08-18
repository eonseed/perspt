//! Section manifests and measured prompt activation (PSP-10 system 25,
//! Gate AE).
//!
//! The base sections are the active baseline. An override whose rendered
//! text differs from its base starts `Experimental` and activates only
//! through a content-addressed [`PromptChangeRecord`] that passes the
//! sample, safety, noninferiority, and benefit rules. Route preference and
//! online statistics can never activate a section.

use serde::{Deserialize, Serialize};

use crate::canon::CanonicalEncoder;
use crate::error::{Result, SdkError};

use super::section::{PromptSectionId, PromptSectionVersion, PROMPT_DIGEST_TAG};

/// Gate AE lifecycle state of an override.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationState {
    /// Runs only under `--allow-experimental-prompts`, which is ledgered.
    Experimental,
    /// Activated by a passing, digest-bound change record.
    Active,
}

/// One manifest entry: a section or override's committed identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub id: PromptSectionId,
    pub version: PromptSectionVersion,
    pub stage: String,
    pub role: String,
    pub required: bool,
    pub priority: u16,
    pub max_bytes: usize,
    pub owner: String,
    pub content_hash: String,
    /// Present on overrides only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activation: Option<ActivationState>,
    /// The bound change-record digest, for `Active` overrides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change_record: Option<String>,
}

/// The committed, CLI-generated manifest of one section library.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PromptManifest {
    pub entries: Vec<ManifestEntry>,
}

impl PromptManifest {
    /// The manifest digest that enters session provenance.
    pub fn digest(&self) -> String {
        let mut encoder = CanonicalEncoder::new(PROMPT_DIGEST_TAG);
        encoder.text("manifest");
        for entry in &self.entries {
            encoder
                .text(&entry.id.0)
                .u64(u64::from(entry.version.0))
                .text(&entry.stage)
                .text(&entry.role)
                .bool(entry.required)
                .u64(u64::from(entry.priority))
                .u64(entry.max_bytes as u64)
                .text(&entry.owner)
                .text(&entry.content_hash)
                .text(match &entry.activation {
                    None => "base",
                    Some(ActivationState::Experimental) => "experimental",
                    Some(ActivationState::Active) => "active",
                })
                .text(entry.change_record.as_deref().unwrap_or(""));
        }
        encoder.digest()
    }
}

/// Configuration bounds for activation. The floor may only be raised and
/// the margin narrowed; invalid values fail at startup.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ActivationBounds {
    /// Minimum paired tasks; never below 30.
    pub min_tasks: u32,
    /// Noninferiority margin ε ∈ [0, 0.05].
    pub noninferiority_margin: f64,
}

impl Default for ActivationBounds {
    fn default() -> Self {
        Self {
            min_tasks: 30,
            noninferiority_margin: 0.05,
        }
    }
}

impl ActivationBounds {
    /// Startup validation of configured bounds.
    pub fn validate(&self) -> Result<()> {
        if self.min_tasks < 30 {
            return Err(SdkError::Domain(format!(
                "activation_min_tasks {} is below the 30-task floor",
                self.min_tasks
            )));
        }
        if !(0.0..=0.05).contains(&self.noninferiority_margin) {
            return Err(SdkError::Domain(format!(
                "noninferiority_margin {} outside [0, 0.05]",
                self.noninferiority_margin
            )));
        }
        Ok(())
    }
}

/// The number of paired percentile-bootstrap resamples Gate AE requires.
pub const ACTIVATION_BOOTSTRAP_RESAMPLES: u64 = 10_000;

/// The content-addressed evidence that activates one section override.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptChangeRecord {
    pub base_id: PromptSectionId,
    pub base_version: PromptSectionVersion,
    pub base_hash: String,
    pub override_id: PromptSectionId,
    pub override_version: PromptSectionVersion,
    pub override_hash: String,
    pub baseline_manifest_digest: String,
    pub candidate_manifest_digest: String,
    pub route: String,
    pub stage: String,
    pub benchmark_digest: String,
    pub task_order_seed: u64,
    pub resampling_seed: u64,
    pub resamples: u64,
    pub model_revision: String,
    pub catalog_digest: String,
    pub budgets: String,
    pub paired_tasks: u32,
    /// Paired 95% bootstrap interval for the hidden hard-pass difference.
    pub hard_pass_ci: (f64, f64),
    /// Upper endpoint of the predeclared normalized cost difference CI.
    pub cost_diff_upper: f64,
    /// New escaped hard-gate regressions introduced by the candidate.
    pub escaped_regressions: u32,
    pub reviewer: String,
    pub decision: String,
}

impl PromptChangeRecord {
    /// The record's content address.
    pub fn digest(&self) -> String {
        let mut encoder = CanonicalEncoder::new(PROMPT_DIGEST_TAG);
        encoder
            .text("change-record")
            .text(&self.base_id.0)
            .u64(u64::from(self.base_version.0))
            .text(&self.base_hash)
            .text(&self.override_id.0)
            .u64(u64::from(self.override_version.0))
            .text(&self.override_hash)
            .text(&self.baseline_manifest_digest)
            .text(&self.candidate_manifest_digest)
            .text(&self.route)
            .text(&self.stage)
            .text(&self.benchmark_digest)
            .u64(self.task_order_seed)
            .u64(self.resampling_seed)
            .u64(self.resamples)
            .text(&self.model_revision)
            .text(&self.catalog_digest)
            .text(&self.budgets)
            .u64(u64::from(self.paired_tasks))
            .text(&format!("{:?}", self.hard_pass_ci))
            .text(&format!("{:?}", self.cost_diff_upper))
            .u64(u64::from(self.escaped_regressions))
            .text(&self.reviewer)
            .text(&self.decision);
        encoder.digest()
    }

    /// Gate AE: every activation condition, each independently falsifiable.
    /// `override_hash` is the live override's hash — a record not bound to
    /// it cannot activate it.
    pub fn permits_activation(
        &self,
        live_override_hash: &str,
        bounds: &ActivationBounds,
    ) -> Result<()> {
        bounds.validate()?;
        let refuse = |reason: &str| Err(SdkError::Domain(format!("activation refused: {reason}")));
        if self.override_hash != live_override_hash {
            return refuse("record is not bound to this override's content hash");
        }
        if self.paired_tasks < bounds.min_tasks {
            return refuse("paired sample below the task floor");
        }
        if self.resamples != ACTIVATION_BOOTSTRAP_RESAMPLES {
            return refuse("resampling procedure differs from the declared 10,000");
        }
        if self.escaped_regressions > 0 {
            return refuse("candidate escaped a hard-gate regression");
        }
        let (lower, _upper) = self.hard_pass_ci;
        if lower < -bounds.noninferiority_margin {
            return refuse("hard-pass difference fails noninferiority");
        }
        if lower <= 0.0 && self.cost_diff_upper >= 0.0 {
            return refuse("neither a hard-pass benefit nor a cost benefit is demonstrated");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> PromptChangeRecord {
        PromptChangeRecord {
            base_id: PromptSectionId("graph_plan/update_protocol".into()),
            base_version: PromptSectionVersion(1),
            base_hash: "sha256:base".into(),
            override_id: PromptSectionId("graph_plan/update_protocol".into()),
            override_version: PromptSectionVersion(1),
            override_hash: "sha256:override".into(),
            baseline_manifest_digest: "sha256:m0".into(),
            candidate_manifest_digest: "sha256:m1".into(),
            route: "genai/Qwen".into(),
            stage: "graph_plan".into(),
            benchmark_digest: "sha256:bench".into(),
            task_order_seed: 7,
            resampling_seed: 11,
            resamples: ACTIVATION_BOOTSTRAP_RESAMPLES,
            model_revision: "qwen-3.8".into(),
            catalog_digest: "sha256:catalog".into(),
            budgets: "default".into(),
            paired_tasks: 30,
            hard_pass_ci: (0.01, 0.09),
            cost_diff_upper: 0.2,
            escaped_regressions: 0,
            reviewer: "vikrant".into(),
            decision: "activate".into(),
        }
    }

    #[test]
    fn a_passing_record_activates_and_is_content_addressed() {
        let record = record();
        let bounds = ActivationBounds::default();
        assert!(record
            .permits_activation("sha256:override", &bounds)
            .is_ok());
        assert_eq!(record.digest(), record.digest());
    }

    #[test]
    fn each_activation_condition_is_independently_falsifiable() {
        let bounds = ActivationBounds::default();
        // Digest binding.
        assert!(record()
            .permits_activation("sha256:other", &bounds)
            .is_err());
        // Sample floor.
        let mut r = record();
        r.paired_tasks = 29;
        assert!(r.permits_activation("sha256:override", &bounds).is_err());
        // Resampling procedure.
        let mut r = record();
        r.resamples = 9_999;
        assert!(r.permits_activation("sha256:override", &bounds).is_err());
        // Escaped regression.
        let mut r = record();
        r.escaped_regressions = 1;
        assert!(r.permits_activation("sha256:override", &bounds).is_err());
        // Noninferiority.
        let mut r = record();
        r.hard_pass_ci = (-0.06, 0.0);
        assert!(r.permits_activation("sha256:override", &bounds).is_err());
        // Benefit: neither hard-pass lower bound > 0 nor cost upper < 0.
        let mut r = record();
        r.hard_pass_ci = (-0.01, 0.05);
        r.cost_diff_upper = 0.1;
        assert!(r.permits_activation("sha256:override", &bounds).is_err());
        // Cost benefit alone suffices under noninferiority.
        let mut r = record();
        r.hard_pass_ci = (-0.01, 0.05);
        r.cost_diff_upper = -0.02;
        assert!(r.permits_activation("sha256:override", &bounds).is_ok());
    }

    #[test]
    fn configured_bounds_are_floor_and_range_checked() {
        assert!(ActivationBounds {
            min_tasks: 29,
            noninferiority_margin: 0.05
        }
        .validate()
        .is_err());
        assert!(ActivationBounds {
            min_tasks: 30,
            noninferiority_margin: 0.06
        }
        .validate()
        .is_err());
        assert!(ActivationBounds {
            min_tasks: 50,
            noninferiority_margin: 0.0
        }
        .validate()
        .is_ok());
    }
}
