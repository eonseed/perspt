//! Workspace classification, project initialization, tool prerequisites,
//! and project-naming helpers.

use super::*;

impl SRBNOrchestrator {
    /// Step 0: Project Initialization
    ///
    /// Check that required OS and language tools are available before
    /// running init commands. Emits install instructions for any missing tools.
    ///
    /// Returns `true` if all critical tools (needed for init) are present.
    /// Optional tools (LSP, linters) emit warnings but don't block.
    pub(super) fn check_tool_prerequisites(&self, plugin: &dyn perspt_core::plugin::LanguagePlugin) -> bool {
        // Common OS tools that perspt uses for context retrieval and code manipulation
        let common_tools: &[(&str, &str)] = &[
            (
                "grep",
                "Install coreutils: brew install grep (macOS) or apt install grep (Linux)",
            ),
            (
                "sed",
                "Install coreutils: brew install gnu-sed (macOS) or apt install sed (Linux)",
            ),
            (
                "awk",
                "Install coreutils: brew install gawk (macOS) or apt install gawk (Linux)",
            ),
        ];

        let mut missing_critical = Vec::new();
        let mut missing_optional = Vec::new();
        let mut install_instructions = Vec::new();

        // Check common OS tools
        for (binary, hint) in common_tools {
            if !perspt_core::plugin::host_binary_available(binary) {
                missing_optional.push((*binary, "OS utility"));
                install_instructions.push(format!("  {} ({}): {}", binary, "OS utility", hint));
            }
        }

        // Check language-specific tools
        let required = plugin.required_binaries();
        for (binary, role, hint) in &required {
            if !perspt_core::plugin::host_binary_available(binary) {
                // init and build tools are critical; LSP and linters are optional
                if *role == "language server" || role.contains("lint") {
                    missing_optional.push((*binary, role));
                } else {
                    missing_critical.push((*binary, role));
                }
                install_instructions.push(format!("  {} ({}): {}", binary, role, hint));
            }
        }

        // Emit results
        if !missing_critical.is_empty() {
            let names: Vec<String> = missing_critical
                .iter()
                .map(|(b, r)| format!("{} ({})", b, r))
                .collect();
            self.emit_log(format!("🚫 Missing critical tools: {}", names.join(", ")));
        }
        if !missing_optional.is_empty() {
            let names: Vec<String> = missing_optional
                .iter()
                .map(|(b, r)| format!("{} ({})", b, r))
                .collect();
            self.emit_log(format!(
                "⚠️ Missing optional tools (degraded mode): {}",
                names.join(", ")
            ));
        }
        if !install_instructions.is_empty() {
            self.emit_log(format!(
                "📋 Install instructions:\n{}",
                install_instructions.join("\n")
            ));
        }

        if missing_critical.is_empty() {
            if missing_optional.is_empty() {
                self.emit_log(format!("✅ All {} tools available", plugin.name()));
            }
            true
        } else {
            self.emit_log("❌ Cannot proceed with project initialization — install missing critical tools first.".to_string());
            false
        }
    }

