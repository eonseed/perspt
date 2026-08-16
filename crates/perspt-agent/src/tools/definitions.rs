/// Get tool definitions for LLM function calling
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    let mut tools = core_tools();
    tools.extend(power_tools());
    tools
}

/// Workspace read/write and command tools.
fn core_tools() -> Vec<ToolDefinition> {
    vec![
        def(
            "read_file",
            "Read the contents of a file",
            &[("path", "Path to the file to read", true)],
        ),
        def(
            "search_code",
            "Search for code patterns in the workspace using grep/ripgrep",
            &[
                ("query", "Search pattern (regex supported)", true),
                (
                    "path",
                    "Directory to search in (default: working directory)",
                    false,
                ),
            ],
        ),
        def(
            "apply_patch",
            "Write or replace file contents",
            &[
                ("path", "Path to the file to write", true),
                ("content", "New file contents", true),
            ],
        ),
        def(
            "apply_diff",
            "Apply a Unified Diff patch to a file",
            &[
                ("path", "Path to the file to patch", true),
                ("diff", "Unified Diff content", true),
            ],
        ),
        def(
            "run_command",
            "Execute a shell command in the working directory",
            &[("command", "Shell command to execute", true)],
        ),
        def(
            "list_files",
            "List files in a directory",
            &[("path", "Directory path (default: working directory)", false)],
        ),
    ]
}

/// OS-level power tools.
fn power_tools() -> Vec<ToolDefinition> {
    vec![
        def(
            "sed_replace",
            "Replace text in a file using sed-like pattern matching",
            &[
                ("path", "Path to the file", true),
                ("pattern", "Search pattern", true),
                ("replacement", "Replacement text", true),
            ],
        ),
        def(
            "awk_filter",
            "Filter file content using awk-like field selection",
            &[
                ("path", "Path to the file", true),
                (
                    "filter",
                    "Awk filter expression (e.g., '$1 == \"error\"')",
                    true,
                ),
            ],
        ),
        def(
            "diff_files",
            "Show differences between two files",
            &[
                ("file1", "First file path", true),
                ("file2", "Second file path", true),
            ],
        ),
    ]
}

/// Build one tool definition from `(name, description, required)` parameters.
fn def(name: &str, description: &str, params: &[(&str, &str, bool)]) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: description.to_string(),
        parameters: params
            .iter()
            .map(|(name, description, required)| ToolParameter {
                name: name.to_string(),
                description: description.to_string(),
                required: *required,
            })
            .collect(),
    }
}

/// Tool definition for LLM function calling
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ToolParameter>,
}

/// Tool parameter definition
#[derive(Debug, Clone)]
pub struct ToolParameter {
    pub name: String,
    pub description: String,
    pub required: bool,
}
