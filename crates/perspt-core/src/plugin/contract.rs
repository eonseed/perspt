use super::*;

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
    /// Syntax / type check (e.g. `cargo check`, `uvx ty check .`)
    SyntaxCheck,
    /// Build step (e.g. `cargo build`, `npm run build`)
    Build,
    /// Test execution (e.g. `cargo test`, `uv run pytest`)
    Test,
    /// Lint pass (e.g. `cargo clippy`, `uv run ruff check .`)
    Lint,
    /// Formatting check (e.g. `cargo fmt --check`). Declared by plugins
    /// whose formatter has a check form; gates only when the run enables
    /// `require_format`.
    Format,
}

impl VerifierStage {
    /// The canonical stage name used by `HardGatePolicy::required_stages`
    /// declarations. Domains declare `"syntax"`, not the `Display` form
    /// `"syntax_check"`; this mapping is the single source for both sides
    /// (PSP-10 Phase 1).
    pub fn policy_name(&self) -> &'static str {
        match self {
            VerifierStage::SyntaxCheck => "syntax",
            VerifierStage::Build => "build",
            VerifierStage::Test => "test",
            VerifierStage::Lint => "lint",
            VerifierStage::Format => "format",
        }
    }
}

impl std::fmt::Display for VerifierStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifierStage::SyntaxCheck => write!(f, "syntax_check"),
            VerifierStage::Build => write!(f, "build"),
            VerifierStage::Test => write!(f, "test"),
            VerifierStage::Lint => write!(f, "lint"),
            VerifierStage::Format => write!(f, "format"),
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
/// A governed dependency-management action (Gate J).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyAction {
    Add,
    Remove,
    Update,
}

impl DependencyAction {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "add" => Some(Self::Add),
            "remove" => Some(Self::Remove),
            "update" => Some(Self::Update),
            _ => None,
        }
    }
}

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

    /// Get the command to run the project in a specific directory.
    ///
    /// Override this to inspect pyproject.toml, Cargo.toml, etc. and return a
    /// more appropriate run command than the generic default.
    fn run_command_for_dir(&self, _path: &Path) -> String {
        self.run_command()
    }

    // =========================================================================
    // PSP-5: Capability-Based Runtime Contract
    // =========================================================================

    /// Get the syntax/type check command (e.g., `cargo check`, `uvx ty check .`)
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

    /// The governed formatter command (`run_formatter`; e.g. `cargo fmt`).
    fn format_command(&self) -> Option<String> {
        None
    }

    /// The formatter's non-mutating check form (e.g. `cargo fmt --check`),
    /// declared as the `format` verifier stage when present.
    fn format_check_command(&self) -> Option<String> {
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

    /// Whether an existing file contributes executable test-oracle semantics.
    /// The governed candidate runs a second verifier pass with source
    /// pre-images restored whenever one of these files is modified.
    fn is_test_file(&self, _path: &str) -> bool {
        false
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

    // =========================================================================
    // PSP-7: Correction Contract
    // =========================================================================

    /// Legal support files that the LLM is allowed to create beyond declared
    /// `output_files` (e.g., `Cargo.toml` for Rust, `__init__.py` for Python).
    ///
    /// These are files that commonly accompany code generation but are not
    /// explicitly listed in the plan. The typed parse pipeline's Layer E
    /// uses this to accept known auxiliary files without flagging them as
    /// ownership violations.
    fn legal_support_files(&self) -> &[&str] {
        &[]
    }

    /// Policy for manifest file mutations produced by the LLM.
    ///
    /// Returns whether a given manifest path may be modified. Plugins can
    /// deny mutations to key files (e.g., root `Cargo.toml` in a workspace)
    /// while allowing leaf-level manifest edits.
    fn manifest_mutation_policy(
        &self,
        _manifest_path: &str,
    ) -> crate::types::ManifestMutationPolicy {
        crate::types::ManifestMutationPolicy::Allow
    }

    /// The command sequence realizing a governed dependency action, or
    /// empty when this plugin does not support dependency mutation. Every
    /// returned command must itself satisfy `dependency_command_policy`,
    /// and the handler re-checks it (fail closed).
    fn dependency_commands(
        &self,
        _action: DependencyAction,
        _packages: &[String],
        _dev: bool,
    ) -> Vec<String> {
        Vec::new()
    }

    /// Workspace-relative manifest and lockfile paths a dependency command
    /// may mutate — the promotable footprint of `mutate_dependencies`.
    fn dependency_files(&self) -> Vec<String> {
        Vec::new()
    }

    /// Policy for dependency-management commands emitted by the LLM.
    ///
    /// Replaces the hardcoded command allowlist in the correction pipeline.
    /// Each command string (e.g., `"cargo add serde"`) is checked against
    /// this policy before execution.
    fn dependency_command_policy(&self, _command: &str) -> crate::types::CommandPolicyDecision {
        crate::types::CommandPolicyDecision::Allow
    }

    /// Glob patterns that identify test files for this language.
    ///
    /// Used by plan validation to infer that test-type tasks should depend on
    /// the code tasks whose output files match these patterns' sibling sources.
    fn test_file_patterns(&self) -> &[&str] {
        &[]
    }
}
