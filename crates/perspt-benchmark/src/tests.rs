use super::*;

fn corpus() -> Vec<Task> {
    load_tasks(&source_corpus_root()).unwrap()
}

#[test]
fn packaged_corpus_restores_nested_rust_manifests() {
    let packaged = materialize_corpus().unwrap();
    let packaged_tasks = load_tasks(packaged.path()).unwrap();
    assert_eq!(packaged_tasks.len(), corpus().len());
    assert!(packaged
        .path()
        .join("graph-rust-dependent/fixture/Cargo.toml")
        .is_file());
    assert!(!packaged
        .path()
        .join("graph-rust-dependent/fixture/Cargo.toml.fixture")
        .exists());
}

#[test]
fn committed_corpus_clears_the_composition_gate() {
    let validation = validate_corpus_shape(&corpus());
    assert!(validation.ready, "{:?}", validation.violations);
    assert_eq!(validation.coverage.rust, 16);
    assert_eq!(validation.coverage.python, 10);
    assert_eq!(validation.coverage.mixed, 4);
    assert!(!validation.corpus_digest.is_empty());
}

#[test]
fn task_randomization_is_seeded_and_not_sorted() {
    let mut first = corpus();
    let mut second = corpus();
    shuffle_tasks(&mut first, TASK_ORDER_SEED);
    shuffle_tasks(&mut second, TASK_ORDER_SEED);
    let first_ids: Vec<_> = first.iter().map(|task| &task.id).collect();
    let second_ids: Vec<_> = second.iter().map(|task| &task.id).collect();
    assert_eq!(first_ids, second_ids);
    assert!(first_ids.windows(2).any(|pair| pair[0] > pair[1]));
}

#[test]
fn paired_bootstrap_is_deterministic() {
    let differences = [1.0, 0.0, -1.0, 1.0];
    assert_eq!(
        bootstrap_ci(&differences, 11, 10_000),
        bootstrap_ci(&differences, 11, 10_000)
    );
}

#[test]
fn aggregation_requires_two_families_on_one_corpus() {
    let directory = tempfile::tempdir().unwrap();
    let mut paths = Vec::new();
    for (index, family) in ["family-a", "family-b"].into_iter().enumerate() {
        let path = directory.path().join(format!("{index}.json"));
        let report = serde_json::json!({
            "suite": "full",
            "topology": {
                "actuator": format!("provider::{index}"),
                "actuator_family": family,
            },
            "gate_ae_ready": true,
            "adaptive_route_accepted": true,
            "corpus_validation": {"corpus_digest": "same"},
        });
        std::fs::write(&path, serde_json::to_vec(&report).unwrap()).unwrap();
        paths.push(path);
    }
    let aggregate = aggregate_reports(&paths).unwrap();
    assert_eq!(aggregate["adaptive_default_accepted"], true);
}

#[test]
fn topology_comes_from_configuration_without_benchmark_model_arguments() {
    let config = perspt_core::Config::from_toml_str(
        r#"
[models]
actuator = "provider-a::custom-actuator"
architect = "provider-b::custom-planner"
speculator = "provider-c::custom-explorer"
verifier = "provider-d::custom-verifier"
adjudicator = "provider-e::custom-judge"
"#,
    )
    .unwrap();
    let portfolio = perspt_core::ModelPortfolio::from_config(&config).unwrap();
    let topology = serde_json::to_value(configured_topology(&config, &portfolio).unwrap()).unwrap();
    assert_eq!(topology["actuator"], "provider-a::custom-actuator");
    assert_eq!(topology["architect"], "provider-b::custom-planner");
    assert_eq!(topology["speculator"], "provider-c::custom-explorer");
    assert_eq!(topology["verifier"], "provider-d::custom-verifier");
    assert_eq!(topology["adjudicator"], "provider-e::custom-judge");
}
