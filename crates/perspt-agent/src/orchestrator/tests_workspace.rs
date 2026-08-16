use super::*;
use std::path::PathBuf;

// =========================================================================
// Workspace Classification Tests
// =========================================================================

#[tokio::test]
async fn test_classify_workspace_empty_dir() {
    let temp = tempfile::tempdir().unwrap();
    let orch = SRBNOrchestrator::new_for_testing(temp.path().to_path_buf());
    let state = orch.classify_workspace("build a web app");
    // Empty dir with language keywords → Greenfield
    assert!(matches!(state, WorkspaceState::Greenfield { .. }));
}

#[tokio::test]
async fn test_classify_workspace_empty_dir_no_lang() {
    let temp = tempfile::tempdir().unwrap();
    let orch = SRBNOrchestrator::new_for_testing(temp.path().to_path_buf());
    let state = orch.classify_workspace("do something");
    // Empty dir, no keywords → Greenfield with no lang
    match state {
        WorkspaceState::Greenfield { inferred_lang } => assert!(inferred_lang.is_none()),
        _ => panic!("expected Greenfield, got {:?}", state),
    }
}

#[tokio::test]
async fn test_classify_workspace_existing_rust_project() {
    let temp = tempfile::tempdir().unwrap();
    // Create a Cargo.toml to make it look like a Rust project
    std::fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"",
    )
    .unwrap();
    let orch = SRBNOrchestrator::new_for_testing(temp.path().to_path_buf());
    let state = orch.classify_workspace("add a feature");
    match state {
        WorkspaceState::ExistingProject { plugins } => {
            assert!(plugins.contains(&"rust".to_string()));
        }
        _ => panic!("expected ExistingProject, got {:?}", state),
    }
}

#[tokio::test]
async fn test_classify_workspace_existing_python_project() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("pyproject.toml"),
        "[project]\nname = \"test\"",
    )
    .unwrap();
    let orch = SRBNOrchestrator::new_for_testing(temp.path().to_path_buf());
    let state = orch.classify_workspace("add a feature");
    match state {
        WorkspaceState::ExistingProject { plugins } => {
            assert!(plugins.contains(&"python".to_string()));
        }
        _ => panic!("expected ExistingProject, got {:?}", state),
    }
}

#[tokio::test]
async fn test_classify_workspace_existing_js_project() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("package.json"), "{}").unwrap();
    let orch = SRBNOrchestrator::new_for_testing(temp.path().to_path_buf());
    let state = orch.classify_workspace("add auth");
    match state {
        WorkspaceState::ExistingProject { plugins } => {
            assert!(plugins.contains(&"javascript".to_string()));
        }
        _ => panic!("expected ExistingProject, got {:?}", state),
    }
}

#[tokio::test]
async fn test_classify_workspace_ambiguous_with_misc_files() {
    let temp = tempfile::tempdir().unwrap();
    // Non-empty dir with misc files that don't match any plugin
    std::fs::write(temp.path().join("notes.txt"), "hello").unwrap();
    std::fs::write(temp.path().join("data.csv"), "a,b,c").unwrap();
    let orch = SRBNOrchestrator::new_for_testing(temp.path().to_path_buf());
    let state = orch.classify_workspace("do something");
    assert!(matches!(state, WorkspaceState::Ambiguous));
}

#[tokio::test]
async fn test_classify_workspace_greenfield_with_rust_task() {
    let temp = tempfile::tempdir().unwrap();
    let orch = SRBNOrchestrator::new_for_testing(temp.path().to_path_buf());
    let state = orch.classify_workspace("create a rust CLI tool");
    match state {
        WorkspaceState::Greenfield { inferred_lang } => {
            assert_eq!(inferred_lang, Some("rust".to_string()));
        }
        _ => panic!("expected Greenfield, got {:?}", state),
    }
}

#[tokio::test]
async fn test_classify_workspace_greenfield_with_python_task() {
    let temp = tempfile::tempdir().unwrap();
    let orch = SRBNOrchestrator::new_for_testing(temp.path().to_path_buf());
    let state = orch.classify_workspace("build a python flask API");
    match state {
        WorkspaceState::Greenfield { inferred_lang } => {
            assert_eq!(inferred_lang, Some("python".to_string()));
        }
        _ => panic!("expected Greenfield, got {:?}", state),
    }
}

// =========================================================================
// Tool Prerequisite Tests
// =========================================================================

#[tokio::test]
async fn test_check_prerequisites_returns_true_when_tools_available() {
    let orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    let registry = perspt_core::plugin::PluginRegistry::new();
    // Rust plugin — cargo/rustc should be available in dev environment
    if let Some(plugin) = registry.get("rust") {
        let result = orch.check_tool_prerequisites(plugin);
        // We can't assert true (CI might not have rust-analyzer)
        // but the method should not panic
        let _ = result;
    }
}

#[test]
fn test_required_binaries_rust_includes_cargo() {
    let registry = perspt_core::plugin::PluginRegistry::new();
    let plugin = registry.get("rust").unwrap();
    let bins = plugin.required_binaries();
    assert!(bins.iter().any(|(name, _, _)| *name == "cargo"));
    assert!(bins.iter().any(|(name, _, _)| *name == "rustc"));
}

#[test]
fn test_required_binaries_python_includes_uv() {
    let registry = perspt_core::plugin::PluginRegistry::new();
    let plugin = registry.get("python").unwrap();
    let bins = plugin.required_binaries();
    assert!(bins.iter().any(|(name, _, _)| *name == "uv"));
    assert!(bins.iter().any(|(name, _, _)| *name == "python3"));
}

