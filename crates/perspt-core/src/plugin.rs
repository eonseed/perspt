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

// =============================================================================
// PSP-5 Phase 4: Verifier Capability Declarations
// =============================================================================

/// Verification stage in the plugin-driven pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VerifierStage {
    /// Syntax / type check (e.g. `cargo check`, `uv run ty check .`)
    SyntaxCheck,
    /// Build step (e.g. `cargo build`, `npm run build`)
    Build,
    /// Test execution (e.g. `cargo test`, `uv run pytest`)
    Test,
    /// Lint pass (e.g. `cargo clippy`, `uv run ruff check .`)
    Lint,
}

impl std::fmt::Display for VerifierStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifierStage::SyntaxCheck => write!(f, "syntax_check"),
            VerifierStage::Build => write!(f, "build"),
            VerifierStage::Test => write!(f, "test"),
            VerifierStage::Lint => write!(f, "lint"),
        }
    }
}

/// A single verifier sensor: one stage of the verification pipeline.
///
/// Each capability independently declares its command, host-tool availability,
/// and optional fallback. This replaces the coarse single `host_tool_available()`
/// check with per-sensor probing.
#[derive(Debug, Clone)]
pub struct VerifierCapability {
    /// Which stage this capability covers.
    pub stage: VerifierStage,
    /// Primary command to execute (None if this stage is not supported).
    pub command: Option<String>,
    /// Whether the primary command's host tool is available on this machine.
    pub available: bool,
    /// Fallback command when the primary tool is unavailable.
    pub fallback_command: Option<String>,
    /// Whether the fallback tool is available.
    pub fallback_available: bool,
}

impl VerifierCapability {
    /// True if either the primary or fallback tool is available.
    pub fn any_available(&self) -> bool {
        self.available || self.fallback_available
    }

    /// The best available command, preferring primary over fallback.
    pub fn effective_command(&self) -> Option<&str> {
        if self.available {
            self.command.as_deref()
        } else if self.fallback_available {
            self.fallback_command.as_deref()
        } else {
            None
        }
    }
}

/// LSP availability and fallback for a plugin.
#[derive(Debug, Clone)]
pub struct LspCapability {
    /// Primary LSP configuration.
    pub primary: LspConfig,
    /// Whether the primary LSP binary is available on the host.
    pub primary_available: bool,
    /// Fallback LSP configuration (if any).
    pub fallback: Option<LspConfig>,
    /// Whether the fallback binary is available.
    pub fallback_available: bool,
}

impl LspCapability {
    /// Return the best available LSP config, preferring primary.
    pub fn effective_config(&self) -> Option<&LspConfig> {
        if self.primary_available {
            Some(&self.primary)
        } else if self.fallback_available {
            self.fallback.as_ref()
        } else {
            None
        }
    }
}

/// Complete verifier profile for a plugin.
///
/// Bundles all per-sensor capabilities and LSP availability into one
/// inspectable structure. Built by `LanguagePlugin::verifier_profile()`.
#[derive(Debug, Clone)]
pub struct VerifierProfile {
    /// Name of the plugin that produced this profile.
    pub plugin_name: String,
    /// Per-stage verifier capabilities.
    pub capabilities: Vec<VerifierCapability>,
    /// LSP availability and fallback.
    pub lsp: LspCapability,
}

impl VerifierProfile {
    /// Get the capability for a given stage, if declared.
    pub fn get(&self, stage: VerifierStage) -> Option<&VerifierCapability> {
        self.capabilities.iter().find(|c| c.stage == stage)
    }

    /// Stages that have at least one available tool (primary or fallback).
    pub fn available_stages(&self) -> Vec<VerifierStage> {
        self.capabilities
            .iter()
            .filter(|c| c.any_available())
            .map(|c| c.stage)
            .collect()
    }

    /// True when every declared stage has zero available tools.
    pub fn fully_degraded(&self) -> bool {
        self.capabilities.iter().all(|c| !c.any_available())
    }
}

// =============================================================================
// Utility: host binary probe
// =============================================================================

