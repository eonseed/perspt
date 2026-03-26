//! Agent Trait and Implementations
//!
//! Defines the interface for all agent implementations and provides
//! LLM-integrated implementations for Architect, Actuator, and Verifier roles.

use crate::types::{AgentContext, AgentMessage, ModelTier, SRBNNode};
use anyhow::Result;
use async_trait::async_trait;
use perspt_core::llm_provider::GenAIProvider;
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
            &format!("Context Files: {:?}\nOutput Targets: {:?}", node.context_files, node.output_targets),
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

### MODULAR PROJECT STRUCTURE (CRITICAL)
Your plan MUST create a COMPLETE, RUNNABLE project with proper modularity:

1. **Entry Point First**: Create a main entry point file (e.g., `main.py`, `src/main.rs`, `index.js`)
2. **Logical Modules**: Split functionality into separate files/modules with clear responsibilities
3. **Proper Imports**: Ensure all cross-file imports will resolve correctly
4. **Package Structure**: For Python, include `__init__.py` files in subdirectories
5. **Test Isolation**: Put tests in a `tests/` directory or use `test_*.py` naming

### TASK ORDERING
1. Create foundational modules before dependent ones
2. Specify dependencies accurately between tasks
3. Entry point task should depend on all modules it imports

### COMPLETENESS CHECKLIST
- [ ] Every import in generated code must reference an existing or planned file
- [ ] The project must be immediately runnable after all tasks complete
- [ ] Include at least one test file for core functionality
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
      "goal": "Description of what this task accomplishes",
      "context_files": ["existing_file.py"],
      "output_files": ["new_file.py"],
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
      "id": "main_entry",
      "goal": "Create main.py entry point that imports and uses other modules",
      "context_files": ["module_a.py", "module_b.py"],
      "output_files": ["main.py"],
      "dependencies": ["task_1", "task_2"],
      "task_type": "code"
    }},
    {{
      "id": "test_1",
      "goal": "Unit tests for task_1",
      "context_files": ["new_file.py"],
      "output_files": ["tests/test_new_file.py"],
      "dependencies": ["task_1"],
      "task_type": "unit_test"
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
  "commands": []
}}
```

Each artifact entry must have:
- `path`: Relative path within the workspace
- `operation`: Either `"write"` (full file) or `"diff"` (unified diff patch)
- `content` (for write) or `patch` (for diff): The file content or patch

RULES:
- Paths MUST be relative (no leading `/`)
- Use `"write"` for new files or full rewrites
- Use `"diff"` with proper unified diff format for small changes to existing files
- Include ALL files needed for the task in a single bundle"#,
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

## Instructions
1. Implement the required functionality
2. Follow the interface signature exactly
3. Maintain all specified invariants
4. Avoid all forbidden patterns
5. Write clean, well-documented, production-quality code
6. Include proper imports at the top of the file
7. Add type annotations if missing
8. Import any missing modules

{output_format}"#,
            goal = node.goal,
            interface = contract.interface_signature,
            invariants = contract.invariants,
            forbidden = contract.forbidden_patterns,
            working_dir = ctx.working_dir,
            context_files = node.context_files,
            target_file = target_file,
            output_format = output_format_section,
        )
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
    // Note: Integration tests would require actual API keys
    // These are unit tests for the prompt building logic

    #[test]
    fn test_architect_prompt_building() {
        // Would need provider mock for full test
    }
}
