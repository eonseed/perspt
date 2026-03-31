//! Externalized prompt templates for agent roles.
//!
//! Each public constant holds a prompt template with `{placeholder}` markers.
//! Typed `render_*` helpers substitute them via `.replace()` — this can be
//! upgraded to MiniJinja later if conditional blocks or loops become necessary.

/// Architect prompt for *existing-project* task decomposition.
///
/// Placeholders: `{task}`, `{working_dir}`, `{project_context}`,
/// `{error_feedback}`, `{evidence_section}`.
pub const ARCHITECT_EXISTING: &str = r#"You are an Architect agent in a multi-agent coding system.

## Task
{task}

## Working Directory
{working_dir}

## Existing Project Structure
{project_context}
{error_feedback}
{evidence_section}
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

## CRITICAL CONSTRAINTS — MANIFEST FILES
- DO NOT create the ROOT project manifest (`Cargo.toml` at workspace root, `pyproject.toml` at project root, `package.json` at project root) — the system manages it automatically.
- For **multi-crate Rust workspaces** (when you plan `crates/<name>/` sub-directories), you MUST include each sub-crate's `Cargo.toml` in the owning task's `output_files` (e.g., `crates/my-lib/Cargo.toml`). The system will automatically convert the root manifest to a `[workspace]`.
- For **Python** projects, DO NOT create `pyproject.toml` — the system handles it.
- For **Node.js** projects, DO NOT create the root `package.json` — the system handles it. Sub-package `package.json` files in `packages/*/` are allowed.
- If you need to add dependencies, include them in `dependency_expectations.required_packages`.

### WORKSPACE / MULTI-CRATE PROJECTS
When the task asks for multiple crates, packages, or modules in subdirectories:
- **Rust**: Put each crate under `crates/<name>/` with its own `Cargo.toml` and `src/lib.rs` (or `src/main.rs` for binaries). The root `Cargo.toml` will be auto-converted to `[workspace]` with `members = ["crates/*"]`.
- **Python**: Keep all code under `src/<package_name>/` with submodules. Multiple top-level packages are not standard in Python — use submodules instead.
- **Node.js**: Put each package under `packages/<name>/` with its own `package.json`.

Do NOT place source files directly in the root `src/` directory when planning sub-crates under `crates/` — each crate must be self-contained.

### DEPENDENCY EXPECTATIONS
For each task, declare the packages/crates the generated code will import under `dependency_expectations`:
- `required_packages`: list of third-party packages the task's code imports (e.g., `["requests", "pydantic"]` or `["serde", "tokio"]`)
- `setup_commands`: commands that must succeed before this task runs (e.g., `["cargo fetch"]`)
- `min_toolchain_version`: optional minimum toolchain version string (e.g., `"1.75"` for Rust, `"3.11"` for Python)
Only include EXTERNAL / third-party dependencies, not standard-library modules.

## Output Format
Respond with ONLY a JSON object in this exact format:
```json
{OPEN_BRACE}
  "tasks": [
    {OPEN_BRACE}
      "id": "task_1",
      "goal": "Create module_a with core functionality",
      "context_files": [],
      "output_files": ["module_a.py"],
      "dependencies": [],
      "task_type": "code",
      "dependency_expectations": {OPEN_BRACE}
        "required_packages": [],
        "setup_commands": [],
        "min_toolchain_version": null
      {CLOSE_BRACE},
      "contract": {OPEN_BRACE}
        "interface_signature": "def function_name(arg: Type) -> ReturnType",
        "invariants": ["Must handle edge cases"],
        "forbidden_patterns": ["no bare except"],
        "tests": [
          {OPEN_BRACE}"name": "test_function_name", "criticality": "Critical"{CLOSE_BRACE}
        ]
      {CLOSE_BRACE}
    {CLOSE_BRACE},
    {OPEN_BRACE}
      "id": "test_task_1",
      "goal": "Unit tests for module_a (ONLY this module)",
      "context_files": ["module_a.py"],
      "output_files": ["tests/test_module_a.py"],
      "dependencies": ["task_1"],
      "task_type": "unit_test"
    {CLOSE_BRACE},
    {OPEN_BRACE}
      "id": "task_2",
      "goal": "Create module_b with helper utilities",
      "context_files": [],
      "output_files": ["module_b.py"],
      "dependencies": [],
      "task_type": "code"
    {CLOSE_BRACE},
    {OPEN_BRACE}
      "id": "test_task_2",
      "goal": "Unit tests for module_b (ONLY this module)",
      "context_files": ["module_b.py"],
      "output_files": ["tests/test_module_b.py"],
      "dependencies": ["task_2"],
      "task_type": "unit_test"
    {CLOSE_BRACE},
    {OPEN_BRACE}
      "id": "main_entry",
      "goal": "Create main.py entry point that imports and uses other modules",
      "context_files": ["module_a.py", "module_b.py"],
      "output_files": ["main.py"],
      "dependencies": ["task_1", "task_2"],
      "task_type": "code"
    {CLOSE_BRACE}
  ]
{CLOSE_BRACE}
```