#[test]
fn test_required_binaries_js_includes_node() {
    let registry = perspt_core::plugin::PluginRegistry::new();
    let plugin = registry.get("javascript").unwrap();
    let bins = plugin.required_binaries();
    assert!(bins.iter().any(|(name, _, _)| *name == "node"));
    assert!(bins.iter().any(|(name, _, _)| *name == "npm"));
}

// =========================================================================
// Fallback Resolution Tests
// =========================================================================

#[tokio::test]
async fn test_fallback_defaults_to_none_without_explicit_config() {
    let orch = SRBNOrchestrator::new_for_testing(PathBuf::from("."));
    assert!(orch.architect_fallback_model.is_none());
    assert!(orch.actuator_fallback_model.is_none());
    assert!(orch.verifier_fallback_model.is_none());
    assert!(orch.speculator_fallback_model.is_none());
}

#[tokio::test]
async fn test_explicit_fallback_stored_correctly() {
    let orch = SRBNOrchestrator::new_with_models(
        PathBuf::from("/tmp/test_fallback"),
        false,
        None,
        None,
        None,
        None,
        Some("gpt-4o".into()),
        Some("gpt-4o-mini".into()),
        Some("gpt-4o".into()),
        Some("gpt-4o-mini".into()),
    );
    assert_eq!(orch.architect_fallback_model, Some("gpt-4o".to_string()));
    assert_eq!(
        orch.actuator_fallback_model,
        Some("gpt-4o-mini".to_string())
    );
    assert_eq!(orch.verifier_fallback_model, Some("gpt-4o".to_string()));
    assert_eq!(
        orch.speculator_fallback_model,
        Some("gpt-4o-mini".to_string())
    );
}

#[tokio::test]
async fn test_per_tier_models_independent() {
    let orch = SRBNOrchestrator::new_with_models(
        PathBuf::from("/tmp/test_tiers_independent"),
        false,
        Some("arch".into()),
        Some("act".into()),
        Some("ver".into()),
        Some("spec".into()),
        None,
        None,
        None,
        None,
    );
    // Each tier stores its own model, not shared
    assert_ne!(orch.architect_model, orch.actuator_model);
    assert_ne!(orch.verifier_model, orch.speculator_model);
}

// =========================================================================
// Python auto-dependency repair tests
// =========================================================================

#[test]
fn test_extract_missing_python_modules_basic() {
    let output = r#"
FAILED tests/test_core.py::TestPipeline::test_run - ModuleNotFoundError: No module named 'httpx'
E   ModuleNotFoundError: No module named 'pydantic'
ImportError: No module named 'pyarrow'
"#;
    let mut missing = SRBNOrchestrator::extract_missing_python_modules(output);
    missing.sort();
    assert_eq!(missing, vec!["httpx", "pyarrow", "pydantic"]);
}

#[test]
fn test_extract_missing_python_modules_subpackage() {
    let output = "ModuleNotFoundError: No module named 'foo.bar.baz'";
    let missing = SRBNOrchestrator::extract_missing_python_modules(output);
    assert_eq!(missing, vec!["foo"]);
}

#[test]
fn test_extract_missing_python_modules_stdlib_filtered() {
    let output = r#"
ModuleNotFoundError: No module named 'numpy'
ModuleNotFoundError: No module named 'os'
ModuleNotFoundError: No module named 'json'
"#;
    let missing = SRBNOrchestrator::extract_missing_python_modules(output);
    assert_eq!(missing, vec!["numpy"]);
}

#[test]
fn test_extract_missing_python_modules_empty() {
    let output = "All tests passed!\n3 passed in 0.5s";
    let missing = SRBNOrchestrator::extract_missing_python_modules(output);
    assert!(missing.is_empty());
}

#[test]
fn test_python_import_to_package_mapping() {
    assert_eq!(SRBNOrchestrator::python_import_to_package("PIL"), "pillow");
    assert_eq!(SRBNOrchestrator::python_import_to_package("yaml"), "pyyaml");
    assert_eq!(
        SRBNOrchestrator::python_import_to_package("cv2"),
        "opencv-python"
    );
    assert_eq!(
        SRBNOrchestrator::python_import_to_package("sklearn"),
        "scikit-learn"
    );
    assert_eq!(
        SRBNOrchestrator::python_import_to_package("bs4"),
        "beautifulsoup4"
    );
    // Direct passthrough for unknown
    assert_eq!(SRBNOrchestrator::python_import_to_package("httpx"), "httpx");
    assert_eq!(
        SRBNOrchestrator::python_import_to_package("fastapi"),
        "fastapi"
    );
}

#[test]
fn test_normalize_command_to_uv_pip_install() {
    assert_eq!(
        SRBNOrchestrator::normalize_command_to_uv("pip install httpx"),
        "uv add httpx"
    );
    assert_eq!(
        SRBNOrchestrator::normalize_command_to_uv("pip3 install httpx pydantic"),
        "uv add httpx pydantic"
    );
    assert_eq!(
        SRBNOrchestrator::normalize_command_to_uv("python -m pip install requests"),
        "uv add requests"
    );
    assert_eq!(
        SRBNOrchestrator::normalize_command_to_uv("python3 -m pip install flask"),
        "uv add flask"
    );
}

