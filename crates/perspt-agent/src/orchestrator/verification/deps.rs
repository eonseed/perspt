use super::*;

impl SRBNOrchestrator {
    // =========================================================================
    // Auto-dependency repair helpers
    // =========================================================================

    /// Parse `cargo check` / `cargo build` stderr and extract crate names that
    /// are missing.  Handles patterns like:
    ///   - `error[E0432]: unresolved import \`thiserror\``
    ///   - `error[E0463]: can't find crate for \`serde\``
    ///   - `use of undeclared crate or module \`clap\``
    pub(crate) fn extract_missing_crates(output: &str) -> Vec<String> {
        use std::collections::HashSet;

        let mut crates: HashSet<String> = HashSet::new();

        for line in output.lines() {
            let lower = line.to_lowercase();

            // Pattern: "use of undeclared crate or module `foo`"
            if lower.contains("undeclared crate or module") {
                if let Some(name) = Self::extract_backtick_ident(line) {
                    if !name.contains("::") {
                        crates.insert(name);
                    }
                }
            }
            // Pattern: "can't find crate for `foo`"
            else if lower.contains("can't find crate for")
                || lower.contains("cant find crate for")
            {
                if let Some(name) = Self::extract_backtick_ident(line) {
                    crates.insert(name);
                }
            }
            // Pattern: "unresolved import `thiserror`" at top level
            else if lower.contains("unresolved import") {
                if let Some(name) = Self::extract_backtick_ident(line) {
                    let root = name.split("::").next().unwrap_or(&name).to_string();
                    if root != "crate" && root != "self" && root != "super" {
                        crates.insert(root);
                    }
                }
            }
        }

        let builtins: HashSet<&str> = ["std", "core", "alloc", "proc_macro", "test"]
            .iter()
            .copied()
            .collect();

        crates
            .into_iter()
            .filter(|c| !builtins.contains(c.as_str()))
            .collect()
    }

    /// Extract the first back-tick–quoted identifier from a line.
    pub(crate) fn extract_backtick_ident(line: &str) -> Option<String> {
        let start = line.find('`')? + 1;
        let rest = &line[start..];
        let end = rest.find('`')?;
        let ident = &rest[..end];
        if ident.is_empty() {
            None
        } else {
            Some(ident.to_string())
        }
    }

    /// Extract dependency commands from a correction LLM response.
    /// PSP-7: Extract dependency commands from correction response, validated by plugin policy.
    ///
    /// Replaces the legacy hardcoded allowlist with plugin `dependency_command_policy()`.
    pub(crate) fn extract_commands_from_correction(
        response: &str,
        owner_plugin: &str,
    ) -> Vec<String> {
        let registry = perspt_core::plugin::PluginRegistry::new();
        let plugin = registry.get(owner_plugin);

        let mut commands = Vec::new();
        let mut in_commands_section = false;
        let mut in_code_block = false;

        for line in response.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("Commands:")
                || trimmed.starts_with("**Commands:")
                || trimmed.starts_with("### Commands")
            {
                in_commands_section = true;
                continue;
            }

            if in_commands_section {
                if trimmed.starts_with("```") {
                    in_code_block = !in_code_block;
                    continue;
                }

                if !in_code_block
                    && (trimmed.is_empty()
                        || trimmed.starts_with('#')
                        || trimmed.starts_with("File:")
                        || trimmed.starts_with("Diff:"))
                {
                    in_commands_section = false;
                    continue;
                }

                let cmd = trimmed
                    .trim_start_matches("- ")
                    .trim_start_matches("$ ")
                    .trim();

                if !cmd.is_empty() {
                    let decision = plugin
                        .map(|p| p.dependency_command_policy(cmd))
                        .unwrap_or(perspt_core::types::CommandPolicyDecision::Allow);

                    match decision {
                        perspt_core::types::CommandPolicyDecision::Allow
                        | perspt_core::types::CommandPolicyDecision::RequireApproval => {
                            commands.push(cmd.to_string());
                        }
                        perspt_core::types::CommandPolicyDecision::Deny => {
                            log::warn!(
                                "Command '{}' denied by plugin policy for '{}'",
                                cmd,
                                owner_plugin
                            );
                        }
                    }
                }
            }
        }

