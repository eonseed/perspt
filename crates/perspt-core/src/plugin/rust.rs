use super::*;

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
            (
                "rust-analyzer",
                "language server",
                "rustup component add rust-analyzer",
            ),
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
        &["rs", "Cargo.toml"]
    }

    fn is_test_file(&self, path: &str) -> bool {
        let normalized = path.replace('\\', "/");
        let file = normalized.rsplit('/').next().unwrap_or(&normalized);
        normalized.starts_with("tests/")
            || normalized.contains("/tests/")
            || file.ends_with("_test.rs")
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

    // PSP-7 correction contract

    fn legal_support_files(&self) -> &[&str] {
        &["Cargo.toml", "build.rs"]
    }

    fn manifest_mutation_policy(
        &self,
        manifest_path: &str,
    ) -> crate::types::ManifestMutationPolicy {
        // Allow leaf Cargo.toml edits, deny workspace root mutations
        if manifest_path == "Cargo.toml" {
            // Root workspace Cargo.toml — deny by default
            crate::types::ManifestMutationPolicy::Deny
        } else {
            crate::types::ManifestMutationPolicy::Allow
        }
    }

    fn dependency_commands(
        &self,
        action: crate::plugin::DependencyAction,
        packages: &[String],
        dev: bool,
    ) -> Vec<String> {
        use crate::plugin::DependencyAction;
        let list = packages.join(" ");
        match action {
            DependencyAction::Add if dev => vec![format!("cargo add --dev {list}")],
            DependencyAction::Add => vec![format!("cargo add {list}")],
            DependencyAction::Remove => vec![format!("cargo remove {list}")],
            DependencyAction::Update => vec![format!("cargo update {list}")],
        }
    }

    fn dependency_files(&self) -> Vec<String> {
        vec!["Cargo.toml".into(), "Cargo.lock".into()]
    }

    fn dependency_command_policy(&self, command: &str) -> crate::types::CommandPolicyDecision {
        let trimmed = command.trim();
        if trimmed.starts_with("cargo add ")
            || trimmed.starts_with("cargo install ")
            || trimmed.starts_with("cargo fetch")
        {
            crate::types::CommandPolicyDecision::Allow
        } else if trimmed.starts_with("cargo remove ") {
            crate::types::CommandPolicyDecision::RequireApproval
        } else if trimmed.starts_with("cargo ") {
            // Other cargo subcommands: build, test, check, etc. are fine
            crate::types::CommandPolicyDecision::Allow
        } else {
            crate::types::CommandPolicyDecision::Deny
        }
    }

    fn test_file_patterns(&self) -> &[&str] {
        &["tests/*.rs", "tests/**/*.rs", "**/tests.rs"]
    }
}