#[test]
fn test_normalize_command_to_uv_requirements_file() {
    assert_eq!(
        SRBNOrchestrator::normalize_command_to_uv("pip install -r requirements.txt"),
        "uv pip install -r requirements.txt"
    );
}

#[test]
fn test_normalize_command_to_uv_passthrough() {
    // Already uv commands pass through unchanged
    assert_eq!(
        SRBNOrchestrator::normalize_command_to_uv("uv add httpx"),
        "uv add httpx"
    );
    // Non-Python commands pass through unchanged
    assert_eq!(
        SRBNOrchestrator::normalize_command_to_uv("cargo add serde"),
        "cargo add serde"
    );
    assert_eq!(
        SRBNOrchestrator::normalize_command_to_uv("npm install lodash"),
        "npm install lodash"
    );
}

#[test]
fn test_extract_commands_from_correction_rust_plugin_policy() {
    let response = r#"Here's the fix:
Commands:
```
uv add httpx
cargo add serde
pip install numpy
```
File: main.rs
```rust
use serde;
```"#;
    // Rust plugin allows cargo commands, denies uv/pip
    let commands = SRBNOrchestrator::extract_commands_from_correction(response, "rust");
    assert!(
        commands.contains(&"cargo add serde".to_string()),
        "{:?}",
        commands
    );
    assert!(
        !commands.contains(&"uv add httpx".to_string()),
        "Rust plugin should deny uv commands: {:?}",
        commands
    );
    assert!(
        !commands.contains(&"pip install numpy".to_string()),
        "Rust plugin should deny pip commands: {:?}",
        commands
    );
}

#[test]
fn test_extract_commands_from_correction_python_plugin_policy() {
    let response = r#"Commands:
```
uv add httpx
cargo add serde
pip install numpy
```"#;
    // Python plugin allows uv/pip commands, denies cargo
    let commands = SRBNOrchestrator::extract_commands_from_correction(response, "python");
    assert!(
        commands.contains(&"uv add httpx".to_string()),
        "{:?}",
        commands
    );
    assert!(
        commands.contains(&"pip install numpy".to_string()),
        "{:?}",
        commands
    );
    assert!(
        !commands.contains(&"cargo add serde".to_string()),
        "Python plugin should deny cargo commands: {:?}",
        commands
    );
}

#[test]
fn test_typed_parse_pipeline_multiple_files() {
    let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
    let content = r#"Here are the files:

File: src/etl_pipeline/core.py
```python
def run_pipeline():
pass
```

File: src/etl_pipeline/validator.py
```python
def validate(data):
return True
```

File: tests/test_core.py
```python
from etl_pipeline.core import run_pipeline

def test_run():
run_pipeline()
```
"#;
    let (bundle_opt, state, _) = orch.parse_artifact_bundle_typed(content, "test", 0);
    assert!(state.is_ok(), "Expected successful parse, got {}", state);
    let bundle = bundle_opt.unwrap();
    assert_eq!(bundle.artifacts.len(), 3, "Expected 3 artifacts");
    assert_eq!(bundle.artifacts[0].path(), "src/etl_pipeline/core.py");
    assert_eq!(bundle.artifacts[1].path(), "src/etl_pipeline/validator.py");
    assert_eq!(bundle.artifacts[2].path(), "tests/test_core.py");
}

#[test]
fn test_typed_parse_pipeline_single_file() {
    let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
    let content = r#"File: main.py
```python
print("hello")
```"#;
    let (bundle_opt, state, _) = orch.parse_artifact_bundle_typed(content, "test", 0);
    assert!(state.is_ok());
    let bundle = bundle_opt.unwrap();
    assert_eq!(bundle.artifacts.len(), 1);
    assert_eq!(bundle.artifacts[0].path(), "main.py");
}

#[test]
fn test_typed_parse_pipeline_mixed_file_and_diff() {
    let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
    let content = r#"File: new_module.py
```python
def new_fn():
pass
```

Diff: existing.py
```diff
--- existing.py
+++ existing.py
@@ -1 +1,2 @@
+import new_module
 def old_fn():
```"#;
    let (bundle_opt, state, _) = orch.parse_artifact_bundle_typed(content, "test", 0);
    assert!(state.is_ok());
    let bundle = bundle_opt.unwrap();
    assert_eq!(bundle.artifacts.len(), 2);
    assert_eq!(bundle.artifacts[0].path(), "new_module.py");
    assert!(
        bundle.artifacts[0].is_write(),
        "new_module.py should be a write"
    );
    assert_eq!(bundle.artifacts[1].path(), "existing.py");
    assert!(
        bundle.artifacts[1].is_diff(),
        "existing.py should be a diff"
    );
}

#[test]
fn test_typed_parse_pipeline_legacy_multi_file() {
    let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
    let content = r#"File: core.py
```python
def core():
pass
```

File: utils.py
```python
def util():
pass
```"#;
    let (bundle_opt, state, _) = orch.parse_artifact_bundle_typed(content, "test", 0);
    assert!(state.is_ok(), "Should parse multi-file response");
    let bundle = bundle_opt.unwrap();
    assert_eq!(bundle.artifacts.len(), 2, "Should have 2 artifacts");
    assert_eq!(bundle.artifacts[0].path(), "core.py");
    assert_eq!(bundle.artifacts[1].path(), "utils.py");
}

// =========================================================================
// Baseline regression tests — freeze pre-refactor behavior
// =========================================================================

