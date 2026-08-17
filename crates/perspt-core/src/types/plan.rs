use super::*;

// =============================================================================
// Task Plan Types - Structured output from Architect
// =============================================================================

/// Task type classification for planning
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// Implementation code
    #[default]
    #[serde(
        alias = "implementation",
        alias = "impl",
        alias = "feature",
        alias = "source"
    )]
    Code,
    /// Shell command execution (e.g., cargo new, npm init)
    #[serde(alias = "shell", alias = "scaffold", alias = "setup", alias = "init")]
    Command,
    /// Unit tests. Models commonly emit the bare word "test"/"tests", so accept
    /// those as aliases — rejecting them was forcing valid plans to fail and
    /// fall back to the deterministic graph.
    #[serde(
        alias = "test",
        alias = "tests",
        alias = "unittest",
        alias = "unit-test"
    )]
    UnitTest,
    /// Integration/E2E tests
    #[serde(alias = "integration", alias = "e2e", alias = "integration-test")]
    IntegrationTest,
    /// Refactoring existing code
    #[serde(alias = "refactoring")]
    Refactor,
    /// Documentation
    #[serde(alias = "docs", alias = "doc")]
    Documentation,
}

/// Structured task plan from Architect
/// Output as JSON for reliable parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    /// List of tasks to execute
    pub tasks: Vec<PlannedTask>,
}

impl TaskPlan {
    /// Create an empty plan
    pub fn new() -> Self {
        Self { tasks: Vec::new() }
    }

    /// Get the total number of tasks
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Check if plan is empty
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Get task by ID
    pub fn get_task(&self, id: &str) -> Option<&PlannedTask> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// Validate the plan structure
    pub fn validate(&self) -> Result<(), String> {
        if self.tasks.is_empty() {
            return Err("Plan has no tasks".to_string());
        }

        // Check for duplicate IDs
        let mut seen_ids = std::collections::HashSet::new();
        for task in &self.tasks {
            if !seen_ids.insert(&task.id) {
                return Err(format!("Duplicate task ID: {}", task.id));
            }
            if task.goal.is_empty() {
                return Err(format!("Task {} has empty goal", task.id));
            }
        }

        // Check for invalid dependencies
        for task in &self.tasks {
            for dep in &task.dependencies {
                if !seen_ids.contains(dep) {
                    return Err(format!("Task {} has unknown dependency: {}", task.id, dep));
                }
            }
        }

        let file_owners = self.validate_ownership()?;
        self.validate_acyclic()?;

        // PSP-7: Implicit dependency enforcement — if task A reads a file that
        // task B produces (context_files ∩ output_files), A must depend on B.
        for task in &self.tasks {
            for ctx_file in &task.context_files {
                if let Some(&owner) = file_owners.get(ctx_file.as_str()) {
                    if owner != task.id && !task.dependencies.iter().any(|d| d == owner) {
                        return Err(format!(
                            "Task '{}' reads '{}' produced by '{}' but does not declare it as a dependency",
                            task.id, ctx_file, owner
                        ));
                    }
                }
            }
        }

        self.validate_test_task_dependencies()
    }

    /// PSP-5 ownership closure: each output file belongs to exactly one task.
    fn validate_ownership(&self) -> Result<std::collections::HashMap<&str, &str>, String> {
        let mut file_owners: std::collections::HashMap<&str, &str> =
            std::collections::HashMap::new();
        for task in &self.tasks {
            for file in &task.output_files {
                if let Some(prev_owner) = file_owners.insert(file.as_str(), task.id.as_str()) {
                    return Err(format!(
                        "Ownership violation in plan: file '{}' claimed by both '{}' and '{}'. \
                         Each output file must appear in exactly one task's output_files.",
                        file, prev_owner, task.id
                    ));
                }
            }
        }
        Ok(file_owners)
    }

