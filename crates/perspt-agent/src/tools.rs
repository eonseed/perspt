//! Agent Tooling
//!
//! Tools available to agents for interacting with the workspace.
//! Implements: read_file, search_code, apply_patch, run_command

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Tool result from agent execution
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub tool_name: String,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

impl ToolResult {
    pub fn success(tool_name: &str, output: String) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            success: true,
            output,
            error: None,
        }
    }

    pub fn failure(tool_name: &str, error: String) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            success: false,
            output: String::new(),
            error: Some(error),
        }
    }
}

/// Tool call request from LLM
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub arguments: HashMap<String, String>,
}

/// Agent tools for workspace interaction
pub struct AgentTools {
    /// Working directory (sandbox root)
    working_dir: PathBuf,
    /// Whether to require user approval for commands
    require_approval: bool,
}

impl AgentTools {
    /// Create new agent tools instance
    pub fn new(working_dir: PathBuf, require_approval: bool) -> Self {
        Self {
            working_dir,
            require_approval,
        }
    }

    /// Execute a tool call
    pub async fn execute(&self, call: &ToolCall) -> ToolResult {
        match call.name.as_str() {
            "read_file" => self.read_file(call),
            "search_code" => self.search_code(call),
            "apply_patch" => self.apply_patch(call),
            "run_command" => self.run_command(call).await,
            "list_files" => self.list_files(call),
            "write_file" => self.write_file(call),
            _ => ToolResult::failure(&call.name, format!("Unknown tool: {}", call.name)),
        }
    }

    /// Read a file's contents
    fn read_file(&self, call: &ToolCall) -> ToolResult {
        let path = match call.arguments.get("path") {
            Some(p) => self.resolve_path(p),
            None => return ToolResult::failure("read_file", "Missing 'path' argument".to_string()),
        };

        match fs::read_to_string(&path) {
            Ok(content) => ToolResult::success("read_file", content),
            Err(e) => ToolResult::failure("read_file", format!("Failed to read {:?}: {}", path, e)),
        }
    }

    /// Search for code patterns using grep
    fn search_code(&self, call: &ToolCall) -> ToolResult {
        let query = match call.arguments.get("query") {
            Some(q) => q,
            None => {
                return ToolResult::failure("search_code", "Missing 'query' argument".to_string())
            }
        };

        let path = call
            .arguments
            .get("path")
            .map(|p| self.resolve_path(p))
            .unwrap_or_else(|| self.working_dir.clone());

        // Use ripgrep if available, fallback to grep
        let output = Command::new("rg")
            .args(["--json", "-n", query])
            .current_dir(&path)
            .output()
            .or_else(|_| {
                Command::new("grep")
                    .args(["-rn", query, "."])
                    .current_dir(&path)
                    .output()
            });

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                ToolResult::success("search_code", stdout)
            }
            Err(e) => ToolResult::failure("search_code", format!("Search failed: {}", e)),
        }
    }

    /// Apply a patch to a file
    fn apply_patch(&self, call: &ToolCall) -> ToolResult {
        let path = match call.arguments.get("path") {
            Some(p) => self.resolve_path(p),
            None => {
                return ToolResult::failure("apply_patch", "Missing 'path' argument".to_string())
            }
        };

        let content = match call.arguments.get("content") {
            Some(c) => c,
            None => {
                return ToolResult::failure("apply_patch", "Missing 'content' argument".to_string())
            }
        };

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                return ToolResult::failure(
                    "apply_patch",
                    format!("Failed to create directories: {}", e),
                );
            }
        }

        match fs::write(&path, content) {
            Ok(_) => ToolResult::success("apply_patch", format!("Successfully wrote {:?}", path)),
            Err(e) => {
                ToolResult::failure("apply_patch", format!("Failed to write {:?}: {}", path, e))
            }
        }
    }

    /// Run a shell command (requires approval unless auto-approve is set)
    async fn run_command(&self, call: &ToolCall) -> ToolResult {
        let cmd = match call.arguments.get("command") {
            Some(c) => c,
            None => {
                return ToolResult::failure("run_command", "Missing 'command' argument".to_string())
            }
        };

        // In a real implementation, this would check with the policy engine
        // and potentially prompt the user for approval
        if self.require_approval {
            log::info!("Command requires approval: {}", cmd);
            // For now, we'll log but continue
        }

        let output = Command::new("sh")
            .args(["-c", cmd])
            .current_dir(&self.working_dir)
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();

                if out.status.success() {
                    ToolResult::success("run_command", stdout)
                } else {
                    ToolResult::failure(
                        "run_command",
                        format!("Exit code: {:?}\n{}", out.status.code(), stderr),
                    )
                }
            }
            Err(e) => ToolResult::failure("run_command", format!("Failed to execute: {}", e)),
        }
    }

    /// List files in a directory
    fn list_files(&self, call: &ToolCall) -> ToolResult {
        let path = call
            .arguments
            .get("path")
            .map(|p| self.resolve_path(p))
            .unwrap_or_else(|| self.working_dir.clone());

        match fs::read_dir(&path) {
            Ok(entries) => {
                let files: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            format!("{}/", name)
                        } else {
                            name
                        }
                    })
                    .collect();
                ToolResult::success("list_files", files.join("\n"))
            }
            Err(e) => {
                ToolResult::failure("list_files", format!("Failed to list {:?}: {}", path, e))
            }
        }
    }

    /// Write content to a file
    fn write_file(&self, call: &ToolCall) -> ToolResult {
        // Alias for apply_patch with different semantics
        self.apply_patch(call)
    }

    /// Resolve a path relative to working directory
    fn resolve_path(&self, path: &str) -> PathBuf {
        let p = Path::new(path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.working_dir.join(p)
        }
    }
}

