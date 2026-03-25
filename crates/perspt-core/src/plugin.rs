//! Language Plugin Architecture
//!
//! Provides a trait-based plugin system for polyglot support.
//! Each language (Rust, Python, JS, etc.) implements this trait.
//!
//! PSP-000005 expands plugins from init-only to full runtime verification contracts.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// LSP Configuration for a language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspConfig {
    /// LSP server binary name
    pub server_binary: String,
    /// Arguments to pass to the server
    pub args: Vec<String>,
    /// Language ID for textDocument/didOpen
    pub language_id: String,
}

/// Options for project initialization
#[derive(Debug, Clone, Default)]
pub struct InitOptions {
    /// Project name
    pub name: String,
    /// Whether to use a specific package manager (e.g., "poetry", "pdm", "npm", "pnpm")
    pub package_manager: Option<String>,
    /// Additional flags
    pub flags: Vec<String>,
    /// Whether the target directory is empty
    pub is_empty_dir: bool,
}

/// Action to take for project initialization or tooling sync
#[derive(Debug, Clone)]
pub enum ProjectAction {
    /// Execute a shell command
    ExecCommand {
        /// The command to run
        command: String,
        /// Human-readable description of what this command does
        description: String,
    },
    /// No action needed
    NoAction,
}

/// A plugin for a specific programming language
///
/// PSP-5 expands this trait beyond init/test/run to a full capability-based
/// runtime contract that governs detection, verification, LSP, and ownership.
pub trait LanguagePlugin: Send + Sync {
    /// Name of the language
    fn name(&self) -> &str;

    /// File extensions this plugin handles
    fn extensions(&self) -> &[&str];

    /// Key files that identify this language (e.g., Cargo.toml, pyproject.toml)
    fn key_files(&self) -> &[&str];

