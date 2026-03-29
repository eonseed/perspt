//! Agent Trait and Implementations
//!
//! Defines the interface for all agent implementations and provides
//! LLM-integrated implementations for Architect, Actuator, and Verifier roles.

use crate::types::{AgentContext, AgentMessage, ModelTier, SRBNNode};
use anyhow::Result;
use async_trait::async_trait;
use perspt_core::llm_provider::GenAIProvider;
use std::fs;
use std::path::Path;
use std::sync::Arc;

/// The Agent trait defines the interface for SRBN agents.
///
/// Each agent role (Architect, Actuator, Verifier, Speculator) implements
/// this trait to provide specialized behavior.
#[async_trait]
pub trait Agent: Send + Sync {
    /// Process a task and return a message
    async fn process(&self, node: &SRBNNode, ctx: &AgentContext) -> Result<AgentMessage>;

    /// Get the agent's display name
    fn name(&self) -> &str;

    /// Check if this agent can handle the given node
    fn can_handle(&self, node: &SRBNNode) -> bool;

    /// Get the model name used by this agent (for logging)
    fn model(&self) -> &str;

    /// Build the prompt for this agent (for logging)
    fn build_prompt(&self, node: &SRBNNode, ctx: &AgentContext) -> String;
}

/// Architect agent - handles planning and DAG construction
pub struct ArchitectAgent {
    model: String,
    provider: Arc<GenAIProvider>,
}

impl ArchitectAgent {
    pub fn new(provider: Arc<GenAIProvider>, model: Option<String>) -> Self {
        Self {
            model: model.unwrap_or_else(|| ModelTier::Architect.default_model().to_string()),
            provider,
        }
    }

    pub fn build_planning_prompt(&self, node: &SRBNNode, ctx: &AgentContext) -> String {
        // Delegate to the canonical task decomposition prompt with node-level context
        Self::build_task_decomposition_prompt(
            &node.goal,
            &ctx.working_dir,
            &format!(
                "Context Files: {:?}\nOutput Targets: {:?}",
                node.context_files, node.output_targets
            ),
            None,
        )
    }