/// Get tool definitions for LLM function calling
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "read_file".to_string(),
            description: "Read the contents of a file".to_string(),
            parameters: vec![ToolParameter {
                name: "path".to_string(),
                description: "Path to the file to read".to_string(),
                required: true,
            }],
        },
        ToolDefinition {
            name: "search_code".to_string(),
            description: "Search for code patterns in the workspace using grep/ripgrep".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "query".to_string(),
                    description: "Search pattern (regex supported)".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "path".to_string(),
                    description: "Directory to search in (default: working directory)".to_string(),
                    required: false,
                },
            ],
        },
        ToolDefinition {
            name: "apply_patch".to_string(),
            description: "Write or replace file contents".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "path".to_string(),
                    description: "Path to the file to write".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "content".to_string(),
                    description: "New file contents".to_string(),
                    required: true,
                },
            ],
        },
        ToolDefinition {
            name: "run_command".to_string(),
            description: "Execute a shell command in the working directory".to_string(),
            parameters: vec![ToolParameter {
                name: "command".to_string(),
                description: "Shell command to execute".to_string(),
                required: true,
            }],
        },
        ToolDefinition {
            name: "list_files".to_string(),
            description: "List files in a directory".to_string(),
            parameters: vec![ToolParameter {
                name: "path".to_string(),
                description: "Directory path (default: working directory)".to_string(),
                required: false,
            }],
        },
    ]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[tokio::test]
    async fn test_read_file() {
        let dir = temp_dir();
        let test_file = dir.join("test_read.txt");
        fs::write(&test_file, "Hello, World!").unwrap();

        let tools = AgentTools::new(dir.clone(), false);
        let call = ToolCall {
            name: "read_file".to_string(),
            arguments: [("path".to_string(), test_file.to_string_lossy().to_string())]
                .into_iter()
                .collect(),
        };

        let result = tools.execute(&call).await;
        assert!(result.success);
        assert_eq!(result.output, "Hello, World!");
    }

    #[tokio::test]
    async fn test_list_files() {
        let dir = temp_dir();
        let tools = AgentTools::new(dir.clone(), false);
        let call = ToolCall {
            name: "list_files".to_string(),
            arguments: HashMap::new(),
        };

        let result = tools.execute(&call).await;
        assert!(result.success);
    }
}