    /// Detect if this plugin should handle the given project directory
    fn detect(&self, path: &Path) -> bool {
        // Check for key files
        for key_file in self.key_files() {
            if path.join(key_file).exists() {
                return true;
            }
        }

        // Check for files with handled extensions
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    let ext_str = ext.to_string_lossy();
                    if self.extensions().iter().any(|e| *e == ext_str) {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Get the LSP configuration for this language
    fn get_lsp_config(&self) -> LspConfig;

    /// Get the action to initialize a new project (greenfield)
    fn get_init_action(&self, opts: &InitOptions) -> ProjectAction;

    /// Check if an existing project needs tooling sync (e.g., uv sync, cargo fetch)
    fn check_tooling_action(&self, path: &Path) -> ProjectAction;

    /// Get the command to initialize a new project
    /// DEPRECATED: Use get_init_action instead
    fn init_command(&self, opts: &InitOptions) -> String;

    /// Get the command to run tests
    fn test_command(&self) -> String;

    /// Get the command to run the project (for verification)
    fn run_command(&self) -> String;

    // =========================================================================
    // PSP-5: Capability-Based Runtime Contract
    // =========================================================================

    /// Get the syntax/type check command (e.g., `cargo check`, `uv run ty check .`)
    ///
    /// Returns None if the plugin has no syntax check command (uses LSP only).
    fn syntax_check_command(&self) -> Option<String> {
        None
    }

    /// Get the build command (e.g., `cargo build`, `npm run build`)
    ///
    /// Returns None if the language doesn't have a separate build step.
    fn build_command(&self) -> Option<String> {
        None
    }

    /// Get the lint command (e.g., `cargo clippy -- -D warnings`)
    ///
    /// Used only in VerifierStrictness::Strict mode.
    fn lint_command(&self) -> Option<String> {
        None
    }

    /// File glob patterns this plugin owns (e.g., `["*.rs", "Cargo.toml"]`)
    ///
    /// Used for node ownership matching in multi-language repos.
    fn file_ownership_patterns(&self) -> &[&str] {
        self.extensions()
    }

    /// PSP-5 Phase 2: Check if a file path belongs to this plugin's ownership domain
    ///
    /// Uses `file_ownership_patterns()` for suffix/extension matching.
    fn owns_file(&self, path: &str) -> bool {
        let path_lower = path.to_lowercase();
        self.file_ownership_patterns().iter().any(|pattern| {
            let pattern = pattern.trim_start_matches('*');
            path_lower.ends_with(pattern)
        })
    }

    /// Check if the host has the required build tools available
    ///
    /// Returns true if the plugin's primary toolchain is installed and callable.
    /// When false, the runtime enters degraded-validation mode.
    fn host_tool_available(&self) -> bool {
        true
    }

    /// Get fallback LSP config when primary is unavailable
    fn lsp_fallback(&self) -> Option<LspConfig> {
        None
    }
}

/// Rust language plugin
pub struct RustPlugin;

impl LanguagePlugin for RustPlugin {
    fn name(&self) -> &str {
        "rust"
    }

    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn key_files(&self) -> &[&str] {
        &["Cargo.toml", "Cargo.lock"]
    }

    fn get_lsp_config(&self) -> LspConfig {
        LspConfig {
            server_binary: "rust-analyzer".to_string(),
            args: vec![],
            language_id: "rust".to_string(),
        }
    }

    fn get_init_action(&self, opts: &InitOptions) -> ProjectAction {
        let command = if opts.is_empty_dir || opts.name == "." || opts.name == "./" {
            "cargo init .".to_string()
        } else {
            format!("cargo new {}", opts.name)
        };
        ProjectAction::ExecCommand {
            command,
            description: "Initialize Rust project with Cargo".to_string(),
        }
    }

    fn check_tooling_action(&self, path: &Path) -> ProjectAction {
        // Check if Cargo.lock exists; if not, suggest cargo fetch
        if !path.join("Cargo.lock").exists() && path.join("Cargo.toml").exists() {
            ProjectAction::ExecCommand {
                command: "cargo fetch".to_string(),
                description: "Fetch Rust dependencies".to_string(),
            }
        } else {
            ProjectAction::NoAction
        }
    }

    fn init_command(&self, opts: &InitOptions) -> String {
        if opts.name == "." || opts.name == "./" {
            "cargo init .".to_string()
        } else {
            format!("cargo new {}", opts.name)
        }
    }

    fn test_command(&self) -> String {
        "cargo test".to_string()
    }

    fn run_command(&self) -> String {
        "cargo run".to_string()
    }

    // PSP-5 capability methods

    fn syntax_check_command(&self) -> Option<String> {
        Some("cargo check".to_string())
    }

    fn build_command(&self) -> Option<String> {
        Some("cargo build".to_string())
    }

    fn lint_command(&self) -> Option<String> {
        Some("cargo clippy -- -D warnings".to_string())
    }

    fn file_ownership_patterns(&self) -> &[&str] {
        &["rs"]
    }

    fn host_tool_available(&self) -> bool {
        std::process::Command::new("cargo")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

/// Python language plugin (uses ty via uvx)
pub struct PythonPlugin;

impl LanguagePlugin for PythonPlugin {
    fn name(&self) -> &str {
        "python"
    }

    fn extensions(&self) -> &[&str] {
        &["py"]
    }

    fn key_files(&self) -> &[&str] {
        &["pyproject.toml", "setup.py", "requirements.txt", "uv.lock"]
    }

    fn get_lsp_config(&self) -> LspConfig {
        // Prefer ty (via uvx) as the native Python support
        // Falls back to pyright if ty is not available
        LspConfig {
            server_binary: "uvx".to_string(),
            args: vec!["ty".to_string(), "server".to_string()],
            language_id: "python".to_string(),
        }
    }

    fn get_init_action(&self, opts: &InitOptions) -> ProjectAction {
        let command = match opts.package_manager.as_deref() {
            Some("poetry") => {
                if opts.is_empty_dir || opts.name == "." || opts.name == "./" {
                    "poetry init --no-interaction".to_string()
                } else {
                    format!("poetry new {}", opts.name)
                }
            }
            Some("pdm") => {
                if opts.is_empty_dir || opts.name == "." || opts.name == "./" {
                    "pdm init --non-interactive".to_string()
                } else {
                    format!(
                        "mkdir -p {} && cd {} && pdm init --non-interactive",
                        opts.name, opts.name
                    )
                }
            }
            _ => {
                // Default to uv
                if opts.is_empty_dir || opts.name == "." || opts.name == "./" {
                    "uv init".to_string()
                } else {
                    format!("uv init {}", opts.name)
                }
            }
        };
        let description = match opts.package_manager.as_deref() {
            Some("poetry") => "Initialize Python project with Poetry",
            Some("pdm") => "Initialize Python project with PDM",
            _ => "Initialize Python project with uv",
        };
        ProjectAction::ExecCommand {
            command,
            description: description.to_string(),
        }
    }

    fn check_tooling_action(&self, path: &Path) -> ProjectAction {
        // Check for pyproject.toml but missing .venv or uv.lock
        let has_pyproject = path.join("pyproject.toml").exists();
        let has_venv = path.join(".venv").exists();
        let has_uv_lock = path.join("uv.lock").exists();

        if has_pyproject && (!has_venv || !has_uv_lock) {
            ProjectAction::ExecCommand {
                command: "uv sync".to_string(),
                description: "Sync Python dependencies with uv".to_string(),
            }
        } else {
            ProjectAction::NoAction
        }
    }

    fn init_command(&self, opts: &InitOptions) -> String {
        if opts.package_manager.as_deref() == Some("poetry") {
            if opts.name == "." || opts.name == "./" {
                "poetry init".to_string()
            } else {
                format!("poetry new {}", opts.name)
            }
        } else {
            // uv init supports "." for current directory
            format!("uv init {}", opts.name)
        }
    }

    fn test_command(&self) -> String {
        "uv run pytest".to_string()
    }

    fn run_command(&self) -> String {
        "uv run python -m main".to_string()
    }

    // PSP-5 capability methods

    fn syntax_check_command(&self) -> Option<String> {
        Some("uv run ty check .".to_string())
    }

    fn lint_command(&self) -> Option<String> {
        Some("uv run ruff check .".to_string())
    }

    fn file_ownership_patterns(&self) -> &[&str] {
        &["py"]
    }

    fn host_tool_available(&self) -> bool {
        std::process::Command::new("uv")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn lsp_fallback(&self) -> Option<LspConfig> {
        Some(LspConfig {
            server_binary: "pyright-langserver".to_string(),
            args: vec!["--stdio".to_string()],
            language_id: "python".to_string(),
        })
    }
}

/// JavaScript/TypeScript language plugin
pub struct JsPlugin;

impl LanguagePlugin for JsPlugin {
    fn name(&self) -> &str {
        "javascript"
    }

    fn extensions(&self) -> &[&str] {
        &["js", "ts", "jsx", "tsx"]
    }

    fn key_files(&self) -> &[&str] {
        &["package.json", "tsconfig.json"]
    }

    fn get_lsp_config(&self) -> LspConfig {
        LspConfig {
            server_binary: "typescript-language-server".to_string(),
            args: vec!["--stdio".to_string()],
            language_id: "typescript".to_string(),
        }
    }

    fn get_init_action(&self, opts: &InitOptions) -> ProjectAction {
        let command = match opts.package_manager.as_deref() {
            Some("pnpm") => {
                if opts.is_empty_dir || opts.name == "." || opts.name == "./" {
                    "pnpm init".to_string()
                } else {
                    format!("mkdir -p {} && cd {} && pnpm init", opts.name, opts.name)
                }
            }
            Some("yarn") => {
                if opts.is_empty_dir || opts.name == "." || opts.name == "./" {
                    "yarn init -y".to_string()
                } else {
                    format!("mkdir -p {} && cd {} && yarn init -y", opts.name, opts.name)
                }
            }
            _ => {
                // Default to npm
                if opts.is_empty_dir || opts.name == "." || opts.name == "./" {
                    "npm init -y".to_string()
                } else {
                    format!("mkdir -p {} && cd {} && npm init -y", opts.name, opts.name)
                }
            }
        };
        let description = match opts.package_manager.as_deref() {
            Some("pnpm") => "Initialize JavaScript project with pnpm",
            Some("yarn") => "Initialize JavaScript project with Yarn",
            _ => "Initialize JavaScript project with npm",
        };
        ProjectAction::ExecCommand {
            command,
            description: description.to_string(),
        }
    }

    fn check_tooling_action(&self, path: &Path) -> ProjectAction {
        // Check for package.json but missing node_modules
        let has_package_json = path.join("package.json").exists();
        let has_node_modules = path.join("node_modules").exists();

        if has_package_json && !has_node_modules {
            ProjectAction::ExecCommand {
                command: "npm install".to_string(),
                description: "Install Node.js dependencies".to_string(),
            }
        } else {
            ProjectAction::NoAction
        }
    }

    fn init_command(&self, opts: &InitOptions) -> String {
        format!("npm init -y && mv package.json {}/", opts.name)
    }

    fn test_command(&self) -> String {
        "npm test".to_string()
    }

    fn run_command(&self) -> String {
        "npm start".to_string()
    }

    // PSP-5 capability methods

    fn syntax_check_command(&self) -> Option<String> {
        Some("npx tsc --noEmit".to_string())
    }

    fn build_command(&self) -> Option<String> {
        Some("npm run build".to_string())
    }

    fn lint_command(&self) -> Option<String> {
        Some("npx eslint .".to_string())
    }

    fn file_ownership_patterns(&self) -> &[&str] {
        &["js", "ts", "jsx", "tsx"]
    }

    fn host_tool_available(&self) -> bool {
        std::process::Command::new("node")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

/// Plugin registry for dynamic language detection
pub struct PluginRegistry {
    plugins: Vec<Box<dyn LanguagePlugin>>,
}

impl PluginRegistry {
    /// Create a new registry with all built-in plugins
    pub fn new() -> Self {
        Self {
            plugins: vec![
                Box::new(RustPlugin),
                Box::new(PythonPlugin),
                Box::new(JsPlugin),
            ],
        }
    }

    /// Detect which plugin should handle the given path (first match)
    pub fn detect(&self, path: &Path) -> Option<&dyn LanguagePlugin> {
        self.plugins
            .iter()
            .find(|p| p.detect(path))
            .map(|p| p.as_ref())
    }

    /// PSP-5: Detect ALL plugins that match the given path (polyglot support)
    ///
    /// Returns all matching plugins instead of just the first, enabling
    /// multi-language verification in polyglot repositories.
    pub fn detect_all(&self, path: &Path) -> Vec<&dyn LanguagePlugin> {
        self.plugins
            .iter()
            .filter(|p| p.detect(path))
            .map(|p| p.as_ref())
            .collect()
    }

    /// Get a plugin by name
    pub fn get(&self, name: &str) -> Option<&dyn LanguagePlugin> {
        self.plugins
            .iter()
            .find(|p| p.name() == name)
            .map(|p| p.as_ref())
    }

    /// Get all registered plugins
    pub fn all(&self) -> &[Box<dyn LanguagePlugin>] {
        &self.plugins
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_owns_file() {
        let rust = RustPlugin;
        assert!(rust.owns_file("src/main.rs"));
        assert!(rust.owns_file("crates/core/src/lib.rs"));
        assert!(!rust.owns_file("main.py"));
        assert!(!rust.owns_file("index.js"));

        let python = PythonPlugin;
        assert!(python.owns_file("main.py"));
        assert!(python.owns_file("tests/test_main.py"));
        assert!(!python.owns_file("src/main.rs"));

        let js = JsPlugin;
        assert!(js.owns_file("index.js"));
        assert!(js.owns_file("src/app.ts"));
        assert!(!js.owns_file("main.py"));
        assert!(!js.owns_file("src/main.rs"));
    }
}