/// Check whether a given binary name is available on the host PATH.
///
/// Runs `<binary> --version` silently; returns `true` if the process exits
/// successfully. Used by plugins for per-sensor host-tool probing.
pub fn host_binary_available(binary: &str) -> bool {
    std::process::Command::new(binary)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
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

    /// Required host binaries for this plugin, grouped by role.
    ///
    /// Each entry is `(binary_name, role_description, install_hint)`.
    /// The orchestrator checks these before init and emits install directions
    /// for any that are missing.
    fn required_binaries(&self) -> Vec<(&str, &str, &str)> {
        Vec::new()
    }

    /// Get fallback LSP config when primary is unavailable
    fn lsp_fallback(&self) -> Option<LspConfig> {
        None
    }

    // =========================================================================
    // PSP-5 Phase 4: Verifier Profile Assembly
    // =========================================================================

    /// Build a complete verifier profile by probing each capability.
    ///
    /// The default implementation auto-assembles from the existing
    /// `syntax_check_command()`, `build_command()`, `test_command()`,
    /// `lint_command()`, and `host_tool_available()` methods.
    ///
    /// Plugins override this method to provide per-sensor probing
    /// with distinct fallback commands and independent availability checks.
    fn verifier_profile(&self) -> VerifierProfile {
        let tool_available = self.host_tool_available();

        let mut capabilities = Vec::new();

        if let Some(cmd) = self.syntax_check_command() {
            capabilities.push(VerifierCapability {
                stage: VerifierStage::SyntaxCheck,
                command: Some(cmd),
                available: tool_available,
                fallback_command: None,
                fallback_available: false,
            });
        }

        if let Some(cmd) = self.build_command() {
            capabilities.push(VerifierCapability {
                stage: VerifierStage::Build,
                command: Some(cmd),
                available: tool_available,
                fallback_command: None,
                fallback_available: false,
            });
        }

        // Test always has a command (test_command is required)
        capabilities.push(VerifierCapability {
            stage: VerifierStage::Test,
            command: Some(self.test_command()),
            available: tool_available,
            fallback_command: None,
            fallback_available: false,
        });

        if let Some(cmd) = self.lint_command() {
            capabilities.push(VerifierCapability {
                stage: VerifierStage::Lint,
                command: Some(cmd),
                available: tool_available,
                fallback_command: None,
                fallback_available: false,
            });
        }

        let primary_config = self.get_lsp_config();
        let primary_available = host_binary_available(&primary_config.server_binary);
        let fallback = self.lsp_fallback();
        let fallback_available = fallback
            .as_ref()
            .map(|f| host_binary_available(&f.server_binary))
            .unwrap_or(false);

        VerifierProfile {
            plugin_name: self.name().to_string(),
            capabilities,
            lsp: LspCapability {
                primary: primary_config,
                primary_available,
                fallback,
                fallback_available,
            },
        }
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

    fn required_binaries(&self) -> Vec<(&str, &str, &str)> {
        vec![
            ("cargo", "build/init", "Install Rust via https://rustup.rs"),
            ("rustc", "compiler", "Install Rust via https://rustup.rs"),
            ("rust-analyzer", "language server", "rustup component add rust-analyzer"),
        ]
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
        host_binary_available("cargo")
    }

    fn verifier_profile(&self) -> VerifierProfile {
        let cargo = host_binary_available("cargo");
        let clippy = cargo; // clippy is a cargo subcommand, same binary

        let capabilities = vec![
            VerifierCapability {
                stage: VerifierStage::SyntaxCheck,
                command: Some("cargo check".to_string()),
                available: cargo,
                fallback_command: None,
                fallback_available: false,
            },
            VerifierCapability {
                stage: VerifierStage::Build,
                command: Some("cargo build".to_string()),
                available: cargo,
                fallback_command: None,
                fallback_available: false,
            },
            VerifierCapability {
                stage: VerifierStage::Test,
                command: Some("cargo test".to_string()),
                available: cargo,
                fallback_command: None,
                fallback_available: false,
            },
            VerifierCapability {
                stage: VerifierStage::Lint,
                command: Some("cargo clippy -- -D warnings".to_string()),
                available: clippy,
                fallback_command: None,
                fallback_available: false,
            },
        ];

        let primary = self.get_lsp_config();
        let primary_available = host_binary_available(&primary.server_binary);

        VerifierProfile {
            plugin_name: self.name().to_string(),
            capabilities,
            lsp: LspCapability {
                primary,
                primary_available,
                fallback: None,
                fallback_available: false,
            },
        }
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

    fn required_binaries(&self) -> Vec<(&str, &str, &str)> {
        vec![
            ("uv", "package manager", "curl -LsSf https://astral.sh/uv/install.sh | sh"),
            ("python3", "interpreter", "uv python install (or install from https://python.org)"),
            ("uvx", "tool runner/LSP", "Installed with uv — curl -LsSf https://astral.sh/uv/install.sh | sh"),
        ]
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
        host_binary_available("uv")
    }

    fn lsp_fallback(&self) -> Option<LspConfig> {
        Some(LspConfig {
            server_binary: "pyright-langserver".to_string(),
            args: vec!["--stdio".to_string()],
            language_id: "python".to_string(),
        })
    }

    fn verifier_profile(&self) -> VerifierProfile {
        let uv = host_binary_available("uv");
        let pyright = host_binary_available("pyright");

        let capabilities = vec![
            VerifierCapability {
                stage: VerifierStage::SyntaxCheck,
                command: Some("uv run ty check .".to_string()),
                available: uv,
                // pyright as CLI fallback for syntax checking
                fallback_command: Some("pyright .".to_string()),
                fallback_available: pyright,
            },
            VerifierCapability {
                stage: VerifierStage::Test,
                command: Some("uv run pytest".to_string()),
                available: uv,
                // bare pytest fallback
                fallback_command: Some("python -m pytest".to_string()),
                fallback_available: host_binary_available("python3")
                    || host_binary_available("python"),
            },
            VerifierCapability {
                stage: VerifierStage::Lint,
                command: Some("uv run ruff check .".to_string()),
                available: uv,
                fallback_command: Some("ruff check .".to_string()),
                fallback_available: host_binary_available("ruff"),
            },
        ];

        let primary = self.get_lsp_config();
        let primary_available = host_binary_available("uvx");
        let fallback = self.lsp_fallback();
        let fallback_available = fallback
            .as_ref()
            .map(|f| host_binary_available(&f.server_binary))
            .unwrap_or(false);

        VerifierProfile {
            plugin_name: self.name().to_string(),
            capabilities,
            lsp: LspCapability {
                primary,
                primary_available,
                fallback,
                fallback_available,
            },
        }
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

    fn required_binaries(&self) -> Vec<(&str, &str, &str)> {
        vec![
            ("node", "runtime", "Install Node.js from https://nodejs.org or via nvm"),
            ("npm", "package manager", "Included with Node.js — install from https://nodejs.org"),
            ("typescript-language-server", "language server", "npm install -g typescript-language-server typescript"),
        ]
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
        host_binary_available("node")
    }

    fn verifier_profile(&self) -> VerifierProfile {
        let node = host_binary_available("node");
        let npx = host_binary_available("npx");

        let capabilities = vec![
            VerifierCapability {
                stage: VerifierStage::SyntaxCheck,
                command: Some("npx tsc --noEmit".to_string()),
                available: npx,
                fallback_command: None,
                fallback_available: false,
            },
            VerifierCapability {
                stage: VerifierStage::Build,
                command: Some("npm run build".to_string()),
                available: node,
                fallback_command: None,
                fallback_available: false,
            },
            VerifierCapability {
                stage: VerifierStage::Test,
                command: Some("npm test".to_string()),
                available: node,
                fallback_command: None,
                fallback_available: false,
            },
            VerifierCapability {
                stage: VerifierStage::Lint,
                command: Some("npx eslint .".to_string()),
                available: npx,
                fallback_command: None,
                fallback_available: false,
            },
        ];

        let primary = self.get_lsp_config();
        let primary_available = host_binary_available(&primary.server_binary);

        VerifierProfile {
            plugin_name: self.name().to_string(),
            capabilities,
            lsp: LspCapability {
                primary,
                primary_available,
                fallback: None,
                fallback_available: false,
            },
        }
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

    // =========================================================================
    // Verifier Capability & Profile Tests
    // =========================================================================

    #[test]
    fn test_verifier_capability_effective_command() {
        // Primary available → primary wins
        let cap = VerifierCapability {
            stage: VerifierStage::SyntaxCheck,
            command: Some("cargo check".to_string()),
            available: true,
            fallback_command: Some("rustc --edition 2021".to_string()),
            fallback_available: true,
        };
        assert_eq!(cap.effective_command(), Some("cargo check"));
        assert!(cap.any_available());

        // Primary unavailable, fallback available → fallback wins
        let cap2 = VerifierCapability {
            stage: VerifierStage::Lint,
            command: Some("uv run ruff check .".to_string()),
            available: false,
            fallback_command: Some("ruff check .".to_string()),
            fallback_available: true,
        };
        assert_eq!(cap2.effective_command(), Some("ruff check ."));
        assert!(cap2.any_available());

        // Both unavailable → None
        let cap3 = VerifierCapability {
            stage: VerifierStage::Build,
            command: Some("cargo build".to_string()),
            available: false,
            fallback_command: None,
            fallback_available: false,
        };
        assert_eq!(cap3.effective_command(), None);
        assert!(!cap3.any_available());
    }

    #[test]
    fn test_verifier_profile_get_and_available_stages() {
        let profile = VerifierProfile {
            plugin_name: "test".to_string(),
            capabilities: vec![
                VerifierCapability {
                    stage: VerifierStage::SyntaxCheck,
                    command: Some("check".to_string()),
                    available: true,
                    fallback_command: None,
                    fallback_available: false,
                },
                VerifierCapability {
                    stage: VerifierStage::Build,
                    command: Some("build".to_string()),
                    available: false,
                    fallback_command: None,
                    fallback_available: false,
                },
                VerifierCapability {
                    stage: VerifierStage::Test,
                    command: Some("test".to_string()),
                    available: true,
                    fallback_command: None,
                    fallback_available: false,
                },
            ],
            lsp: LspCapability {
                primary: LspConfig {
                    server_binary: "test-ls".to_string(),
                    args: vec![],
                    language_id: "test".to_string(),
                },
                primary_available: false,
                fallback: None,
                fallback_available: false,
            },
        };

        assert!(profile.get(VerifierStage::SyntaxCheck).is_some());
        assert!(profile.get(VerifierStage::Lint).is_none());

        let available = profile.available_stages();
        assert_eq!(available.len(), 2);
        assert!(available.contains(&VerifierStage::SyntaxCheck));
        assert!(available.contains(&VerifierStage::Test));
        assert!(!available.contains(&VerifierStage::Build));
        assert!(!profile.fully_degraded());
    }

    #[test]
    fn test_verifier_profile_fully_degraded() {
        let profile = VerifierProfile {
            plugin_name: "empty".to_string(),
            capabilities: vec![VerifierCapability {
                stage: VerifierStage::Build,
                command: Some("build".to_string()),
                available: false,
                fallback_command: None,
                fallback_available: false,
            }],
            lsp: LspCapability {
                primary: LspConfig {
                    server_binary: "none".to_string(),
                    args: vec![],
                    language_id: "none".to_string(),
                },
                primary_available: false,
                fallback: None,
                fallback_available: false,
            },
        };
        assert!(profile.fully_degraded());
        assert!(profile.available_stages().is_empty());
    }

    #[test]
    fn test_lsp_capability_effective_config() {
        let lsp = LspCapability {
            primary: LspConfig {
                server_binary: "rust-analyzer".to_string(),
                args: vec![],
                language_id: "rust".to_string(),
            },
            primary_available: true,
            fallback: None,
            fallback_available: false,
        };
        assert_eq!(
            lsp.effective_config().unwrap().server_binary,
            "rust-analyzer"
        );

        // Primary unavailable, fallback available
        let lsp2 = LspCapability {
            primary: LspConfig {
                server_binary: "uvx".to_string(),
                args: vec![],
                language_id: "python".to_string(),
            },
            primary_available: false,
            fallback: Some(LspConfig {
                server_binary: "pyright-langserver".to_string(),
                args: vec!["--stdio".to_string()],
                language_id: "python".to_string(),
            }),
            fallback_available: true,
        };
        assert_eq!(
            lsp2.effective_config().unwrap().server_binary,
            "pyright-langserver"
        );

        // Both unavailable
        let lsp3 = LspCapability {
            primary: LspConfig {
                server_binary: "nope".to_string(),
                args: vec![],
                language_id: "none".to_string(),
            },
            primary_available: false,
            fallback: None,
            fallback_available: false,
        };
        assert!(lsp3.effective_config().is_none());
    }

    #[test]
    fn test_rust_plugin_verifier_profile_shape() {
        let rust = RustPlugin;
        let profile = rust.verifier_profile();
        assert_eq!(profile.plugin_name, "rust");
        // Rust should declare all 4 stages
        assert_eq!(profile.capabilities.len(), 4);
        let stages: Vec<_> = profile.capabilities.iter().map(|c| c.stage).collect();
        assert!(stages.contains(&VerifierStage::SyntaxCheck));
        assert!(stages.contains(&VerifierStage::Build));
        assert!(stages.contains(&VerifierStage::Test));
        assert!(stages.contains(&VerifierStage::Lint));
    }

    #[test]
    fn test_python_plugin_verifier_profile_shape() {
        let py = PythonPlugin;
        let profile = py.verifier_profile();
        assert_eq!(profile.plugin_name, "python");
        // Python: syntax_check, test, lint (no build)
        assert_eq!(profile.capabilities.len(), 3);
        let stages: Vec<_> = profile.capabilities.iter().map(|c| c.stage).collect();
        assert!(stages.contains(&VerifierStage::SyntaxCheck));
        assert!(stages.contains(&VerifierStage::Test));
        assert!(stages.contains(&VerifierStage::Lint));
        assert!(!stages.contains(&VerifierStage::Build));
        // Python has an LSP fallback declared
        assert!(profile.lsp.fallback.is_some());
    }

    #[test]
    fn test_js_plugin_verifier_profile_shape() {
        let js = JsPlugin;
        let profile = js.verifier_profile();
        assert_eq!(profile.plugin_name, "javascript");
        // JS: all 4 stages
        assert_eq!(profile.capabilities.len(), 4);
    }

    #[test]
    fn test_verifier_stage_display() {
        assert_eq!(format!("{}", VerifierStage::SyntaxCheck), "syntax_check");
        assert_eq!(format!("{}", VerifierStage::Build), "build");
        assert_eq!(format!("{}", VerifierStage::Test), "test");
        assert_eq!(format!("{}", VerifierStage::Lint), "lint");
    }
}