        commands
    }

    /// Run `cargo add <crate>` for each missing crate. Returns count of successes.
    pub(crate) async fn auto_install_crate_deps(
        crates: &[String],
        working_dir: &std::path::Path,
    ) -> usize {
        let mut installed = 0usize;
        for krate in crates {
            log::info!("Auto-installing crate: cargo add {}", krate);
            let result = tokio::process::Command::new("cargo")
                .args(["add", krate])
                .current_dir(working_dir)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .await;

            match result {
                Ok(output) if output.status.success() => {
                    log::info!("Successfully installed crate: {}", krate);
                    installed += 1;
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    log::warn!("Failed to install crate {}: {}", krate, stderr);
                }
                Err(e) => {
                    log::warn!("Failed to run cargo add {}: {}", krate, e);
                }
            }
        }
        installed
    }

    // =========================================================================
    // Python auto-dependency repair helpers (uv-first)
    // =========================================================================

    /// Parse Python test/import output and extract module names that are missing.
    ///
    /// Handles patterns like:
    ///   - `ModuleNotFoundError: No module named 'httpx'`
    ///   - `ImportError: cannot import name 'foo' from 'bar'`
    ///   - `E   ModuleNotFoundError: No module named 'pydantic'`
    pub(crate) fn extract_missing_python_modules(output: &str) -> Vec<String> {
        use std::collections::HashSet;

        let mut modules: HashSet<String> = HashSet::new();

        for line in output.lines() {
            let trimmed = line.trim().trim_start_matches("E").trim();

            // Pattern: "ModuleNotFoundError: No module named 'foo'"
            // Also matches: "ModuleNotFoundError: No module named 'foo.bar'"
            // Can appear anywhere in the line (e.g. after FAILED test_x.py::test - ...)
            if trimmed.contains("ModuleNotFoundError: No module named ") {
                // Extract the quoted module name after "No module named "
                if let Some(pos) = trimmed.find("No module named ") {
                    let after = &trimmed[pos + "No module named ".len()..];
                    let name = after.trim().trim_matches('\'').trim_matches('"');
                    let root = name.split('.').next().unwrap_or(name);
                    if !root.is_empty() {
                        modules.insert(root.to_string());
                    }
                }
            }
            // Pattern: "ImportError: cannot import name 'X' from 'Y'"
            // or "ImportError: No module named 'X'"
            else if trimmed.contains("ImportError") && trimmed.contains("No module named") {
                if let Some(start) = trimmed.find('\'') {
                    let rest = &trimmed[start + 1..];
                    if let Some(end) = rest.find('\'') {
                        let name = &rest[..end];
                        let root = name.split('.').next().unwrap_or(name);
                        if !root.is_empty() {
                            modules.insert(root.to_string());
                        }
                    }
                }
            }
        }

        // Filter out standard library modules that are always present
        let stdlib: HashSet<&str> = [
            "os",
            "sys",
            "json",
            "re",
            "math",
            "datetime",
            "collections",
            "itertools",
            "functools",
            "pathlib",
            "typing",
            "abc",
            "io",
            "unittest",
            "logging",
            "argparse",
            "sqlite3",
            "csv",
            "hashlib",
            "tempfile",
            "shutil",
            "copy",
            "contextlib",
            "dataclasses",
            "enum",
            "textwrap",
            "importlib",
            "inspect",
            "traceback",
            "subprocess",
            "threading",
            "multiprocessing",
            "asyncio",
            "socket",
            "http",
            "urllib",
            "xml",
            "html",
            "email",
            "string",
            "struct",
            "array",
            "queue",
            "heapq",
            "bisect",
            "pprint",
            "decimal",
            "fractions",
            "random",
            "secrets",
            "time",
            "calendar",
            "zlib",
            "gzip",
            "zipfile",
            "tarfile",
            "glob",
            "fnmatch",
            "stat",
            "fileinput",
            "codecs",
            "uuid",
            "base64",
            "binascii",
            "pickle",
            "shelve",
            "dbm",
            "platform",
            "signal",
            "mmap",
            "ctypes",
            "configparser",
            "tomllib",
            "warnings",
            "weakref",
            "types",
            "operator",
            "numbers",
            "__future__",
        ]
        .iter()
        .copied()
        .collect();

        modules
            .into_iter()
            .filter(|m| !stdlib.contains(m.as_str()))
            .collect()
    }

    /// Map a Python import name to its PyPI package name.
    ///
    /// Most packages use the same name for import and install, but some
    /// notable exceptions exist. We handle the common ones here.
    pub(crate) fn python_import_to_package(import_name: &str) -> &str {
        match import_name {
            "PIL" | "pil" => "pillow",
            "cv2" => "opencv-python",
            "yaml" => "pyyaml",
            "bs4" => "beautifulsoup4",
            "sklearn" => "scikit-learn",
            "attr" | "attrs" => "attrs",
            "dateutil" => "python-dateutil",
            "dotenv" => "python-dotenv",
            "gi" => "PyGObject",
            "serial" => "pyserial",
            "usb" => "pyusb",
            "wx" => "wxPython",
            "lxml" => "lxml",
            "Crypto" => "pycryptodome",
            "jose" => "python-jose",
            "jwt" => "PyJWT",
            "magic" => "python-magic",
            "docx" => "python-docx",
            "pptx" => "python-pptx",
            "git" => "gitpython",
            "psycopg2" => "psycopg2-binary",
            other => other,
        }
    }

    /// Run `uv add <package>` for each missing Python module. Returns count of successes.
    pub(crate) async fn auto_install_python_deps(
        modules: &[String],
        working_dir: &std::path::Path,
    ) -> usize {
        let mut installed = 0usize;
        for module in modules {
            let package = Self::python_import_to_package(module);
            log::info!("Auto-installing Python package: uv add {}", package);
            let result = tokio::process::Command::new("uv")
                .args(["add", package])
                .current_dir(working_dir)
                .env_remove("VIRTUAL_ENV")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .await;

            match result {
                Ok(output) if output.status.success() => {
                    log::info!("Successfully installed Python package: {}", package);
                    installed += 1;
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    log::warn!("Failed to install Python package {}: {}", package, stderr);
                }
                Err(e) => {
                    log::warn!("Failed to run uv add {}: {}", package, e);
                }
            }
        }

        // Always sync after adding dependencies to ensure venv is up-to-date
        if installed > 0 {
            log::info!("Running uv sync --dev after dependency install...");
            let _ = tokio::process::Command::new("uv")
                .args(["sync", "--dev"])
                .current_dir(working_dir)
                .env_remove("VIRTUAL_ENV")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .await;
        }

        installed
    }

    /// Normalize a dependency command to its uv-first equivalent.
    ///
    /// Converts generic pip/pip3/python -m pip install commands to `uv add`,
    /// leaving already-correct uv commands and non-Python commands unchanged.
    pub(crate) fn normalize_command_to_uv(command: &str) -> String {
        let trimmed = command.trim();

        // pip install foo → uv add foo
        // pip3 install foo → uv add foo
        // python -m pip install foo → uv add foo
        // python3 -m pip install foo → uv add foo
        let pip_install_prefixes = [
            "pip install ",
            "pip3 install ",
            "python -m pip install ",
            "python3 -m pip install ",
        ];
        for prefix in &pip_install_prefixes {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let packages = rest.trim();
                if packages.is_empty() {
                    return command.to_string();
                }
                // Strip -r/--requirement flags (uv add doesn't support those directly)
                if packages.starts_with("-r ") || packages.starts_with("--requirement ") {
                    return format!("uv pip install {}", packages);
                }
                return format!("uv add {}", packages);
            }
        }

        // pip install -e . → uv pip install -e .
        if trimmed.starts_with("pip install -") || trimmed.starts_with("pip3 install -") {
            return format!("uv {}", trimmed);
        }

        command.to_string()
    }
}