    /// PSP-5 Fix F: Canonical task decomposition prompt with the full JSON schema contract.
    /// Used by both the ArchitectAgent (node-level) and the Orchestrator (initial planning).
    pub fn build_task_decomposition_prompt(
        task: &str,
        working_dir: &Path,
        project_context: &str,
        last_error: Option<&str>,
    ) -> String {
        let error_feedback = if let Some(e) = last_error {
            format!(
                "\n## Previous Attempt Failed\nError: {}\nPlease fix the JSON format and try again.\n",
                e
            )
        } else {
            String::new()
        };

        format!(
            r#"You are an Architect agent in a multi-agent coding system.

## Task
{task}

## Working Directory
{working_dir}

## Existing Project Structure
{project_context}
{error_feedback}
## Instructions
Analyze this task and produce a structured execution plan as JSON.

### OWNERSHIP CLOSURE (CRITICAL — violating this will fail the build)
Each file path MUST appear in the `output_files` of EXACTLY ONE task.
- NO two tasks may list the same file in their `output_files`.
- A task that creates `src/math.py` MUST NOT also appear in another task's `output_files`.
- Test files (e.g., `tests/test_math.py`) are owned by whichever single task creates them.
- If a task needs to READ a file owned by another task, list it in `context_files`, NOT `output_files`.

### MODULAR PROJECT STRUCTURE
Your plan MUST create a COMPLETE, RUNNABLE project with proper modularity:

1. **Entry Point First**: Create a main entry point file (e.g., `main.py`, `src/main.rs`, `index.js`)
2. **Logical Modules**: Split functionality into separate files/modules with clear responsibilities
3. **Proper Imports**: Ensure all cross-file imports will resolve correctly
4. **Package Structure**: For Python, include `__init__.py` files in subdirectories
5. **One Test Task Per Module**: Each module's tests go in their OWN task with a UNIQUE test file.
   - Task for `src/math.py` → its test task owns `tests/test_math.py`
   - Task for `src/strings.py` → its test task owns `tests/test_strings.py`
   - NEVER put tests for multiple modules in the same test file

### TASK ORDERING
1. Create foundational modules before dependent ones
2. Specify dependencies accurately between tasks
3. Entry point task should depend on all modules it imports
4. Test tasks depend on the module they test

### COMPLETENESS CHECKLIST
- [ ] Every file path appears in exactly one task's `output_files` (no duplicates across tasks)
- [ ] Every import in generated code must reference an existing or planned file
- [ ] The project must be immediately runnable after all tasks complete
- [ ] Include at least one test file per core module
- [ ] All functions must have type hints (Python) or type annotations (Rust/TS)

## CRITICAL CONSTRAINTS
- DO NOT create `pyproject.toml`, `requirements.txt`, `package.json`, `Cargo.toml`, or any project configuration files
- The system handles project initialization separately via CLI tools (uv, npm, cargo)
- Focus ONLY on source code files (.py, .js, .rs, etc.) and test files
- If you need to add dependencies, include them in the task goal description (e.g., "Add requests library for HTTP calls")

## Output Format
Respond with ONLY a JSON object in this exact format:
```json
{{
  "tasks": [
    {{
      "id": "task_1",
      "goal": "Create module_a with core functionality",
      "context_files": [],
      "output_files": ["module_a.py"],
      "dependencies": [],
      "task_type": "code",
      "contract": {{
        "interface_signature": "def function_name(arg: Type) -> ReturnType",
        "invariants": ["Must handle edge cases"],
        "forbidden_patterns": ["no bare except"],
        "tests": [
          {{"name": "test_function_name", "criticality": "Critical"}}
        ]
      }}
    }},
    {{
      "id": "test_task_1",
      "goal": "Unit tests for module_a (ONLY this module)",
      "context_files": ["module_a.py"],
      "output_files": ["tests/test_module_a.py"],
      "dependencies": ["task_1"],
      "task_type": "unit_test"
    }},
    {{
      "id": "task_2",
      "goal": "Create module_b with helper utilities",
      "context_files": [],
      "output_files": ["module_b.py"],
      "dependencies": [],
      "task_type": "code"
    }},
    {{
      "id": "test_task_2",
      "goal": "Unit tests for module_b (ONLY this module)",
      "context_files": ["module_b.py"],
      "output_files": ["tests/test_module_b.py"],
      "dependencies": ["task_2"],
      "task_type": "unit_test"
    }},
    {{
      "id": "main_entry",
      "goal": "Create main.py entry point that imports and uses other modules",
      "context_files": ["module_a.py", "module_b.py"],
      "output_files": ["main.py"],
      "dependencies": ["task_1", "task_2"],
      "task_type": "code"
    }}
  ]
}}
```

Valid task_type values: "code", "unit_test", "integration_test", "refactor", "documentation"
Valid criticality values: "Critical", "High", "Low"

IMPORTANT: Output ONLY the JSON, no other text."#,
            task = task,
            working_dir = working_dir.display(),
            project_context = project_context,
            error_feedback = error_feedback
        )
    }
}

#[async_trait]
impl Agent for ArchitectAgent {
    async fn process(&self, node: &SRBNNode, ctx: &AgentContext) -> Result<AgentMessage> {
        log::info!(
            "[Architect] Processing node: {} with model {}",
            node.node_id,
            self.model
        );

        let prompt = self.build_planning_prompt(node, ctx);

        let response = self
            .provider
            .generate_response_simple(&self.model, &prompt)
            .await?;

        Ok(AgentMessage::new(ModelTier::Architect, response))
    }

    fn name(&self) -> &str {
        "Architect"
    }

    fn can_handle(&self, node: &SRBNNode) -> bool {
        matches!(node.tier, ModelTier::Architect)
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn build_prompt(&self, node: &SRBNNode, ctx: &AgentContext) -> String {
        self.build_planning_prompt(node, ctx)
    }
}

/// Actuator agent - handles code generation
pub struct ActuatorAgent {
    model: String,
    provider: Arc<GenAIProvider>,
}

impl ActuatorAgent {
    pub fn new(provider: Arc<GenAIProvider>, model: Option<String>) -> Self {
        Self {
            model: model.unwrap_or_else(|| ModelTier::Actuator.default_model().to_string()),
            provider,
        }
    }

    pub fn build_coding_prompt(&self, node: &SRBNNode, ctx: &AgentContext) -> String {
        let contract = &node.contract;
        let allowed_output_paths: Vec<String> = node
            .output_targets
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect();
        let workspace_import_hints = Self::workspace_import_hints(&ctx.working_dir);

        // Determine target file from output_targets or generate default
        let target_file = node
            .output_targets
            .first()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "main.py".to_string());

