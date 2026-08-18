//! Measured prompt activation mechanism checks (PSP-10 Gate AE, Phase 5).
//!
//! A section override is experimental until a content-addressed
//! `PromptChangeRecord` bound to its section identity, version, and hash
//! satisfies the sample, safety, noninferiority, and benefit rules. Each
//! falsifier below fails one condition independently.

use perspt_sdk::prompt::{
    ActivationBounds, ActivationState, PromptChangeRecord, PromptSectionId, PromptSectionVersion,
    SectionOverride, SectionTemplate, SectionVariants, ACTIVATION_BOOTSTRAP_RESAMPLES,
};
use perspt_sdk::prompt::{OverrideOrigin, PromptMessageRole, PromptRoute, SectionSchema};

fn template(body: &str) -> SectionTemplate {
    SectionTemplate {
        schema: SectionSchema {
            id: PromptSectionId("graph_plan/update_protocol".into()),
            version: PromptSectionVersion(1),
            role: PromptMessageRole::System,
            required: true,
            priority: 0,
            max_bytes: 4096,
            vars: vec![],
        },
        content_hash: SectionTemplate::hash_body(body),
        body: body.into(),
    }
}

fn passing_record(override_hash: &str) -> PromptChangeRecord {
    PromptChangeRecord {
        base_id: PromptSectionId("graph_plan/update_protocol".into()),
        base_version: PromptSectionVersion(1),
        base_hash: "sha256:base".into(),
        override_id: PromptSectionId("graph_plan/update_protocol".into()),
        override_version: PromptSectionVersion(1),
        override_hash: override_hash.into(),
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
        reviewer: "reviewer".into(),
        decision: "activate".into(),
    }
}

/// An experimental override never resolves; the base always serves.
#[test]
fn an_experimental_override_never_resolves() {
    let variants = SectionVariants {
        base: template("base text"),
        overrides: vec![SectionOverride {
            origin: OverrideOrigin::Family("Qwen".into()),
            activation: ActivationState::Experimental,
            template: template("experimental text"),
        }],
    };
    let route = PromptRoute {
        adapter: "genai".into(),
        family: perspt_sdk::ModelFamily::Qwen,
        exact_model: None,
    };
    assert_eq!(variants.resolve(&route).0.body, "base text");
}

/// The full activation matrix: one falsifier per Gate AE condition.
#[test]
fn every_activation_condition_is_independently_falsifiable() {
    let bounds = ActivationBounds::default();
    let live_hash = "sha256:override";
    assert!(passing_record(live_hash)
        .permits_activation(live_hash, &bounds)
        .is_ok());
    // Digest binding: a record for other content cannot activate this one.
    assert!(passing_record("sha256:other")
        .permits_activation(live_hash, &bounds)
        .is_err());
    // Sample floor.
    let mut r = passing_record(live_hash);
    r.paired_tasks = 29;
    assert!(r.permits_activation(live_hash, &bounds).is_err());
    // Changed resampling procedure.
    let mut r = passing_record(live_hash);
    r.resamples = 1_000;
    assert!(r.permits_activation(live_hash, &bounds).is_err());
    // A new escaped hard-gate regression is disqualifying regardless of CI.
    let mut r = passing_record(live_hash);
    r.escaped_regressions = 1;
    assert!(r.permits_activation(live_hash, &bounds).is_err());
    // Noninferiority: the lower CI endpoint must clear -epsilon.
    let mut r = passing_record(live_hash);
    r.hard_pass_ci = (-0.051, 0.02);
    assert!(r.permits_activation(live_hash, &bounds).is_err());
    // Benefit: noninferior alone is not enough without a cost win.
    let mut r = passing_record(live_hash);
    r.hard_pass_ci = (-0.01, 0.03);
    r.cost_diff_upper = 0.0;
    assert!(r.permits_activation(live_hash, &bounds).is_err());
    // A demonstrated cost benefit under noninferiority activates.
    let mut r = passing_record(live_hash);
    r.hard_pass_ci = (-0.01, 0.03);
    r.cost_diff_upper = -0.05;
    assert!(r.permits_activation(live_hash, &bounds).is_ok());
}

/// Configuration may only tighten the bounds; loosening fails at startup.
#[test]
fn configured_bounds_may_only_tighten() {
    assert!(ActivationBounds {
        min_tasks: 29,
        noninferiority_margin: 0.05
    }
    .validate()
    .is_err());
    assert!(ActivationBounds {
        min_tasks: 30,
        noninferiority_margin: 0.051
    }
    .validate()
    .is_err());
    assert!(ActivationBounds {
        min_tasks: 100,
        noninferiority_margin: 0.01
    }
    .validate()
    .is_ok());
}

/// The record digest is content-addressed and stable.
#[test]
fn change_records_are_content_addressed() {
    let record = passing_record("sha256:override");
    assert!(record.digest().starts_with("sha256:"));
    assert_eq!(record.digest(), passing_record("sha256:override").digest());
    let mut changed = passing_record("sha256:override");
    changed.paired_tasks = 31;
    assert_ne!(record.digest(), changed.digest());
}