Valid task_type values: "code", "unit_test", "integration_test", "refactor", "documentation"
Valid criticality values: "Critical", "High", "Low"

IMPORTANT: Output ONLY the JSON, no other text."#;

/// Architect prompt for *greenfield* task decomposition.
///
/// Identical structure to [`ARCHITECT_EXISTING`] but omits the evidence
/// section and adjusts framing for empty-workspace contexts.
/// Placeholders: `{task}`, `{working_dir}`, `{project_context}`,
/// `{error_feedback}`.
pub const ARCHITECT_GREENFIELD: &str = r#"You are an Architect agent in a multi-agent coding system planning a NEW project from scratch.

## Task
{task}

## Working Directory
{working_dir}

## Project Context
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

### GREENFIELD PROJECT LAYOUT
Since this is a new project, design the file structure from scratch:
1. **Entry Point First**: Create a main entry point file (e.g., `main.py`, `src/main.rs`, `index.js`)
2. **Logical Modules**: Split functionality into separate files/modules with clear responsibilities
3. **Proper Imports**: Ensure all cross-file imports will resolve correctly
4. **Package Structure**: For Python, include `__init__.py` files in subdirectories
5. **One Test Task Per Module**: Each module's tests go in their OWN task with a UNIQUE test file

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

## CRITICAL CONSTRAINTS — MANIFEST FILES
- DO NOT create the ROOT project manifest (`Cargo.toml` at workspace root, `pyproject.toml` at project root, `package.json` at project root) — the system manages it automatically.
- For **multi-crate Rust workspaces** (when you plan `crates/<name>/` sub-directories), you MUST include each sub-crate's `Cargo.toml` in the owning task's `output_files` (e.g., `crates/my-lib/Cargo.toml`). The system will automatically convert the root manifest to a `[workspace]`.
- For **Python** projects, DO NOT create `pyproject.toml` — the system handles it.
- For **Node.js** projects, DO NOT create the root `package.json` — the system handles it. Sub-package `package.json` files in `packages/*/` are allowed.
- If you need to add dependencies, include them in `dependency_expectations.required_packages`.

### WORKSPACE / MULTI-CRATE PROJECTS
When the task asks for multiple crates, packages, or modules in subdirectories:
- **Rust**: Put each crate under `crates/<name>/` with its own `Cargo.toml` and `src/lib.rs` (or `src/main.rs` for binaries). The root `Cargo.toml` will be auto-converted to `[workspace]` with `members = ["crates/*"]`.
- **Python**: Keep all code under `src/<package_name>/` with submodules. Multiple top-level packages are not standard in Python — use submodules instead.
- **Node.js**: Put each package under `packages/<name>/` with its own `package.json`.

Do NOT place source files directly in the root `src/` directory when planning sub-crates under `crates/` — each crate must be self-contained.

### DEPENDENCY EXPECTATIONS
For each task, declare the packages/crates the generated code will import under `dependency_expectations`:
- `required_packages`: list of third-party packages the task's code imports (e.g., `["requests", "pydantic"]` or `["serde", "tokio"]`)
- `setup_commands`: commands that must succeed before this task runs (e.g., `["cargo fetch"]`)
- `min_toolchain_version`: optional minimum toolchain version string (e.g., `"1.75"` for Rust, `"3.11"` for Python)
Only include EXTERNAL / third-party dependencies, not standard-library modules.