        // PSP-5: Determine output format based on execution mode and plugin
        let is_project_mode = ctx.execution_mode == perspt_core::types::ExecutionMode::Project;
        let has_multiple_outputs = node.output_targets.len() > 1;

        let output_format_section = if is_project_mode || has_multiple_outputs {
            format!(
                r#"## Output Format (Multi-Artifact Bundle)
When producing multi-file output, use this JSON format wrapped in a ```json code block:

```json
{{
  "artifacts": [
    {{ "path": "{target_file}", "operation": "write", "content": "..." }},
    {{ "path": "tests/test_main.py", "operation": "write", "content": "..." }}
  ],
  "commands": ["cargo add serde --features derive", "cargo add thiserror"]
}}
```

The `commands` array should contain dependency install commands (e.g. `cargo add <crate>`, `pip install <pkg>`) that must run BEFORE the code can compile. Leave it empty `[]` only if no new dependencies are needed.

Each artifact entry must have:
- `path`: Relative path within the workspace
- `operation`: Either `"write"` (full file) or `"diff"` (unified diff patch)
- `content` (for write) or `patch` (for diff): The file content or patch

RULES:
- Paths MUST be relative (no leading `/`)
- Use `"write"` for new files or full rewrites
- Use `"diff"` with proper unified diff format for small changes to existing files
- Include ALL files needed for the task in a single bundle
- ONLY emit artifacts for the declared allowed output paths listed below
- DO NOT create, modify, or patch any file not listed in `Allowed Output Paths`"#,
                target_file = target_file
            )
        } else {
            format!(
                r#"## Output Format
Use one of these formats:

### Creating a New File
File: {target_file}
```python
# your code here
```

### Modifying an Existing File
Diff: {target_file}
```diff
--- {target_file}
+++ {target_file}
@@ -10,2 +10,3 @@
 def calculate(x):
-    return x * 2
+    return x * 3
```

IMPORTANT:
- Use 'Diff:' for existing files to save tokens
- Use 'File:' ONLY for new files or full rewrites"#,
                target_file = target_file
            )
        };

        format!(
            r#"You are an Actuator agent responsible for implementing code.

## Task
Goal: {goal}

## Behavioral Contract
Interface Signature: {interface}
Invariants: {invariants:?}
Forbidden Patterns: {forbidden:?}

## Context
Working Directory: {working_dir:?}
Files to Read: {context_files:?}
Target Output File: {target_file}
Allowed Output Paths: {allowed_output_paths:?}
Workspace Import Hints: {workspace_import_hints:?}

## Instructions
1. Implement the required functionality
2. Follow the interface signature exactly
3. Maintain all specified invariants
4. Avoid all forbidden patterns
5. Write clean, well-documented, production-quality code
6. Include proper imports at the top of the file
7. Add type annotations if missing
8. Import any missing modules
9. Restrict all file edits to `Allowed Output Paths` only
10. If another file needs changes, do not modify it in this node; keep that need implicit for its owning node
11. Use `Workspace Import Hints` exactly for crate/package imports in tests, entry points, and cross-file references
12. For library source modules (e.g. `src/*.rs` in Rust), use `crate::` for intra-crate imports, never the package name. Only use the package name in `tests/`, `examples/`, or `main.rs`.
13. When your code uses external crates/packages not already listed in the project manifest (e.g. `Cargo.toml`, `pyproject.toml`, `package.json`), you MUST include the install commands in the `commands` array. For Rust: `cargo add <crate>` (with `--features <f>` if needed). For Python: `uv add <pkg>`. For Node.js: `npm install <pkg>`. Without these commands, the build will fail due to missing dependencies.
14. For Python projects:
    - Prefer src-layout: put all library code under `src/<package_name>/` with an `__init__.py`.
    - Keep ALL modules inside the declared package directory — never mix top-level .py files with `src/<pkg>/` modules.
    - Use relative imports (`from . import utils`, `from .core import Pipeline`) inside the package.
    - Use the package name for imports from tests and entry points (`from mypackage.core import Foo`), never `src.mypackage`.
    - Put tests in a top-level `tests/` directory (not inside `src/`), using `test_*.py` naming.
    - Use `uv add <pkg>` (not `pip install`) for dependency commands. Use `uv add --dev <pkg>` for test/dev-only tools like `pytest` or `ruff`.
    - Ensure test files import real symbols that actually exist in the generated code — do not invent class or function names that are not defined.

{output_format}"#,
            goal = node.goal,
            interface = contract.interface_signature,
            invariants = contract.invariants,
            forbidden = contract.forbidden_patterns,
            working_dir = ctx.working_dir,
            context_files = node.context_files,
            target_file = target_file,
            allowed_output_paths = allowed_output_paths,
            workspace_import_hints = workspace_import_hints,
            output_format = output_format_section,
        )
    }

    fn workspace_import_hints(working_dir: &Path) -> Vec<String> {
        let mut hints = Vec::new();

        if let Some(crate_name) = Self::detect_rust_crate_name(working_dir) {
            hints.push(format!(
                "Rust crate name: {}. Integration tests and external modules must import via `{}`.",
                crate_name, crate_name
            ));
        }

        if let Some(package_name) = Self::detect_python_package_name(working_dir) {
            hints.push(format!(
                "Python package import root: {}. Tests and entry points must import `{}` and never `src.{}`.",
                package_name, package_name, package_name
            ));
        }

        hints
    }

    fn detect_rust_crate_name(working_dir: &Path) -> Option<String> {
        let cargo_toml = fs::read_to_string(working_dir.join("Cargo.toml")).ok()?;
        let mut in_package = false;

        for raw_line in cargo_toml.lines() {
            let line = raw_line.trim();
            if line.starts_with('[') {
                in_package = line == "[package]";
                continue;
            }

            if in_package && line.starts_with("name") {
                let (_, value) = line.split_once('=')?;
                return Some(value.trim().trim_matches('"').to_string());
            }
        }

        None
    }

    fn detect_python_package_name(working_dir: &Path) -> Option<String> {
        let src_dir = working_dir.join("src");
        if let Ok(entries) = fs::read_dir(&src_dir) {
            for entry in entries.flatten() {
                if entry.file_type().ok()?.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !name.starts_with('.') {
                        return Some(name);
                    }
                }
            }
        }

        let pyproject = fs::read_to_string(working_dir.join("pyproject.toml")).ok()?;
        let mut in_project = false;
        for raw_line in pyproject.lines() {
            let line = raw_line.trim();
            if line.starts_with('[') {
                in_project = line == "[project]";
                continue;
            }

            if in_project && line.starts_with("name") {
                let (_, value) = line.split_once('=')?;
                return Some(value.trim().trim_matches('"').replace('-', "_"));
            }
        }

        None
    }
}

#[async_trait]
impl Agent for ActuatorAgent {
    async fn process(&self, node: &SRBNNode, ctx: &AgentContext) -> Result<AgentMessage> {
        log::info!(
            "[Actuator] Processing node: {} with model {}",
            node.node_id,
            self.model
        );

        let prompt = self.build_coding_prompt(node, ctx);

        let response = self
            .provider
            .generate_response_simple(&self.model, &prompt)
            .await?;

        Ok(AgentMessage::new(ModelTier::Actuator, response))
    }

    fn name(&self) -> &str {
        "Actuator"
    }

    fn can_handle(&self, node: &SRBNNode) -> bool {
        matches!(node.tier, ModelTier::Actuator)
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn build_prompt(&self, node: &SRBNNode, ctx: &AgentContext) -> String {
        self.build_coding_prompt(node, ctx)
    }
}

/// Verifier agent - handles stability verification and contract checking
pub struct VerifierAgent {
    model: String,
    provider: Arc<GenAIProvider>,
}

impl VerifierAgent {
    pub fn new(provider: Arc<GenAIProvider>, model: Option<String>) -> Self {
        Self {
            model: model.unwrap_or_else(|| ModelTier::Verifier.default_model().to_string()),
            provider,
        }
    }

    pub fn build_verification_prompt(&self, node: &SRBNNode, implementation: &str) -> String {
        let contract = &node.contract;

        format!(
            r#"You are a Verifier agent responsible for checking code correctness.

## Task
Verify the implementation satisfies the behavioral contract.

## Behavioral Contract
Interface Signature: {}
Invariants: {:?}
Forbidden Patterns: {:?}
Weighted Tests: {:?}

## Implementation
{}

## Verification Criteria
1. Does the interface match the signature?
2. Are all invariants satisfied?
3. Are any forbidden patterns present?
4. Would the weighted tests pass?

## Output Format
Provide:
- PASS or FAIL status
- Energy score (0.0 = perfect, 1.0 = total failure)
- List of violations if any
- Suggested fixes for each violation"#,
            contract.interface_signature,
            contract.invariants,
            contract.forbidden_patterns,
            contract.weighted_tests,
            implementation,
        )
    }
}

#[async_trait]
impl Agent for VerifierAgent {
    async fn process(&self, node: &SRBNNode, ctx: &AgentContext) -> Result<AgentMessage> {
        log::info!(
            "[Verifier] Processing node: {} with model {}",
            node.node_id,
            self.model
        );

        // In a real implementation, we would get the actual implementation from the context
        let implementation = ctx
            .history
            .last()
            .map(|m| m.content.as_str())
            .unwrap_or("No implementation provided");

        let prompt = self.build_verification_prompt(node, implementation);

        let response = self
            .provider
            .generate_response_simple(&self.model, &prompt)
            .await?;

        Ok(AgentMessage::new(ModelTier::Verifier, response))
    }

    fn name(&self) -> &str {
        "Verifier"
    }

    fn can_handle(&self, node: &SRBNNode) -> bool {
        matches!(node.tier, ModelTier::Verifier)
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn build_prompt(&self, node: &SRBNNode, _ctx: &AgentContext) -> String {
        // Verifier needs implementation context, use a placeholder
        self.build_verification_prompt(node, "<implementation>")
    }
}

/// Speculator agent - handles fast lookahead for exploration
pub struct SpeculatorAgent {
    model: String,
    provider: Arc<GenAIProvider>,
}

impl SpeculatorAgent {
    pub fn new(provider: Arc<GenAIProvider>, model: Option<String>) -> Self {
        Self {
            model: model.unwrap_or_else(|| ModelTier::Speculator.default_model().to_string()),
            provider,
        }
    }
}

#[async_trait]
impl Agent for SpeculatorAgent {
    async fn process(&self, node: &SRBNNode, ctx: &AgentContext) -> Result<AgentMessage> {
        log::info!(
            "[Speculator] Processing node: {} with model {}",
            node.node_id,
            self.model
        );

        let prompt = self.build_prompt(node, ctx);

        let response = self
            .provider
            .generate_response_simple(&self.model, &prompt)
            .await?;

        Ok(AgentMessage::new(ModelTier::Speculator, response))
    }

    fn name(&self) -> &str {
        "Speculator"
    }

    fn can_handle(&self, node: &SRBNNode) -> bool {
        matches!(node.tier, ModelTier::Speculator)
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn build_prompt(&self, node: &SRBNNode, _ctx: &AgentContext) -> String {
        format!("Briefly analyze potential issues for: {}", node.goal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn build_coding_prompt_includes_rust_crate_hint() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"validator_lib\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let provider = Arc::new(GenAIProvider::new().unwrap());
        let agent = ActuatorAgent::new(provider, Some("test-model".into()));
        let mut node = SRBNNode::new("n1".into(), "goal".into(), ModelTier::Actuator);
        node.output_targets.push("tests/integration.rs".into());
        let ctx = AgentContext {
            working_dir: dir.path().to_path_buf(),
            ..Default::default()
        };

        let prompt = agent.build_coding_prompt(&node, &ctx);
        assert!(
            prompt.contains("Rust crate name: validator_lib"),
            "{prompt}"
        );
    }

    #[test]
    fn build_coding_prompt_includes_python_package_hint() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/psp5_python_verify")).unwrap();
        fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nname = \"psp5-python-verify\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let provider = Arc::new(GenAIProvider::new().unwrap());
        let agent = ActuatorAgent::new(provider, Some("test-model".into()));
        let mut node = SRBNNode::new("n1".into(), "goal".into(), ModelTier::Actuator);
        node.output_targets.push("tests/test_main.py".into());
        let ctx = AgentContext {
            working_dir: dir.path().to_path_buf(),
            ..Default::default()
        };

        let prompt = agent.build_coding_prompt(&node, &ctx);
        assert!(
            prompt.contains("Python package import root: psp5_python_verify"),
            "{prompt}"
        );
    }
}