    /// Uses the pre-computed `WorkspaceState` to branch between:
    /// - ExistingProject: check tooling sync, gather context, never re-init.
    /// - Greenfield: create isolated project dir, run language-native init.
    /// - Ambiguous: create a child project dir to avoid polluting misc files.
    pub(super) async fn step_init_project(&mut self, task: &str) -> Result<()> {
        let registry = perspt_core::plugin::PluginRegistry::new();
        log::info!(
            "step_init_project: workspace_state={}",
            self.context.workspace_state
        );

        match self.context.workspace_state.clone() {
            WorkspaceState::ExistingProject { ref plugins } => {
                // Existing project — check tooling sync, never re-init
                let plugin_name = plugins.first().map(|s| s.as_str()).unwrap_or("");
                if let Some(plugin) = registry.get(plugin_name) {
                    self.emit_log(format!("📂 Detected existing {} project", plugin.name()));

                    // Pre-flight: check required tools (non-blocking for existing projects)
                    self.check_tool_prerequisites(plugin);

                    match plugin.check_tooling_action(&self.context.working_dir) {
                        perspt_core::plugin::ProjectAction::ExecCommand {
                            command,
                            description,
                        } => {
                            self.emit_log(format!("🔧 Tooling action needed: {}", description));

                            let approval_result = self
                                .await_approval(
                                    perspt_core::ActionType::Command {
                                        command: command.clone(),
                                    },
                                    description.clone(),
                                    None,
                                )
                                .await;

                            if matches!(
                                approval_result,
                                ApprovalResult::Approved | ApprovalResult::ApprovedWithEdit(_)
                            ) {
                                let mut args = HashMap::new();
                                args.insert("command".to_string(), command.clone());
                                let call = ToolCall {
                                    name: "run_command".to_string(),
                                    arguments: args,
                                };
                                let result = self.tools.execute(&call).await;
                                if result.success {
                                    self.emit_log(format!("✅ {}", description));
                                } else {
                                    self.emit_log(format!("❌ Failed: {:?}", result.error));
                                }
                            }
                        }
                        perspt_core::plugin::ProjectAction::NoAction => {
                            self.emit_log("✓ Project tooling is up to date".to_string());
                        }
                    }
                }
            }

            WorkspaceState::Greenfield { ref inferred_lang } => {
                let lang = match inferred_lang.as_deref() {
                    Some(l) => l,
                    None => {
                        // Try to infer from task at init time
                        match self.detect_language_from_task(task) {
                            Some(l) => l,
                            None => {
                                self.emit_log(
                                    "ℹ️ No language detected, skipping project init".to_string(),
                                );
                                return Ok(());
                            }
                        }
                    }
                };

                // In Team/Project mode we always scaffold — the Solo check
                // already ran in run() and would have short-circuited.
                // Only consult the LLM heuristic when execution mode was not
                // explicitly set (defensive guard for future callers).
                if self.context.execution_mode == perspt_core::types::ExecutionMode::Solo {
                    self.emit_log("ℹ️ Solo mode, skipping project scaffolding.".to_string());
                    return Ok(());
                }

                if let Some(plugin) = registry.get(lang) {
                    log::info!(
                        "step_init_project: Greenfield lang={}, initializing project",
                        lang
                    );
                    self.emit_log(format!("🌱 Initializing new {} project", lang));

                    // Pre-flight: check required tools before attempting init
                    if !self.check_tool_prerequisites(plugin) {
                        log::warn!(
                            "step_init_project: tool prerequisites check failed for {}",
                            lang
                        );
                        return Ok(());
                    }

                    // Determine if working directory is empty
                    let is_empty = std::fs::read_dir(&self.context.working_dir)
                        .map(|mut i| i.next().is_none())
                        .unwrap_or(true);

                    // Isolation: if dir is non-empty, create a child directory
                    // to avoid polluting existing files.
                    let project_name = if is_empty {
                        ".".to_string()
                    } else {
                        self.suggest_project_name(task).await
                    };

                    let opts = perspt_core::plugin::InitOptions {
                        name: project_name.clone(),
                        is_empty_dir: is_empty,
                        ..Default::default()
                    };

                    match plugin.get_init_action(&opts) {
                        perspt_core::plugin::ProjectAction::ExecCommand {
                            command,
                            description,
                        } => {
                            log::info!(
                                "step_init_project: init command='{}', awaiting approval",
                                command
                            );
                            let result = self
                                .await_approval(
                                    perspt_core::ActionType::ProjectInit {
                                        command: command.clone(),
                                        suggested_name: project_name.clone(),
                                    },
                                    description.clone(),
                                    None,
                                )
                                .await;

                            let final_name = match &result {
                                ApprovalResult::ApprovedWithEdit(edited) => edited.clone(),
                                _ => project_name.clone(),
                            };
                            log::info!(
                                "step_init_project: approval result={:?}, final_name={}",
                                result,
                                final_name
                            );

                            if matches!(
                                result,
                                ApprovalResult::Approved | ApprovalResult::ApprovedWithEdit(_)
                            ) {
                                let final_command = if final_name != project_name {
                                    let edited_opts = perspt_core::plugin::InitOptions {
                                        name: final_name.clone(),
                                        is_empty_dir: is_empty,
                                        ..Default::default()
                                    };
                                    match plugin.get_init_action(&edited_opts) {
                                        perspt_core::plugin::ProjectAction::ExecCommand {
                                            command,
                                            ..
                                        } => command,
                                        _ => command.clone(),
                                    }
                                } else {
                                    command.clone()
                                };

                                let mut args = HashMap::new();
                                args.insert("command".to_string(), final_command.clone());
                                let call = ToolCall {
                                    name: "run_command".to_string(),
                                    arguments: args,
                                };
                                let exec_result = self.tools.execute(&call).await;
                                if exec_result.success {
                                    self.emit_log(format!(
                                        "✅ Project '{}' initialized",
                                        final_name
                                    ));

                                    // Update working directory to point at the
                                    // isolated project root if we created a child dir.
                                    if final_name != "." {
                                        let new_dir = self.context.working_dir.join(&final_name);
                                        if new_dir.is_dir() {
                                            self.context.working_dir = new_dir.clone();
                                            self.tools =
                                                AgentTools::new(new_dir, !self.auto_approve);
                                            if let Some(ref sender) = self.event_sender {
                                                self.tools.set_event_sender(sender.clone());
                                            }
                                            self.emit_log(format!(
                                                "📁 Working directory: {}",
                                                self.context.working_dir.display()
                                            ));
                                        }
                                    }
                                } else {
                                    self.emit_log(format!(
                                        "❌ Init failed: {:?}",
                                        exec_result.error
                                    ));
                                }
                            }
                        }
                        perspt_core::plugin::ProjectAction::NoAction => {
                            self.emit_log("ℹ️ No initialization action needed".to_string());
                        }
                    }
                }
            }

            WorkspaceState::Ambiguous => {
                // Ambiguous workspace — try to infer language and create a child
                // project directory rather than initializing in place.
                if let Some(lang) = self.detect_language_from_task(task) {
                    // Same as Greenfield: in Team/Project mode always scaffold.
                    if self.context.execution_mode == perspt_core::types::ExecutionMode::Solo {
                        self.emit_log("ℹ️ Solo mode, skipping project scaffolding.".to_string());
                        return Ok(());
                    }

                    if let Some(plugin) = registry.get(lang) {
                        let project_name = self.suggest_project_name(task).await;
                        self.emit_log(format!(
                            "🌱 Ambiguous workspace — creating isolated {} project '{}'",
                            lang, project_name
                        ));

                        // Pre-flight: check required tools before attempting init
                        if !self.check_tool_prerequisites(plugin) {
                            return Ok(());
                        }

                        let opts = perspt_core::plugin::InitOptions {
                            name: project_name.clone(),
                            is_empty_dir: false,
                            ..Default::default()
                        };

                        match plugin.get_init_action(&opts) {
                            perspt_core::plugin::ProjectAction::ExecCommand {
                                command,
                                description,
                            } => {
                                let result = self
                                    .await_approval(
                                        perspt_core::ActionType::ProjectInit {
                                            command: command.clone(),
                                            suggested_name: project_name.clone(),
                                        },
                                        description.clone(),
                                        None,
                                    )
                                    .await;

                                let final_name = match &result {
                                    ApprovalResult::ApprovedWithEdit(edited) => edited.clone(),
                                    _ => project_name.clone(),
                                };

                                if matches!(
                                    result,
                                    ApprovalResult::Approved | ApprovalResult::ApprovedWithEdit(_)
                                ) {
                                    let final_command = if final_name != project_name {
                                        let edited_opts = perspt_core::plugin::InitOptions {
                                            name: final_name.clone(),
                                            is_empty_dir: false,
                                            ..Default::default()
                                        };
                                        match plugin.get_init_action(&edited_opts) {
                                            perspt_core::plugin::ProjectAction::ExecCommand {
                                                command,
                                                ..
                                            } => command,
                                            _ => command.clone(),
                                        }
                                    } else {
                                        command.clone()
                                    };

                                    let mut args = HashMap::new();
                                    args.insert("command".to_string(), final_command.clone());
                                    let call = ToolCall {
                                        name: "run_command".to_string(),
                                        arguments: args,
                                    };
                                    let exec_result = self.tools.execute(&call).await;
                                    if exec_result.success {
                                        self.emit_log(format!(
                                            "✅ Project '{}' initialized",
                                            final_name
                                        ));

                                        let new_dir = self.context.working_dir.join(&final_name);
                                        if new_dir.is_dir() {
                                            self.context.working_dir = new_dir.clone();
                                            self.tools =
                                                AgentTools::new(new_dir, !self.auto_approve);
                                            if let Some(ref sender) = self.event_sender {
                                                self.tools.set_event_sender(sender.clone());
                                            }
                                            self.emit_log(format!(
                                                "📁 Working directory: {}",
                                                self.context.working_dir.display()
                                            ));
                                        }
                                    } else {
                                        self.emit_log(format!(
                                            "❌ Init failed: {:?}",
                                            exec_result.error
                                        ));
                                    }
                                }
                            }
                            perspt_core::plugin::ProjectAction::NoAction => {
                                self.emit_log("ℹ️ No initialization action needed".to_string());
                            }
                        }
                    }
                } else {
                    self.emit_log(
                        "ℹ️ Ambiguous workspace and no language detected, skipping project init"
                            .to_string(),
                    );
                }
            }
        }

        Ok(())
    }