## Output Format
Respond with ONLY a JSON object in this exact format:
```json
{OPEN_BRACE}
  "tasks": [
    {OPEN_BRACE}
      "id": "task_1",
      "goal": "Create module_a with core functionality",
      "context_files": [],
      "output_files": ["module_a.py"],
      "dependencies": [],
      "task_type": "code",
      "dependency_expectations": {OPEN_BRACE}
        "required_packages": [],
        "setup_commands": [],
        "min_toolchain_version": null
      {CLOSE_BRACE},
      "contract": {OPEN_BRACE}
        "interface_signature": "def function_name(arg: Type) -> ReturnType",
        "invariants": ["Must handle edge cases"],
        "forbidden_patterns": ["no bare except"],
        "tests": [
          {OPEN_BRACE}"name": "test_function_name", "criticality": "Critical"{CLOSE_BRACE}
        ]
      {CLOSE_BRACE}
    {CLOSE_BRACE},
    {OPEN_BRACE}
      "id": "test_task_1",
      "goal": "Unit tests for module_a (ONLY this module)",
      "context_files": ["module_a.py"],
      "output_files": ["tests/test_module_a.py"],
      "dependencies": ["task_1"],
      "task_type": "unit_test"
    {CLOSE_BRACE},
    {OPEN_BRACE}
      "id": "main_entry",
      "goal": "Create main.py entry point that imports and uses other modules",
      "context_files": ["module_a.py"],
      "output_files": ["main.py"],
      "dependencies": ["task_1"],
      "task_type": "code"
    {CLOSE_BRACE}
  ]
{CLOSE_BRACE}
```

Valid task_type values: "code", "unit_test", "integration_test", "refactor", "documentation"
Valid criticality values: "Critical", "High", "Low"

IMPORTANT: Output ONLY the JSON, no other text."#;

// Brace constants used inside raw strings where `{{` / `}}` are not legal.
const OPEN_BRACE: &str = "{";
const CLOSE_BRACE: &str = "}";

/// Render an architect prompt template by replacing named placeholders.
///
/// `active_plugins` is an optional list of detected language plugins (e.g.
/// `["rust", "python"]`).  When non-empty, the rendered prompt includes a
/// "Detected Toolchain" section so the architect can plan verification-aware
/// nodes.
///
/// # Panics
/// Does not panic; missing placeholders are left as-is.
pub fn render_architect(
    template: &str,
    task: &str,
    working_dir: &std::path::Path,
    project_context: &str,
    error_feedback: &str,
    evidence_section: &str,
    active_plugins: &[String],
) -> String {
    let plugin_section = if active_plugins.is_empty() {
        String::new()
    } else {
        format!(
            "\n## Detected Toolchain\nActive language plugins: {}\nPlan verification-aware nodes that align with these plugins' build/test capabilities.\n",
            active_plugins.join(", ")
        )
    };
    let enriched_context = if plugin_section.is_empty() {
        project_context.to_string()
    } else {
        format!("{}{}", project_context, plugin_section)
    };
    template
        .replace("{task}", task)
        .replace("{working_dir}", &working_dir.display().to_string())
        .replace("{project_context}", &enriched_context)
        .replace("{error_feedback}", error_feedback)
        .replace("{evidence_section}", evidence_section)
        .replace("{OPEN_BRACE}", OPEN_BRACE)
        .replace("{CLOSE_BRACE}", CLOSE_BRACE)
}

// ---------------------------------------------------------------------------
// Actuator prompts
// ---------------------------------------------------------------------------

/// Actuator prompt body for code generation.
///
/// Placeholders: `{goal}`, `{interface}`, `{invariants}`, `{forbidden}`,
/// `{working_dir}`, `{context_files}`, `{target_file}`,
/// `{allowed_output_paths}`, `{workspace_import_hints}`, `{output_format}`.
pub const ACTUATOR_CODING: &str = r#"You are an Actuator agent responsible for implementing code.

## Task
Goal: {goal}

## Behavioral Contract
Interface Signature: {interface}
Invariants: {invariants}
Forbidden Patterns: {forbidden}

## Context
Working Directory: {working_dir}
Files to Read: {context_files}
Target Output File: {target_file}
Allowed Output Paths: {allowed_output_paths}
Workspace Import Hints: {workspace_import_hints}

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

{output_format}"#;

/// Actuator output format section for multi-artifact bundle mode.
///
/// Placeholders: `{target_file}`, `{OPEN_BRACE}`, `{CLOSE_BRACE}`.
pub const ACTUATOR_MULTI_OUTPUT: &str = r#"## Output Format (Multi-Artifact Bundle)
When producing multi-file output, use this JSON format wrapped in a ```json code block:

```json
{OPEN_BRACE}
  "artifacts": [
    {OPEN_BRACE} "path": "{target_file}", "operation": "write", "content": "..." {CLOSE_BRACE},
    {OPEN_BRACE} "path": "tests/test_main.py", "operation": "write", "content": "..." {CLOSE_BRACE}
  ],
  "commands": ["cargo add serde --features derive", "cargo add thiserror"]
{CLOSE_BRACE}
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
- DO NOT create, modify, or patch any file not listed in `Allowed Output Paths`"#;

