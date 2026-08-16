use super::*;

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
            (
                "node",
                "runtime",
                "Install Node.js from https://nodejs.org or via nvm",
            ),
            (
                "npm",
                "package manager",
                "Included with Node.js — install from https://nodejs.org",
            ),
            (
                "typescript-language-server",
                "language server",
                "npm install -g typescript-language-server typescript",
            ),
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
        &["js", "ts", "jsx", "tsx", "package.json", "tsconfig.json"]
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

    // PSP-7 correction contract

    fn legal_support_files(&self) -> &[&str] {
        &["package.json", "tsconfig.json", "package-lock.json"]
    }

    fn dependency_command_policy(&self, command: &str) -> crate::types::CommandPolicyDecision {
        let trimmed = command.trim();
        if trimmed.starts_with("npm install ")
            || trimmed.starts_with("npm i ")
            || trimmed.starts_with("yarn add ")
            || trimmed.starts_with("pnpm add ")
            || trimmed.starts_with("pnpm install ")
        {
            crate::types::CommandPolicyDecision::Allow
        } else if trimmed.starts_with("npm uninstall ")
            || trimmed.starts_with("yarn remove ")
            || trimmed.starts_with("pnpm remove ")
        {
            crate::types::CommandPolicyDecision::RequireApproval
        } else {
            crate::types::CommandPolicyDecision::Deny
        }
    }

    fn correction_prompt_fragment(&self) -> Option<&str> {
        Some(
            "For JavaScript/TypeScript projects: use `npm install <package>` to add \
             dependencies. Ensure TypeScript projects have a valid tsconfig.json. \
             Use ES module imports consistently.",
        )
    }

    fn test_file_patterns(&self) -> &[&str] {
        &[
            "**/*.test.js",
            "**/*.test.ts",
            "**/*.spec.js",
            "**/*.spec.ts",
            "**/*.test.jsx",
            "**/*.test.tsx",
            "**/*.spec.jsx",
            "**/*.spec.tsx",
        ]
    }
}