    /// Determine if Solo Mode should be used for this task
    /// Solo Mode is for simple single-file tasks that don't need project scaffolding
    /// PSP-5: Detect execution mode from task description
    ///
    /// Project mode is the DEFAULT. Solo mode only activates on explicit
    /// single-file intent keywords or via the `--single-file` CLI flag.
    pub(super) fn detect_execution_mode(&self, task: &str) -> perspt_core::types::ExecutionMode {
        // If execution mode was set explicitly (e.g., CLI flag), honor it
        if self.context.execution_mode != perspt_core::types::ExecutionMode::Project {
            return self.context.execution_mode;
        }

        let task_lower = task.to_lowercase();

        // Only these explicit keywords trigger Solo mode
        let solo_keywords = [
            "single file",
            "single-file",
            "snippet",
            "standalone script",
            "standalone file",
            "one file only",
            "just a file",
        ];

        if solo_keywords.iter().any(|&k| task_lower.contains(k)) {
            log::debug!("Task contains explicit solo keyword, using Solo Mode");
            return perspt_core::types::ExecutionMode::Solo;
        }

        // Default: Project mode for everything else
        log::debug!("Defaulting to Project Mode (PSP-5)");
        perspt_core::types::ExecutionMode::Project
    }

    /// PSP-5: Classify the workspace as existing project, greenfield, or ambiguous.
    ///
    /// This is the single source of truth that drives init/bootstrap/context
    /// strategy for the session. Called once at the start of `run()`.
    pub(super) fn classify_workspace(&self, task: &str) -> WorkspaceState {
        let registry = perspt_core::plugin::PluginRegistry::new();
        let detected: Vec<String> = registry
            .detect_all(&self.context.working_dir)
            .iter()
            .map(|p| p.name().to_string())
            .collect();

        if !detected.is_empty() {
            return WorkspaceState::ExistingProject { plugins: detected };
        }

        // No project metadata found — check if we can infer language from task
        let inferred = self.detect_language_from_task(task).map(|s| s.to_string());

        if inferred.is_some() {
            return WorkspaceState::Greenfield {
                inferred_lang: inferred,
            };
        }

        // Check if directory is empty (empty dirs are greenfield with unknown lang)
        let is_empty = std::fs::read_dir(&self.context.working_dir)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(true);

        if is_empty {
            return WorkspaceState::Greenfield {
                inferred_lang: None,
            };
        }

        // Has files but no project metadata and no language inferred
        WorkspaceState::Ambiguous
    }