/// Actuator output format section for single-file mode.
///
/// Placeholders: `{target_file}`.
pub const ACTUATOR_SINGLE_OUTPUT: &str = r#"## Output Format
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
- Use 'File:' ONLY for new files or full rewrites"#;

/// Render an actuator coding prompt, selecting the appropriate output format.
///
/// All value arguments are pre-formatted strings. The caller is responsible
/// for Debug-formatting slices/paths before passing them here.
#[allow(clippy::too_many_arguments)]
pub fn render_actuator(
    goal: &str,
    interface: &str,
    invariants: &str,
    forbidden: &str,
    working_dir: &str,
    context_files: &str,
    target_file: &str,
    allowed_output_paths: &str,
    workspace_import_hints: &str,
    is_multi_output: bool,
) -> String {
    let output_format = if is_multi_output {
        ACTUATOR_MULTI_OUTPUT
            .replace("{target_file}", target_file)
            .replace("{OPEN_BRACE}", OPEN_BRACE)
            .replace("{CLOSE_BRACE}", CLOSE_BRACE)
    } else {
        ACTUATOR_SINGLE_OUTPUT.replace("{target_file}", target_file)
    };

    ACTUATOR_CODING
        .replace("{goal}", goal)
        .replace("{interface}", interface)
        .replace("{invariants}", invariants)
        .replace("{forbidden}", forbidden)
        .replace("{working_dir}", working_dir)
        .replace("{context_files}", context_files)
        .replace("{target_file}", target_file)
        .replace("{allowed_output_paths}", allowed_output_paths)
        .replace("{workspace_import_hints}", workspace_import_hints)
        .replace("{output_format}", &output_format)
}

// ---------------------------------------------------------------------------
// Verifier prompts
// ---------------------------------------------------------------------------

/// Verifier prompt for contract checking.
///
/// Placeholders: `{interface}`, `{invariants}`, `{forbidden}`,
/// `{weighted_tests}`, `{implementation}`.
pub const VERIFIER_CHECK: &str = r#"You are a Verifier agent responsible for checking code correctness.

## Task
Verify the implementation satisfies the behavioral contract.

## Behavioral Contract
Interface Signature: {interface}
Invariants: {invariants}
Forbidden Patterns: {forbidden}
Weighted Tests: {weighted_tests}

## Implementation
{implementation}

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
- Suggested fixes for each violation"#;

/// Render a verifier prompt with pre-formatted values.
pub fn render_verifier(
    interface: &str,
    invariants: &str,
    forbidden: &str,
    weighted_tests: &str,
    implementation: &str,
) -> String {
    VERIFIER_CHECK
        .replace("{interface}", interface)
        .replace("{invariants}", invariants)
        .replace("{forbidden}", forbidden)
        .replace("{weighted_tests}", weighted_tests)
        .replace("{implementation}", implementation)
}

// ---------------------------------------------------------------------------
// Speculator prompts
// ---------------------------------------------------------------------------

/// Minimal speculator prompt for quick issue analysis.
///
/// Placeholder: `{goal}`.
pub const SPECULATOR_BASIC: &str = "Briefly analyze potential issues for: {goal}";

/// Speculator lookahead prompt for interface contract prediction.
///
/// Placeholders: `{node_id}`, `{goal}`, `{downstream}`.
pub const SPECULATOR_LOOKAHEAD: &str =
    "You are a Speculator agent. Given this task and its downstream dependents, \
produce a brief (3-5 bullet) list of:\n\
1. Interface contracts the current task must satisfy for dependents\n\
2. Common pitfalls (e.g., import paths, missing exports)\n\
3. Edge cases dependents may need\n\n\
Current task: {node_id} — {goal}\n\
Downstream tasks:\n{downstream}\n\n\
Be concise. No code.";

/// Render a speculator lookahead prompt.
pub fn render_speculator_lookahead(node_id: &str, goal: &str, downstream: &str) -> String {
    SPECULATOR_LOOKAHEAD
        .replace("{node_id}", node_id)
        .replace("{goal}", goal)
        .replace("{downstream}", downstream)
}

// ---------------------------------------------------------------------------
// Solo-mode prompts
// ---------------------------------------------------------------------------

/// Solo-mode prompt for single-file Python generation.
///
/// Placeholder: `{task}`.
pub const SOLO_GENERATE: &str = r#"You are an expert Python developer. Complete this task with a SINGLE, self-contained Python file.