    /// PSP-7 cycle detection via topological sort (Kahn's algorithm).
    fn validate_acyclic(&self) -> Result<(), String> {
        let id_to_idx: std::collections::HashMap<&str, usize> = self
            .tasks
            .iter()
            .enumerate()
            .map(|(i, t)| (t.id.as_str(), i))
            .collect();
        let n = self.tasks.len();
        let mut in_degree = vec![0usize; n];
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (i, task) in self.tasks.iter().enumerate() {
            for dep in &task.dependencies {
                if let Some(&dep_idx) = id_to_idx.get(dep.as_str()) {
                    adj[dep_idx].push(i);
                    in_degree[i] += 1;
                }
            }
        }
        let mut queue: std::collections::VecDeque<usize> = in_degree
            .iter()
            .enumerate()
            .filter_map(|(i, &d)| if d == 0 { Some(i) } else { None })
            .collect();
        let mut visited = 0usize;
        while let Some(node) = queue.pop_front() {
            visited += 1;
            for &next in &adj[node] {
                in_degree[next] -= 1;
                if in_degree[next] == 0 {
                    queue.push_back(next);
                }
            }
        }
        if visited != n {
            return Err("Plan contains a dependency cycle".to_string());
        }
        Ok(())
    }

    /// PSP-7 test-task dependency inference via plugin test_file_patterns().
    /// If a task produces only test files (matching some plugin's test
    /// patterns), it must depend on any task that produces non-test sources.
    fn validate_test_task_dependencies(&self) -> Result<(), String> {
        let registry = crate::plugin::PluginRegistry::new();
        let all_test_patterns: Vec<&str> = registry
            .all()
            .iter()
            .flat_map(|p| p.test_file_patterns().iter().copied())
            .collect();
        if all_test_patterns.is_empty() {
            return Ok(());
        }
        let is_test_file = |path: &str| -> bool {
            all_test_patterns
                .iter()
                .any(|pat| glob_matches_simple(pat, path))
        };
        // Identify test-only tasks and source tasks
        let source_task_ids: Vec<&str> = self
            .tasks
            .iter()
            .filter(|t| {
                !t.output_files.is_empty() && t.output_files.iter().any(|f| !is_test_file(f))
            })
            .map(|t| t.id.as_str())
            .collect();
        for task in &self.tasks {
            if task.output_files.is_empty() {
                continue;
            }
            let all_tests = task.output_files.iter().all(|f| is_test_file(f));
            if !all_tests {
                continue;
            }
            // This is a test-only task — it should depend on at least one source task
            for &src_id in &source_task_ids {
                if src_id != task.id && !task.dependencies.iter().any(|d| d == src_id) {
                    return Err(format!(
                        "Test task '{}' produces only test files but does not depend on source task \
                            '{}'",
                        task.id, src_id
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Simple glob matching for test file patterns.
///
/// Supports `*` (any within component) and `**` (any path segment).
/// This is intentionally minimal — only used for plan validation heuristics.
/// Public wrapper: minimal glob matching (`*` within a component, `**`
/// across segments), shared with the agent's `glob` tool.
pub fn glob_matches(pattern: &str, path: &str) -> bool {
    glob_matches_simple(pattern, path)
}

pub(crate) fn glob_matches_simple(pattern: &str, path: &str) -> bool {
    let pat_parts: Vec<&str> = pattern.split('/').collect();
    let path_parts: Vec<&str> = path.split('/').collect();
    glob_match_parts(&pat_parts, &path_parts)
}

fn glob_match_parts(pat: &[&str], path: &[&str]) -> bool {
    if pat.is_empty() {
        return path.is_empty();
    }
    if pat[0] == "**" {
        // ** matches zero or more path segments
        for i in 0..=path.len() {
            if glob_match_parts(&pat[1..], &path[i..]) {
                return true;
            }
        }
        return false;
    }
    if path.is_empty() {
        return false;
    }
    if glob_match_component(pat[0], path[0]) {
        glob_match_parts(&pat[1..], &path[1..])
    } else {
        false
    }
}

fn glob_match_component(pattern: &str, component: &str) -> bool {
    // Simple wildcard matching within a single path component
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == component;
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut pos = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if let Some(found) = component[pos..].find(part) {
            if i == 0 && found != 0 {
                return false; // First part must match at start
            }
            pos += found + part.len();
        } else {
            return false;
        }
    }
    if let Some(last) = parts.last() {
        if !last.is_empty() {
            return component.ends_with(last);
        }
    }
    true
}

impl Default for TaskPlan {
    fn default() -> Self {
        Self::new()
    }
}

/// A planned task from the Architect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedTask {
    /// Unique task identifier (e.g., "task_1", "test_auth")
    pub id: String,
    /// Human-readable goal description
    pub goal: String,
    /// Files to read for context
    #[serde(default)]
    pub context_files: Vec<String>,
    /// Files to create or modify
    #[serde(default)]
    pub output_files: Vec<String>,
    /// Task IDs this depends on (must complete first)
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Type of task
    #[serde(default)]
    pub task_type: TaskType,
    /// Behavioral contract for this task
    #[serde(default)]
    pub contract: PlannedContract,
    /// Command contract (only for TaskType::Command)
    #[serde(default)]
    pub command_contract: Option<CommandContract>,
    /// PSP-5: Node class (Interface / Implementation / Integration)
    #[serde(default)]
    pub node_class: NodeClass,
    /// Declared dependency expectations (packages, setup, toolchain).
    #[serde(default)]
    pub dependency_expectations: DependencyExpectation,
}

impl PlannedTask {
    /// Create a simple task
    pub fn new(id: impl Into<String>, goal: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            goal: goal.into(),
            context_files: Vec::new(),
            output_files: Vec::new(),
            dependencies: Vec::new(),
            task_type: TaskType::Code,
            contract: PlannedContract::default(),
            command_contract: None,
            node_class: NodeClass::default(),
            dependency_expectations: DependencyExpectation::default(),
        }
    }
}

/// Contract specified in the plan
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlannedContract {
    /// Required public API signature
    #[serde(default)]
    pub interface_signature: Option<String>,
    /// Semantic constraints
    #[serde(default)]
    pub invariants: Vec<String>,
    /// Patterns to avoid
    #[serde(default)]
    pub forbidden_patterns: Vec<String>,
    /// Test cases with criticality
    #[serde(default)]
    pub tests: Vec<PlannedTest>,
}

/// Environment expectations a task declares (packages, setup, toolchain).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DependencyExpectation {
    /// Packages or crates the task expects to be available.
    pub required_packages: Vec<String>,
    /// Setup commands that must have succeeded before this task runs.
    pub setup_commands: Vec<String>,
    /// Minimum toolchain version string (e.g. `"1.75"` for Rust).
    pub min_toolchain_version: Option<String>,
}

/// A test case in the plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedTest {
    /// Test name or pattern
    pub name: String,
    /// Criticality level label (informational)
    #[serde(default = "default_criticality")]
    pub criticality: String,
}

fn default_criticality() -> String {
    "high".into()
}

/// Contract for command-type tasks (shell commands)
/// Defines expected outcomes for V_boot calculation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommandContract {
    /// The shell command to execute
    pub command: String,
    /// Expected exit code (default: 0)
    #[serde(default)]
    pub expected_exit_code: i32,
    /// Files that should exist after command completes
    #[serde(default)]
    pub expected_files: Vec<String>,
    /// Patterns that should NOT appear in stderr
    #[serde(default)]
    pub forbidden_stderr_patterns: Vec<String>,
    /// Working directory for the command (relative to project root)
    #[serde(default)]
    pub working_dir: Option<String>,
}

impl CommandContract {
    /// Create a new command contract
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            expected_exit_code: 0,
            expected_files: Vec::new(),
            forbidden_stderr_patterns: Vec::new(),
            working_dir: None,
        }
    }

    /// Calculate V_boot energy from command result
    pub fn calculate_energy(&self, exit_code: i32, stderr: &str, existing_files: &[String]) -> f32 {
        let mut energy = 0.0;

        // Exit code mismatch
        if exit_code != self.expected_exit_code {
            energy += 1.0;
        }

        // Missing expected files
        for expected in &self.expected_files {
            if !existing_files.contains(expected) {
                energy += 0.5;
            }
        }

        // Forbidden stderr patterns
        for pattern in &self.forbidden_stderr_patterns {
            if stderr.contains(pattern) {
                energy += 0.3;
            }
        }

        energy
    }
}