    /// Probe verifier readiness for active plugins and emit `ToolReadiness` event.
    ///
    /// Reads `self.context.active_plugins` and emits available/degraded stage
    /// info so TUI/headless surfaces know which verification stages are ready.
    pub(super) fn emit_plugin_readiness(&self) {
        let registry = perspt_core::plugin::PluginRegistry::new();
        let mut plugin_readiness = Vec::new();

        for plugin_name in &self.context.active_plugins {
            if let Some(plugin) = registry.get(plugin_name) {
                let profile = plugin.verifier_profile();
                let available: Vec<String> = profile
                    .capabilities
                    .iter()
                    .filter(|c| c.available)
                    .map(|c| c.stage.to_string())
                    .collect();
                let degraded: Vec<String> = profile
                    .capabilities
                    .iter()
                    .filter(|c| !c.available && c.fallback_available)
                    .map(|c| format!("{} (fallback)", c.stage))
                    .chain(
                        profile
                            .capabilities
                            .iter()
                            .filter(|c| !c.any_available())
                            .map(|c| format!("{} (unavailable)", c.stage)),
                    )
                    .collect();
                let lsp_status = if profile.lsp.primary_available {
                    format!("{} (primary)", profile.lsp.primary.server_binary)
                } else if profile.lsp.fallback_available {
                    profile
                        .lsp
                        .fallback
                        .as_ref()
                        .map(|f| format!("{} (fallback)", f.server_binary))
                        .unwrap_or_else(|| "fallback".to_string())
                } else {
                    "unavailable".to_string()
                };

                if !degraded.is_empty() {
                    self.emit_log(format!(
                        "⚠️ Plugin '{}' degraded: {}",
                        plugin_name,
                        degraded.join(", ")
                    ));
                }

                plugin_readiness.push(perspt_core::events::PluginReadiness {
                    plugin_name: plugin_name.clone(),
                    available_stages: available,
                    degraded_stages: degraded,
                    lsp_status,
                });
            }
        }

        self.emit_event_ref(perspt_core::AgentEvent::ToolReadiness {
            plugins: plugin_readiness,
            strictness: format!("{:?}", self.context.verifier_strictness),
        });
    }