## Task
{task}

## Requirements
1. Choose a DESCRIPTIVE filename based on the task (e.g., `fibonacci.py` for a fibonacci script, `prime_checker.py` for checking primes)
2. Write ONE Python file that accomplishes the task
3. Include docstrings with doctest examples for all functions
4. Make the file directly runnable with `if __name__ == "__main__":` block
5. Use type hints for all function parameters and return values

## Output Format
File: <your_descriptive_filename.py>
```python
# your complete code here
```

IMPORTANT: Do NOT use generic names like `script.py` or `main.py`. Choose a name that reflects the task."#;

/// Solo-mode correction prompt.
///
/// Placeholders: `{task}`, `{filename}`, `{current_code}`, `{v_syn}`,
/// `{v_log}`, `{v_boot}`, `{error_list}`.
pub const SOLO_CORRECTION: &str = r#"## Code Correction Required

The code you generated has errors. Fix ALL of them.

### Original Task
{task}

### Current Code ({filename})
```python
{current_code}
```

### Errors Found
Energy: V_syn={v_syn}, V_log={v_log}, V_boot={v_boot}

{error_list}

### Instructions
1. Fix ALL errors listed above
2. Maintain the original functionality
3. Ensure the script runs without errors
4. Ensure all doctests pass
5. Return the COMPLETE corrected file

### Output Format
File: {filename}
```python
[complete corrected code]
```"#;

/// Render a solo correction prompt with pre-formatted energy values.
pub fn render_solo_correction(
    task: &str,
    filename: &str,
    current_code: &str,
    v_syn: &str,
    v_log: &str,
    v_boot: &str,
    error_list: &str,
) -> String {
    SOLO_CORRECTION
        .replace("{task}", task)
        .replace("{filename}", filename)
        .replace("{current_code}", current_code)
        .replace("{v_syn}", v_syn)
        .replace("{v_log}", v_log)
        .replace("{v_boot}", v_boot)
        .replace("{error_list}", error_list)
}

// ---------------------------------------------------------------------------
// Initialization prompts
// ---------------------------------------------------------------------------

/// Prompt for LLM-based project name suggestion.
///
/// Placeholder: `{task}`.
pub const PROJECT_NAME_SUGGEST: &str = r#"Extract a short project name from this task description.
Rules:
- Use snake_case (lowercase with underscores)
- Maximum 30 characters
- Must be a valid folder name (letters, numbers, underscores only)
- Return ONLY the name, nothing else

Task: "{task}"

Project name:"#;

// ---------------------------------------------------------------------------
// Convergence / correction prompts
// ---------------------------------------------------------------------------

