use super::*;

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
            (
                "uv",
                "package manager",
                "curl -LsSf https://astral.sh/uv/install.sh | sh",
            ),
            (
                "python3",
                "interpreter",
                "uv python install (or install from https://python.org)",
            ),
            (
                "uvx",
                "tool runner/LSP",
                "Installed with uv — curl -LsSf https://astral.sh/uv/install.sh | sh",
            ),
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
            Some("pipenv") => {
                if opts.is_empty_dir || opts.name == "." || opts.name == "./" {
                    "pipenv install".to_string()
                } else {
                    format!(
                        "mkdir -p {} && cd {} && pipenv install",
                        opts.name, opts.name
                    )
                }
            }
            // uv is the default for any other (or unspecified) value — the plugin
            // owns this fallback, so an unrecognized manager degrades gracefully.
            _ => {
                // Default to uv --lib for src-layout with build-system
                if opts.is_empty_dir || opts.name == "." || opts.name == "./" {
                    "uv init --lib".to_string()
                } else {
                    format!("uv init --lib {}", opts.name)
                }
            }
        };
        let description = match opts.package_manager.as_deref() {
            Some("poetry") => "Initialize Python project with Poetry",
            Some("pdm") => "Initialize Python project with PDM",
            Some("pipenv") => "Initialize Python project with Pipenv",
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
            // uv init --lib for src-layout with build-system
            format!("uv init --lib {}", opts.name)
        }
    }

    fn test_command(&self) -> String {
        "uv run pytest".to_string()
    }

    fn run_command(&self) -> String {
        "uv run python -m main".to_string()
    }

    /// Detect the package name from pyproject.toml or src layout and return
    /// an appropriate run command.
    fn run_command_for_dir(&self, path: &Path) -> String {
        // Check src/<pkg>/__main__.py first
        if let Ok(entries) = std::fs::read_dir(path.join("src")) {
            for entry in entries.flatten() {
                if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !name.starts_with('.') && !name.starts_with('_') {
                        return format!("uv run python -m {}", name);
                    }
                }
            }
        }

        // Check for [project.scripts] in pyproject.toml
        if let Ok(content) = std::fs::read_to_string(path.join("pyproject.toml")) {
            if content.contains("[project.scripts]") {
                // Parse the first script name
                let mut in_scripts = false;
                for raw_line in content.lines() {
                    let line = raw_line.trim();
                    if line == "[project.scripts]" {
                        in_scripts = true;
                        continue;
                    }
                    if in_scripts {
                        if line.starts_with('[') {
                            break;
                        }
                        if let Some((name, _)) = line.split_once('=') {
                            let script = name.trim().trim_matches('"');
                            if !script.is_empty() {
                                return format!("uv run {}", script);
                            }
                        }
                    }
                }
            }
        }

        // Default: run main module
        "uv run python -m main".to_string()
    }

    // PSP-5 capability methods

    fn syntax_check_command(&self) -> Option<String> {
        Some("uvx ty check .".to_string())
    }

    fn lint_command(&self) -> Option<String> {
        Some("uv run ruff check .".to_string())
    }

    fn file_ownership_patterns(&self) -> &[&str] {
        &["py", "pyproject.toml", "setup.py", "requirements.txt"]
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
                command: Some("uvx ty check .".to_string()),
                available: uv,
                // pyright as CLI fallback for syntax checking
                fallback_command: Some("pyright .".to_string()),
                fallback_available: pyright,
            },
            VerifierCapability {
                stage: VerifierStage::Build,
                // Python has no separate build step; declare the capability
                // so the sensor doesn't appear as Unavailable/degraded.
                command: None,
                available: true,
                fallback_command: None,
                fallback_available: false,
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

    // PSP-7 correction contract

    fn legal_support_files(&self) -> &[&str] {
        &[
            "pyproject.toml",
            "setup.py",
            "setup.cfg",
            "__init__.py",
            "conftest.py",
        ]
    }

    fn dependency_command_policy(&self, command: &str) -> crate::types::CommandPolicyDecision {
        let trimmed = command.trim();
        if trimmed.starts_with("uv add ")
            || trimmed.starts_with("uv pip install ")
            || trimmed.starts_with("pip install ")
            || trimmed.starts_with("uv sync")
        {
            crate::types::CommandPolicyDecision::Allow
        } else if trimmed.starts_with("uv remove ") || trimmed.starts_with("pip uninstall ") {
            crate::types::CommandPolicyDecision::RequireApproval
        } else {
            crate::types::CommandPolicyDecision::Deny
        }
    }

    fn correction_prompt_fragment(&self) -> Option<&str> {
        Some(
            "For Python projects: use `uv add <package>` to add dependencies. \
             Ensure new packages are listed in pyproject.toml [project.dependencies]. \
             Create `__init__.py` files for new packages.",
        )
    }

    fn test_file_patterns(&self) -> &[&str] {
        &["tests/*.py", "tests/**/*.py", "test_*.py", "*_test.py"]
    }
}