    /// Re-detect plugins after greenfield project initialization.
    ///
    /// Updates `self.context.active_plugins` and `workspace_state`, then
    /// emits plugin readiness so the verifier stack matches the new project.
    pub(super) fn redetect_plugins_after_init(&mut self) {
        let registry = perspt_core::plugin::PluginRegistry::new();
        let detected: Vec<String> = registry
            .detect_all(&self.context.working_dir)
            .iter()
            .map(|p| p.name().to_string())
            .collect();

        if !detected.is_empty() {
            self.emit_log(format!("🔌 Post-init plugins: {}", detected.join(", ")));
            self.context.active_plugins = detected.clone();
            self.context.workspace_state = WorkspaceState::ExistingProject { plugins: detected };
        } else {
            self.emit_log("⚠️ No plugins detected after project init".to_string());
        }

        self.emit_plugin_readiness();
    }

    /// Check that at least one active plugin has a usable build capability.
    ///
    /// Emits a warning when verification will be fully degraded.  This gives
    /// the user early visibility rather than producing a plan whose nodes can
    /// never be meaningfully verified.
    pub(super) fn check_verifier_readiness_gate(&self) {
        if self.context.active_plugins.is_empty() {
            self.emit_log(
                "⚠️ No language plugins active — verification will be fully degraded".to_string(),
            );
            return;
        }

        let registry = perspt_core::plugin::PluginRegistry::new();
        let mut any_build = false;
        for name in &self.context.active_plugins {
            if let Some(plugin) = registry.get(name) {
                let profile = plugin.verifier_profile();
                if !profile.fully_degraded() {
                    any_build = true;
                    break;
                }
            }
        }

        if !any_build {
            self.emit_log(
                "⚠️ All active plugins report fully degraded verifier — \
                 build/test results may be unreliable"
                    .to_string(),
            );
        }
    }

    /// Detect language from task description using heuristics
    pub(super) fn detect_language_from_task(&self, task: &str) -> Option<&'static str> {
        let task_lower = task.to_lowercase();