/// Preamble for the verifier-guided analysis stage during correction.
///
/// The correction prompt body is appended after this preamble.
pub const VERIFIER_ANALYSIS_PREAMBLE: &str = "\
You are a Verifier agent. Analyze the following correction request and produce \
concise, structured guidance for the code fixer. Identify:\n\
1. Root cause of each failure\n\
2. Which specific functions/lines need changes\n\
3. Constraints that must be preserved\n\
Do NOT produce code — only analysis and guidance.\n\n";

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_architect_existing_contains_placeholders() {
        assert!(ARCHITECT_EXISTING.contains("{task}"));
        assert!(ARCHITECT_EXISTING.contains("{working_dir}"));
        assert!(ARCHITECT_EXISTING.contains("{project_context}"));
        assert!(ARCHITECT_EXISTING.contains("{evidence_section}"));
        assert!(ARCHITECT_EXISTING.contains("dependency_expectations"));
    }

    #[test]
    fn test_architect_greenfield_omits_evidence() {
        assert!(!ARCHITECT_GREENFIELD.contains("{evidence_section}"));
        assert!(ARCHITECT_GREENFIELD.contains("GREENFIELD"));
    }

    #[test]
    fn test_render_architect_substitutes_placeholders() {
        let result = render_architect(
            ARCHITECT_EXISTING,
            "Build a web app",
            Path::new("/tmp/project"),
            "has Cargo.toml",
            "",
            "## Evidence\nfound 3 modules",
            &["rust".to_string()],
        );
        assert!(result.contains("Build a web app"));
        assert!(result.contains("/tmp/project"));
        assert!(result.contains("has Cargo.toml"));
        assert!(result.contains("## Evidence\nfound 3 modules"));
        // Braces should be resolved
        assert!(!result.contains("{OPEN_BRACE}"));
        assert!(!result.contains("{CLOSE_BRACE}"));
        // JSON example should have real braces
        assert!(result.contains(r#""tasks": ["#));
        // Plugin info should be included
        assert!(result.contains("rust"));
        assert!(result.contains("Detected Toolchain"));
    }

    #[test]
    fn test_render_architect_greenfield() {
        let result = render_architect(
            ARCHITECT_GREENFIELD,
            "Build a CLI tool",
            Path::new("/tmp/new"),
            "empty directory",
            "",
            "", // no evidence for greenfield
            &["python".to_string()],
        );
        assert!(result.contains("Build a CLI tool"));
        assert!(result.contains("NEW project from scratch"));
        assert!(result.contains("python"));
        assert!(result.contains("Detected Toolchain"));
    }

    #[test]
    fn test_render_architect_no_plugins() {
        let result = render_architect(
            ARCHITECT_EXISTING,
            "fix a bug",
            Path::new("/tmp/proj"),
            "context",
            "",
            "",
            &[],
        );
        assert!(!result.contains("Detected Toolchain"));
    }

    #[test]
    fn test_actuator_contains_placeholders() {
        assert!(ACTUATOR_CODING.contains("{goal}"));
        assert!(ACTUATOR_CODING.contains("{interface}"));
        assert!(ACTUATOR_CODING.contains("{output_format}"));
    }

    #[test]
    fn test_render_actuator_multi_output() {
        let result = render_actuator(
            "Implement parser",
            "fn parse(input: &str) -> AST",
            r#"["valid input"]"#,
            r#"["no panics"]"#,
            "/tmp/proj",
            r#"["lib.rs"]"#,
            "src/parser.rs",
            r#"["src/parser.rs"]"#,
            r#"["Rust crate: my_crate"]"#,
            true,
        );
        assert!(result.contains("Implement parser"));
        assert!(result.contains("Multi-Artifact Bundle"));
        assert!(result.contains("src/parser.rs"));
        // JSON braces should be resolved
        assert!(!result.contains("{OPEN_BRACE}"));
    }

    #[test]
    fn test_render_actuator_single_output() {
        let result = render_actuator(
            "Fix bug",
            "fn fix() -> bool",
            "[]",
            "[]",
            "/tmp",
            "[]",
            "main.py",
            r#"["main.py"]"#,
            "[]",
            false,
        );
        assert!(result.contains("Fix bug"));
        assert!(!result.contains("Multi-Artifact Bundle"));
        assert!(result.contains("File: main.py"));
    }

    #[test]
    fn test_render_verifier() {
        let result = render_verifier(
            "fn compute() -> i32",
            r#"["returns positive"]"#,
            r#"["no unwrap"]"#,
            r#"[WeightedTest { name: "test_compute", criticality: Critical }]"#,
            "fn compute() -> i32 { 42 }",
        );
        assert!(result.contains("fn compute() -> i32"));
        assert!(result.contains("returns positive"));
        assert!(result.contains("fn compute() -> i32 { 42 }"));
    }

    #[test]
    fn test_speculator_lookahead() {
        let result = render_speculator_lookahead(
            "task_1",
            "Build core module",
            "- task_2: Build tests\n- task_3: Build CLI",
        );
        assert!(result.contains("task_1"));
        assert!(result.contains("Build core module"));
        assert!(result.contains("task_2: Build tests"));
    }

    #[test]
    fn test_solo_generate_placeholder() {
        assert!(SOLO_GENERATE.contains("{task}"));
        assert!(SOLO_GENERATE.contains("SINGLE, self-contained Python file"));
    }

    #[test]
    fn test_render_solo_correction() {
        let result = render_solo_correction(
            "Build calculator",
            "calc.py",
            "def add(a, b): return a + b",
            "0.50",
            "0.00",
            "0.00",
            "- Line 1: missing type hints [WARNING]",
        );
        assert!(result.contains("Build calculator"));
        assert!(result.contains("calc.py"));
        assert!(result.contains("V_syn=0.50"));
        assert!(result.contains("missing type hints"));
    }

    #[test]
    fn test_project_name_placeholder() {
        assert!(PROJECT_NAME_SUGGEST.contains("{task}"));
        assert!(PROJECT_NAME_SUGGEST.contains("snake_case"));
    }

    #[test]
    fn test_verifier_analysis_preamble() {
        assert!(VERIFIER_ANALYSIS_PREAMBLE.contains("Verifier agent"));
        assert!(VERIFIER_ANALYSIS_PREAMBLE.contains("Root cause"));
    }
}