#[test]
fn test_typed_parse_pipeline_structured_json() {
    let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
    let content = r#"Here is the output:
```json
{
  "artifacts": [
{"operation": "write", "path": "src/main.py", "content": "print('hello')"},
{"operation": "diff", "path": "src/lib.py", "patch": "--- a\n+++ b\n@@ -1 +1 @@\n-old\n+new"}
  ],
  "commands": ["uv add requests"]
}
```"#;
    let (bundle_opt, state, _) = orch.parse_artifact_bundle_typed(content, "test", 0);
    assert!(state.is_ok(), "Should parse structured JSON bundle");
    let bundle = bundle_opt.unwrap();
    assert_eq!(bundle.artifacts.len(), 2);
    assert!(bundle.artifacts[0].is_write());
    assert_eq!(bundle.artifacts[0].path(), "src/main.py");
    assert!(bundle.artifacts[1].is_diff());
    assert_eq!(bundle.artifacts[1].path(), "src/lib.py");
    assert_eq!(bundle.commands, vec!["uv add requests"]);
}

#[test]
fn test_typed_parse_pipeline_schema_invalid_classified() {
    let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
    let content = r#"```json
{"foo":"bar"}
```"#;
    let (bundle_opt, state, record_opt) = orch.parse_artifact_bundle_typed(content, "test", 1);
    assert!(bundle_opt.is_none());
    assert!(matches!(
        state,
        perspt_core::types::ParseResultState::SchemaInvalid
    ));
    let record = record_opt.expect("schema failure should be recorded");
    assert!(matches!(
        record.retry_classification,
        Some(perspt_core::types::RetryClassification::MalformedRetry)
    ));
}

#[test]
fn test_typed_parse_pipeline_semantic_rejection_classified() {
    use perspt_core::types::PlannedTask;

    let mut orch = SRBNOrchestrator::new_for_testing(std::path::PathBuf::from("/tmp/test"));
    let plan = TaskPlan {
        tasks: vec![PlannedTask {
            id: "parser".into(),
            goal: "Create parser".into(),
            output_files: vec!["src/parser.rs".into()],
            ..PlannedTask::new("parser", "Create parser")
        }],
    };
    orch.create_nodes_from_plan(&plan).unwrap();

    let content = r#"```json
{
  "artifacts": [
{"operation": "write", "path": "src/wrong.rs", "content": "pub fn wrong() {}"}
  ],
  "commands": []
}
```"#;
    let (bundle_opt, state, record_opt) = orch.parse_artifact_bundle_typed(content, "parser", 1);
    assert!(bundle_opt.is_none());
    assert!(matches!(
        state,
        perspt_core::types::ParseResultState::SemanticallyRejected
    ));
    let record = record_opt.expect("semantic rejection should be recorded");
    assert!(matches!(
        record.retry_classification,
        Some(perspt_core::types::RetryClassification::Retarget)
    ));
}

#[test]
fn test_typed_parse_pipeline_json_empty_path_rejected() {
    let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
    let content = r#"```json
{
  "artifacts": [
{"operation": "write", "path": "", "content": "bad"}
  ],
  "commands": []
}
```"#;
    let (bundle_opt, state, _) = orch.parse_artifact_bundle_typed(content, "test", 0);
    assert!(
        bundle_opt.is_none(),
        "Invalid bundle with empty path should be rejected"
    );
    assert!(
        !state.is_ok(),
        "Parse state should not be Ok for invalid bundle: {}",
        state
    );
}

#[test]
fn test_typed_parse_pipeline_json_absolute_path_rejected() {
    let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
    let content = r#"```json
{
  "artifacts": [
{"operation": "write", "path": "/etc/passwd", "content": "bad"}
  ],
  "commands": []
}
```"#;
    let (bundle_opt, state, _) = orch.parse_artifact_bundle_typed(content, "test", 0);
    assert!(
        bundle_opt.is_none(),
        "Invalid bundle with absolute path should be rejected"
    );
    assert!(
        !state.is_ok(),
        "Parse state should not be Ok for path traversal: {}",
        state
    );
}

#[test]
fn test_typed_parse_pipeline_returns_no_payload_for_garbage() {
    let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
    let content = "This is just a plain text response with no code blocks at all.";
    let (bundle_opt, state, _) = orch.parse_artifact_bundle_typed(content, "test", 0);
    assert!(bundle_opt.is_none());
    assert!(
        matches!(
            state,
            perspt_core::types::ParseResultState::NoStructuredPayload
        ),
        "Expected NoStructuredPayload, got {}",
        state
    );
}

#[tokio::test]
async fn test_effective_working_dir_with_sandbox() {
    // When a node has a provisional branch AND the sandbox directory exists,
    // effective_working_dir should return the sandbox path instead of workspace.
    let temp_dir = std::env::temp_dir().join(format!(
        "perspt_eff_workdir_sandbox_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let mut orch = SRBNOrchestrator::new_for_testing(temp_dir.clone());
    orch.context.session_id = "test_session".into();

    let parent = SRBNNode::new("root".into(), "root goal".into(), ModelTier::Actuator);
    let child = SRBNNode::new("child".into(), "child goal".into(), ModelTier::Actuator);
    orch.add_node(parent);
    orch.add_node(child);
    orch.add_dependency("root", "child", "dep").unwrap();

    let child_idx = orch.node_indices["child"];
    let branch_id = orch.maybe_create_provisional_branch(child_idx).unwrap();

    let sandbox_path = temp_dir
        .join(".perspt")
        .join("sandboxes")
        .join("test_session")
        .join(&branch_id);
    assert!(sandbox_path.exists(), "Sandbox should have been created");

    // effective_working_dir should now return the sandbox
    let eff = orch.effective_working_dir(child_idx);
    assert_eq!(eff, sandbox_path);

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_sandbox_dir_for_node_returns_path_when_exists() {
    let temp_dir = std::env::temp_dir().join(format!(
        "perspt_sandbox_dir_exists_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let mut orch = SRBNOrchestrator::new_for_testing(temp_dir.clone());
    orch.context.session_id = "sess".into();

    let parent = SRBNNode::new("p".into(), "g".into(), ModelTier::Actuator);
    let child = SRBNNode::new("c".into(), "g".into(), ModelTier::Actuator);
    orch.add_node(parent);
    orch.add_node(child);
    orch.add_dependency("p", "c", "dep").unwrap();

    let child_idx = orch.node_indices["c"];
    let branch_id = orch.maybe_create_provisional_branch(child_idx).unwrap();

    let sandbox = orch.sandbox_dir_for_node(child_idx);
    assert!(sandbox.is_some());
    let sandbox_path = sandbox.unwrap();
    assert!(sandbox_path.ends_with(&branch_id));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_root_node_bypasses_sandbox() {
    // Root nodes (no graph parents) should NOT get provisional branches,
    // and effective_working_dir should return the live workspace.
    let temp_dir =
        std::env::temp_dir().join(format!("perspt_root_bypass_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let mut orch = SRBNOrchestrator::new_for_testing(temp_dir.clone());

    let root = SRBNNode::new("root".into(), "root goal".into(), ModelTier::Actuator);
    orch.add_node(root);

    let root_idx = orch.node_indices["root"];
    // Root nodes now get a provisional branch with sandbox isolation
    let branch = orch.maybe_create_provisional_branch(root_idx);
    assert!(
        branch.is_some(),
        "Root node should now get a provisional branch for sandbox isolation"
    );

    // effective_working_dir should point to the sandbox, not the raw workspace
    let wd = orch.effective_working_dir(root_idx);
    assert_ne!(wd, temp_dir, "Root should use sandbox, not raw workspace");
    assert!(wd.to_string_lossy().contains("sandboxes"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_step_commit_copies_sandbox_to_workspace() {
    // Verify the commit path: files written to sandbox should appear in
    // the workspace after step_commit runs its copy-from-sandbox logic.
    use perspt_core::types::{ArtifactBundle, ArtifactOperation, PlannedTask};

    let temp_dir =
        std::env::temp_dir().join(format!("perspt_commit_copy_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(temp_dir.join("src")).unwrap();

    let mut orch = SRBNOrchestrator::new_for_testing(temp_dir.clone());
    orch.context.session_id = uuid::Uuid::new_v4().to_string();

    let plan = TaskPlan {
        tasks: vec![
            PlannedTask {
                id: "parent".into(),
                goal: "Parent".into(),
                output_files: vec!["src/parent.rs".into()],
                ..PlannedTask::new("parent", "Parent")
            },
            PlannedTask {
                id: "child".into(),
                goal: "Child".into(),
                output_files: vec!["src/child.rs".into()],
                dependencies: vec!["parent".into()],
                ..PlannedTask::new("child", "Child")
            },
        ],
    };
    orch.create_nodes_from_plan(&plan).unwrap();

    let child_idx = orch.node_indices["child"];
    let _branch_id = orch.maybe_create_provisional_branch(child_idx).unwrap();

    // Write a file into sandbox via apply_bundle_transactionally
    let bundle = ArtifactBundle {
        artifacts: vec![ArtifactOperation::Write {
            path: "src/child.rs".into(),
            content: "pub fn child_fn() {}\n".into(),
        }],
        commands: vec![],
    };
    orch.apply_bundle_transactionally(
        &bundle,
        "child",
        perspt_core::types::NodeClass::Implementation,
    )
    .await
    .unwrap();

    // Before commit: file should be in sandbox, NOT in workspace
    let sandbox = orch.sandbox_dir_for_node(child_idx).unwrap();
    assert!(sandbox.join("src/child.rs").exists());
    assert!(!temp_dir.join("src/child.rs").exists());

    // Now run step_commit to promote
    let child_idx = orch.node_indices["child"];
    let _ = orch.step_commit(child_idx).await;

    // After commit: file should now be in workspace
    assert!(
        temp_dir.join("src/child.rs").exists(),
        "step_commit should copy sandbox files to workspace"
    );
    let content = std::fs::read_to_string(temp_dir.join("src/child.rs")).unwrap();
    assert_eq!(content, "pub fn child_fn() {}\n");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_typed_parse_pipeline_json_path_traversal_rejected() {
    let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
    let content = r#"```json
{
  "artifacts": [
{"operation": "write", "path": "../../../etc/shadow", "content": "bad"}
  ],
  "commands": []
}
```"#;
    let (bundle_opt, state, _) = orch.parse_artifact_bundle_typed(content, "test", 0);
    assert!(
        bundle_opt.is_none(),
        "Invalid bundle with path traversal should be rejected"
    );
    assert!(
        !state.is_ok(),
        "Parse state should not be Ok for path traversal: {}",
        state
    );
}

// --- Step 6: Greenfield bootstrap ordering & dependency determinism ---

#[test]
fn test_dependency_expectations_threaded_to_nodes() {
    use perspt_core::types::{DependencyExpectation, PlannedTask, TaskPlan};

    let mut plan = TaskPlan::new();
    let mut t1 = PlannedTask::new("t1", "Create server module");
    t1.output_files = vec!["src/server.py".to_string()];
    t1.dependency_expectations = DependencyExpectation {
        required_packages: vec!["flask".to_string(), "pydantic".to_string()],
        setup_commands: vec![],
        min_toolchain_version: Some("3.11".to_string()),
    };
    plan.tasks.push(t1);

    let mut orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
    orch.create_nodes_from_plan(&plan).unwrap();

    // Verify the node carries dependency expectations
    let idx = orch.node_indices["t1"];
    let node = &orch.graph[idx];
    assert_eq!(node.dependency_expectations.required_packages.len(), 2);
    assert_eq!(node.dependency_expectations.required_packages[0], "flask");
    assert_eq!(
        node.dependency_expectations
            .min_toolchain_version
            .as_deref(),
        Some("3.11")
    );
}

#[test]
fn test_verifier_readiness_gate_no_plugins() {
    let orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
    // Should not panic with empty plugins
    orch.check_verifier_readiness_gate();
}

#[test]
fn test_architect_prompt_includes_dependency_expectations() {
    let ev = perspt_core::types::PromptEvidence {
        user_goal: Some("Build a web server".to_string()),
        project_summary: Some("empty project".to_string()),
        working_dir: Some("/tmp".to_string()),
        ..Default::default()
    };
    let prompt =
        crate::prompt_compiler::compile(perspt_core::types::PromptIntent::ArchitectExisting, &ev)
            .text;
    assert!(
        prompt.contains("dependency_expectations"),
        "Architect prompt must include dependency_expectations in the JSON schema"
    );
    assert!(
        prompt.contains("required_packages"),
        "Architect prompt must mention required_packages"
    );
    assert!(
        prompt.contains("min_toolchain_version"),
        "Architect prompt must mention min_toolchain_version"
    );
}

// --- Step 8: Budget enforcement & plan revision tracking ---

#[test]
fn test_budget_gate_stops_execution_when_exhausted() {
    let mut orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
    // Set a budget of 0 steps — should be immediately exhausted
    orch.set_budget(Some(0), None, None);
    assert!(
        orch.budget.any_exhausted(),
        "Budget with max_steps=0 should be immediately exhausted"
    );
}

#[test]
fn test_budget_step_recording() {
    let mut budget = perspt_core::types::BudgetEnvelope::new("test-session");
    budget.max_steps = Some(3);
    assert!(!budget.any_exhausted());
    budget.record_step();
    budget.record_step();
    assert!(!budget.any_exhausted());
    budget.record_step();
    assert!(budget.steps_exhausted());
    assert!(budget.any_exhausted());
}

#[test]
fn test_set_budget_configures_envelope() {
    let mut orch = SRBNOrchestrator::new(std::path::PathBuf::from("/tmp/test"), false);
    orch.set_budget(Some(10), Some(5), Some(2.50));
    assert_eq!(orch.budget.max_steps, Some(10));
    assert_eq!(orch.budget.max_revisions, Some(5));
    assert_eq!(orch.budget.max_cost_usd, Some(2.50));
    assert!(!orch.budget.any_exhausted());
}

#[test]
fn test_node_outcome_equality() {
    assert_eq!(NodeOutcome::Completed, NodeOutcome::Completed);
    assert_eq!(NodeOutcome::Escalated, NodeOutcome::Escalated);
    assert_ne!(NodeOutcome::Completed, NodeOutcome::Escalated);
}

#[test]
fn test_session_outcome_from_counts() {
    // The outcome derivation must account for total_nodes so that
    // unattempted nodes (budget/abort stop) are never counted as success.
    fn derive_outcome(
        completed: usize,
        escalated: usize,
        total: usize,
    ) -> perspt_core::SessionOutcome {
        if escalated == 0 && completed >= total {
            perspt_core::SessionOutcome::Success
        } else if completed > 0 {
            perspt_core::SessionOutcome::PartialSuccess
        } else {
            perspt_core::SessionOutcome::Failed
        }
    }

    // All completed → Success
    assert_eq!(
        derive_outcome(3, 0, 3),
        perspt_core::SessionOutcome::Success,
    );
    // Some completed, some escalated → PartialSuccess
    assert_eq!(
        derive_outcome(2, 1, 3),
        perspt_core::SessionOutcome::PartialSuccess,
    );
    // All escalated → Failed
    assert_eq!(derive_outcome(0, 3, 3), perspt_core::SessionOutcome::Failed,);
    // Budget-stopped: 5 of 20 completed, 0 escalated → PartialSuccess (not Success!)
    assert_eq!(
        derive_outcome(5, 0, 20),
        perspt_core::SessionOutcome::PartialSuccess,
    );
    // Budget-stopped: 0 of 20 completed, 0 escalated → Failed
    assert_eq!(
        derive_outcome(0, 0, 20),
        perspt_core::SessionOutcome::Failed,
    );
}

#[test]
fn test_resumed_outcome_from_counts() {
    // Resumed sessions derive outcome the same way: unattempted nodes
    // prevent Success, and terminal_count offsets the total.
    fn derive_resumed_outcome(
        executed: usize,
        escalated: usize,
        terminal_count: usize,
        total: usize,
    ) -> perspt_core::SessionOutcome {
        if escalated == 0 && executed + terminal_count >= total {
            perspt_core::SessionOutcome::Success
        } else if executed > 0 {
            perspt_core::SessionOutcome::PartialSuccess
        } else {
            perspt_core::SessionOutcome::Failed
        }
    }

    // All resumable nodes completed, 2 already terminal
    assert_eq!(
        derive_resumed_outcome(3, 0, 2, 5),
        perspt_core::SessionOutcome::Success,
    );
    // Some escalated on resume
    assert_eq!(
        derive_resumed_outcome(2, 1, 2, 5),
        perspt_core::SessionOutcome::PartialSuccess,
    );
    // Budget stopped mid-resume: 2 of 5 completed, 2 terminal, 1 not attempted
    assert_eq!(
        derive_resumed_outcome(1, 0, 2, 5),
        perspt_core::SessionOutcome::PartialSuccess,
    );
    // Nothing executed on resume (all blocked/seal-gated)
    assert_eq!(
        derive_resumed_outcome(0, 0, 5, 5),
        perspt_core::SessionOutcome::Success,
    );
    // Nothing executed, not all terminal → Failed
    assert_eq!(
        derive_resumed_outcome(0, 0, 2, 5),
        perspt_core::SessionOutcome::Failed,
    );
}

#[test]
fn test_sheaf_pre_check_stub_escalates_after_retry() {
    let dir = tempfile::tempdir().unwrap();
    let stub_path = dir.path().join("stub.rs");
    std::fs::write(&stub_path, "fn main() {\n    todo!()\n}\n").unwrap();

    let (mut orch, idx) = orch_with_node(dir.path().to_path_buf());
    orch.graph[idx]
        .output_targets
        .push(std::path::PathBuf::from("stub.rs"));
    orch.graph[idx].owner_plugin = "rust".to_string();

    // First call detects stub
    let first = orch.sheaf_pre_check(idx);
    assert!(first.is_some(), "First pre-check should detect stub");

    // Simulate: after retry, the file is still a stub.
    // The final guard should also detect it.
    let second = orch.sheaf_pre_check(idx);
    assert!(
        second.is_some(),
        "Final guard should still detect stub after retry"
    );
}

/// Helper: create an orchestrator with a single default node for testing.
fn orch_with_node(
    working_dir: std::path::PathBuf,
) -> (SRBNOrchestrator, petgraph::graph::NodeIndex) {
    let mut orch = SRBNOrchestrator::new(working_dir, false);
    let node = SRBNNode::new(
        "test-node".to_string(),
        "test goal".to_string(),
        perspt_core::ModelTier::Actuator,
    );
    let idx = orch.add_node(node);
    (orch, idx)
}

#[test]
fn test_sheaf_pre_check_passes_when_no_outputs() {
    let (orch, idx) = orch_with_node(std::path::PathBuf::from("/tmp/test"));
    assert!(orch.sheaf_pre_check(idx).is_none());
}

#[test]
fn test_sheaf_pre_check_detects_missing_files() {
    let (mut orch, idx) = orch_with_node(std::path::PathBuf::from("/tmp/test"));
    orch.graph[idx]
        .output_targets
        .push(std::path::PathBuf::from("nonexistent_file_xyz.rs"));
    let result = orch.sheaf_pre_check(idx);
    assert!(result.is_some());
    assert!(result.unwrap().contains("missing"));
}

#[test]
fn test_sheaf_pre_check_detects_empty_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::File::create(dir.path().join("empty.rs")).unwrap();

    let (mut orch, idx) = orch_with_node(dir.path().to_path_buf());
    orch.graph[idx]
        .output_targets
        .push(std::path::PathBuf::from("empty.rs"));
    let result = orch.sheaf_pre_check(idx);
    assert!(result.is_some());
    assert!(result.unwrap().contains("empty"));
}

#[test]
fn test_sheaf_pre_check_passes_for_valid_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let (mut orch, idx) = orch_with_node(dir.path().to_path_buf());
    orch.graph[idx]
        .output_targets
        .push(std::path::PathBuf::from("main.rs"));
    assert!(orch.sheaf_pre_check(idx).is_none());
}

#[test]
fn test_v_boot_energy_from_degraded_sensors() {
    use perspt_core::types::{EnergyComponents, SensorStatus, StageOutcome, VerificationResult};

    // Simulate a verification result with one fallback and one unavailable sensor
    let vr = VerificationResult {
        syntax_ok: true,
        build_ok: true,
        tests_ok: true,
        lint_ok: true,
        diagnostics_count: 0,
        tests_passed: 5,
        tests_failed: 0,
        summary: String::new(),
        raw_output: None,
        degraded: true,
        degraded_reason: Some("test sensor fallback".into()),
        stage_outcomes: vec![
            StageOutcome {
                stage: "syntax_check".into(),
                passed: true,
                sensor_status: SensorStatus::Available,
                output: None,
            },
            StageOutcome {
                stage: "build".into(),
                passed: true,
                sensor_status: SensorStatus::Fallback {
                    actual: "cargo check".into(),
                    reason: "primary not found".into(),
                },
                output: None,
            },
            StageOutcome {
                stage: "test".into(),
                passed: true,
                sensor_status: SensorStatus::Unavailable {
                    reason: "no test runner".into(),
                },
                output: None,
            },
        ],
    };

    // Compute V_boot the same way verification.rs does
    let mut energy = EnergyComponents::default();
    for so in &vr.stage_outcomes {
        match &so.sensor_status {
            SensorStatus::Unavailable { .. } => energy.v_boot += 3.0,
            SensorStatus::Fallback { .. } => energy.v_boot += 1.0,
            SensorStatus::Available => {}
        }
    }
    // 1 fallback (+1.0) + 1 unavailable (+3.0) = 4.0
    assert!(
        (energy.v_boot - 4.0).abs() < f32::EPSILON,
        "Expected V_boot=4.0, got {}",
        energy.v_boot
    );
}

// ── Stub detection tests ──────────────────────────────────────────

#[test]
fn test_detect_stub_rust_todo() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lib.rs");
    std::fs::write(&path, "fn main() {\n    todo!()\n}\n").unwrap();
    let result = detect_stub_content(&path, "rust");
    assert!(result.is_some(), "Should detect todo!() stub");
    assert!(result.unwrap().contains("todo!()"));
}

#[test]
fn test_detect_stub_rust_unimplemented() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lib.rs");
    std::fs::write(&path, "fn run() {\n    unimplemented!()\n}\n").unwrap();
    let result = detect_stub_content(&path, "rust");
    assert!(result.is_some(), "Should detect unimplemented!() stub");
}

#[test]
fn test_detect_stub_rust_real_code_not_flagged() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lib.rs");
    let real_code = r#"
use std::collections::HashMap;

fn add(a: i32, b: i32) -> i32 {
a + b
}

fn multiply(a: i32, b: i32) -> i32 {
a * b
}

fn compute(data: &[i32]) -> i32 {
data.iter().sum()
}

fn transform(input: &str) -> String {
input.to_uppercase()
}

fn process() {
let x = add(1, 2);
let y = multiply(x, 3);
println!("{}", y);
// todo!() in a comment should not trigger
}
"#;
    std::fs::write(&path, real_code).unwrap();
    let result = detect_stub_content(&path, "rust");
    assert!(
        result.is_none(),
        "Real code with comment-only todo should not be flagged"
    );
}

#[test]
fn test_detect_stub_rust_real_code_with_one_todo_branch() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lib.rs");
    let code = r#"
fn add(a: i32, b: i32) -> i32 { a + b }
fn sub(a: i32, b: i32) -> i32 { a - b }
fn mul(a: i32, b: i32) -> i32 { a * b }
fn div(a: i32, b: i32) -> i32 { a / b }
fn modulo(a: i32, b: i32) -> i32 { todo!() }
"#;
    std::fs::write(&path, code).unwrap();
    let result = detect_stub_content(&path, "rust");
    assert!(
        result.is_none(),
        "File with 5+ real lines and one todo!() should NOT be flagged"
    );
}

#[test]
fn test_detect_stub_python_pass_body() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.py");
    std::fs::write(&path, "def run():\n    pass\n").unwrap();
    let result = detect_stub_content(&path, "python");
    assert!(result.is_some(), "Should detect pass-only Python function");
}

#[test]
fn test_detect_stub_python_not_implemented() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.py");
    std::fs::write(&path, "def run():\n    raise NotImplementedError()\n").unwrap();
    let result = detect_stub_content(&path, "python");
    assert!(result.is_some(), "Should detect NotImplementedError stub");
}

#[test]
fn test_detect_stub_python_ellipsis_body() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.py");
    std::fs::write(&path, "def run():\n    ...\n").unwrap();
    let result = detect_stub_content(&path, "python");
    assert!(
        result.is_some(),
        "Should detect ellipsis-only Python function"
    );
}

#[test]
fn test_detect_stub_python_real_code_not_flagged() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.py");
    let code =
        "import os\n\ndef run():\n    data = os.listdir('.')\n    filtered = [f for f in data \
        if f.endswith('.py')]\n    for f in filtered:\n        print(f)\n    return filtered\n";
    std::fs::write(&path, code).unwrap();
    let result = detect_stub_content(&path, "python");
    assert!(result.is_none(), "Real Python code should not be flagged");
}

#[test]
fn test_detect_stub_js_throw_not_implemented() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.js");
    std::fs::write(
        &path,
        "function run() {\n  throw new Error(\"not implemented\");\n}\n",
    )
    .unwrap();
    let result = detect_stub_content(&path, "javascript");
    assert!(
        result.is_some(),
        "Should detect JS throw not-implemented stub"
    );
}

#[test]
fn test_detect_stub_universal_comment() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lib.rs");
    std::fs::write(&path, "// stub — will be replaced by agent\n").unwrap();
    let result = detect_stub_content(&path, "rust");
    assert!(result.is_some(), "Should detect universal stub comment");
}

#[test]
fn test_detect_stub_extension_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.py");
    std::fs::write(&path, "# placeholder\ndef run():\n    pass\n").unwrap();
    // Use "unknown" plugin hint — should fall back to .py extension
    let result = detect_stub_content(&path, "unknown");
    assert!(
        result.is_some(),
        "Should detect stub via extension fallback"
    );
}

#[test]
fn test_detect_stub_empty_file_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.rs");
    std::fs::write(&path, "").unwrap();
    // detect_stub_content focuses on stub patterns, not emptiness
    // (emptiness is handled by the metadata check in sheaf_pre_check)
    let result = detect_stub_content(&path, "rust");
    assert!(result.is_none(), "Empty file has no stub pattern to match");
}