        if task_lower.contains("rust") || task_lower.contains("cargo") {
            Some("rust")
        } else if task_lower.contains("python")
            || task_lower.contains("flask")
            || task_lower.contains("django")
            || task_lower.contains("fastapi")
            || task_lower.contains("pytorch")
            || task_lower.contains("tensorflow")
            || task_lower.contains("pandas")
            || task_lower.contains("numpy")
            || task_lower.contains("scikit")
            || task_lower.contains("sklearn")
            || task_lower.contains("ml ")
            || task_lower.contains("machine learning")
            || task_lower.contains("deep learning")
            || task_lower.contains("neural")
            || task_lower.contains("dcf")
            || task_lower.contains("data science")
            || task_lower.contains("jupyter")
            || task_lower.contains("notebook")
            || task_lower.contains("streamlit")
            || task_lower.contains("gradio")
            || task_lower.contains("huggingface")
            || task_lower.contains("transformers")
            || task_lower.contains("llm")
            || task_lower.contains("langchain")
            || task_lower.contains("pydantic")
        {
            Some("python")
        } else if task_lower.contains("javascript")
            || task_lower.contains("typescript")
            || task_lower.contains("node")
            || task_lower.contains("react")
            || task_lower.contains("vue")
            || task_lower.contains("angular")
            || task_lower.contains("next.js")
            || task_lower.contains("nextjs")
        {
            Some("javascript")
        } else if task_lower.contains("app") || task_lower.contains("application") {
            // Default to Python for generic "app" or "application" tasks
            Some("python")
        } else {
            None
        }
    }

    /// Suggest a meaningful project name from the task description
    async fn suggest_project_name(&self, task: &str) -> String {
        // 1. Try heuristic extraction first (fast, no LLM)
        if let Some(name) = self.extract_name_heuristic(task) {
            self.emit_log(format!("📁 Suggested project folder: {}", name));
            return name;
        }

        // 2. Fallback to LLM for complex tasks
        let prompt = format!(
            r#"Extract a short project name from this task description.
Rules:
- Use snake_case (lowercase with underscores)
- Maximum 30 characters
- Must be a valid folder name (letters, numbers, underscores only)
- Return ONLY the name, nothing else

Task: "{}"

Project name:"#,
            task
        );

        match self
            .call_llm_with_logging(&self.actuator_model.clone(), &prompt, None)
            .await
        {
            Ok(response) => {
                let suggested = response.trim().to_lowercase();
                if let Some(validated) = self.validate_project_name(&suggested) {
                    self.emit_log(format!("📁 Suggested project folder: {}", validated));
                    return validated;
                }
            }
            Err(e) => {
                log::warn!("Failed to get project name from LLM: {}", e);
            }
        }

        // 3. Final fallback
        let fallback = "perspt_app".to_string();
        self.emit_log(format!("📁 Using default folder: {}", fallback));
        fallback
    }

    /// Extract project name from task using stop-word removal (no LLM)
    fn extract_name_heuristic(&self, task: &str) -> Option<String> {
        let task_lower = task.to_lowercase();

        // Stop words to remove
        let stop_words = [
            // Verbs
            "create",
            "build",
            "make",
            "implement",
            "develop",
            "write",
            "design",
            "add",
            "setup",
            "set",
            "up",
            "generate",
            "please",
            "can",
            "you",
            // Articles
            "a",
            "an",
            "the",
            "this",
            "that",
            // Prepositions
            "in",
            "on",
            "for",
            "with",
            "using",
            "to",
            "from",
            // Languages (we don't want these in folder names)
            "python",
            "rust",
            "javascript",
            "typescript",
            "node",
            "nodejs",
            "react",
            "vue",
            "angular",
            "django",
            "flask",
            "fastapi",
            // Generic terms
            "simple",
            "basic",
            "new",
            "my",
            "our",
            "your",
        ];

        // Split into words and filter
        let words: Vec<&str> = task_lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .filter(|w| !stop_words.contains(w))
            .filter(|w| w.len() > 1) // Skip single chars
            .take(3) // Max 3 words
            .collect();

        if words.is_empty() {
            return None;
        }

        // Join words with underscore
        let name = words.join("_");

        // Validate
        self.validate_project_name(&name)
    }

    /// Validate and sanitize a project name
    fn validate_project_name(&self, name: &str) -> Option<String> {
        // Must start with letter, contain only letters/numbers/underscores
        let cleaned: String = name
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .take(30)
            .collect();

        if cleaned.is_empty() {
            return None;
        }

        // Ensure it starts with a letter
        let first = cleaned.chars().next()?;
        if !first.is_alphabetic() {
            return None;
        }

        Some(cleaned)
    }
}
