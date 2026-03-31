//! Architect planning: task decomposition, node creation, and fallback graphs.

use super::*;

impl SRBNOrchestrator {
    ///
    /// The Architect analyzes the task and produces a structured Task DAG.
    /// This step retries until a valid JSON plan is produced or max attempts reached.
    pub(super) async fn step_sheafify(&mut self, task: String) -> Result<()> {
        log::info!("Step 1: Sheafification - Planning task decomposition");
        self.emit_log("🏗️ Architect is analyzing the task...".to_string());

        const MAX_ATTEMPTS: usize = 3;
        let mut last_error: Option<String> = None;

        for attempt in 1..=MAX_ATTEMPTS {
            log::info!(
                "Sheafification attempt {}/{}: requesting structured plan",
                attempt,
                MAX_ATTEMPTS
            );

            // Build the structured prompt
            let prompt = self.build_architect_prompt(&task, last_error.as_deref())?;

            // Call the Architect with tier-aware fallback for structured-output failures
            let response = self
                .call_llm_with_tier_fallback(
                    &self.get_architect_model(),
                    &prompt,
                    None,
                    ModelTier::Architect,
                    |resp| {
                        // Validate that the response contains parseable JSON plan
                        if resp.contains("tasks") && (resp.contains('{') && resp.contains('}')) {
                            Ok(())
                        } else {
                            Err("Response does not contain a JSON task plan".to_string())
                        }
                    },
                )
                .await
                .context("Failed to get Architect response")?;

            log::debug!("Architect response length: {} chars", response.len());

            // Try to parse the JSON plan
            match self.parse_task_plan(&response) {
                Ok(plan) => {
                    // Validate the plan
                    if let Err(e) = plan.validate() {
                        log::warn!("Plan validation failed (attempt {}): {}", attempt, e);
                        last_error = Some(format!("Validation error: {}", e));

                        if attempt >= MAX_ATTEMPTS {
                            self.emit_log(format!(
                                "❌ Failed to get valid plan after {} attempts",
                                MAX_ATTEMPTS
                            ));
                            // Fall back to single-task execution
                            return self.create_deterministic_fallback_graph(&task);
                        }
                        continue;
                    }

                    // Check complexity gating
                    if plan.len() > self.context.complexity_k && !self.auto_approve {
                        self.emit_log(format!(
                            "⚠️ Plan has {} tasks (exceeds K={})",
                            plan.len(),
                            self.context.complexity_k
                        ));
                        // TODO: Implement interactive approval
                        // For now, auto-approve in headless mode
                    }

                    // FeatureCharter file-budget gate: reject plans that exceed
                    // the session charter (if one is registered).
                    if let Ok(Some(charter)) = self.ledger.get_feature_charter() {
                        let file_count = plan.tasks.iter().flat_map(|t| &t.output_files).count();
                        if let Some(max_files) = charter.max_files {
                            if file_count > max_files as usize {
                                self.emit_log(format!(
                                    "⚠️ Plan produces {} files but charter allows max {}",
                                    file_count, max_files
                                ));
                            }
                        }
                        if let Some(max_modules) = charter.max_modules {
                            if plan.len() > max_modules as usize {
                                self.emit_log(format!(
                                    "⚠️ Plan has {} tasks but charter allows max {} modules",
                                    plan.len(),
                                    max_modules
                                ));
                            }
                        }
                    }

                    self.emit_log(format!(
                        "✅ Architect produced plan with {} task(s)",
                        plan.len()
                    ));

                    // Emit plan generated event
                    self.emit_event(perspt_core::AgentEvent::PlanGenerated(plan.clone()));

                    // Record initial plan revision for audit trail
                    let revision = perspt_core::types::PlanRevision::initial(
                        &self.context.session_id,
                        plan.clone(),
                    );
                    if let Err(e) = self.ledger.record_plan_revision(&revision) {
                        log::warn!("Failed to persist initial plan revision: {}", e);
                    }

                    // Create nodes from the plan
                    self.create_nodes_from_plan(&plan)?;
                    return Ok(());
                }
                Err(e) => {
                    log::warn!("Plan parsing failed (attempt {}): {}", attempt, e);
                    last_error = Some(format!("JSON parse error: {}", e));

                    if attempt >= MAX_ATTEMPTS {
                        self.emit_log(
                            "⚠️ Could not parse structured plan, using single task".to_string(),
                        );
                        return self.create_deterministic_fallback_graph(&task);
                    }
                }
            }
        }

        // Should not reach here
        self.create_deterministic_fallback_graph(&task)
    }

    /// Build the Architect prompt requesting structured JSON output
    ///
    /// PSP-5 Fix F: Delegates to `ArchitectAgent::build_task_decomposition_prompt`
    /// so the JSON schema contract lives in one place.
    fn build_architect_prompt(&self, task: &str, last_error: Option<&str>) -> Result<String> {
        let mut project_context = self.gather_project_context();

        let error_feedback = if let Some(e) = last_error {
            format!(
                "\n## Previous Attempt Failed\nError: {}\nPlease fix the JSON format and try again.\n",
                e
            )
        } else {
            String::new()
        };

        // PSP-5: For existing projects, prepend a structured project summary
        // and gather evidence (API seams, module boundaries, test layout).
        let (template, evidence_section) = if matches!(
            self.context.workspace_state,
            WorkspaceState::ExistingProject { .. }
        ) {
            let retriever = ContextRetriever::new(self.context.working_dir.clone());
            let summary = retriever.get_project_summary();
            if !summary.is_empty() {
                project_context = format!("{}\n\n{}", summary, project_context);
            }
            let evidence = retriever.gather_architect_evidence();
            (crate::prompts::ARCHITECT_EXISTING, evidence)
        } else {
            (crate::prompts::ARCHITECT_GREENFIELD, String::new())
        };

        Ok(crate::prompts::render_architect(
            template,
            task,
            &self.context.working_dir,
            &project_context,
            &error_feedback,
            &evidence_section,
            &self.context.active_plugins,
        ))
    }

    /// Gather existing project context for the Architect prompt
    /// Uses ContextRetriever to read key configuration files
    fn gather_project_context(&self) -> String {
        let mut context_parts = Vec::new();
        let working_dir = &self.context.working_dir;
        let retriever = ContextRetriever::new(working_dir.clone())
            .with_max_file_bytes(8 * 1024) // 8KB per file for config files
            .with_max_context_bytes(32 * 1024); // 32KB total context

        // Key config files to read (in priority order)
        let config_files = [
            "pyproject.toml",
            "Cargo.toml",
            "package.json",
            "requirements.txt",
        ];

        // Read and include config file contents (up to max_file_bytes)
        let mut found_configs = Vec::new();
        for file in &config_files {
            let path = working_dir.join(file);
            if path.exists() {
                if let Ok(content) = retriever.read_file_truncated(&path) {
                    context_parts.push(format!("### {}\n```\n{}\n```", file, content));
                    found_configs.push(*file);
                }
            }
        }

        // List directory structure
        if let Ok(entries) = std::fs::read_dir(working_dir) {
            let mut dirs = Vec::new();
            let mut files = Vec::new();
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue; // Skip hidden files/dirs
                }
                if entry.path().is_dir() {
                    dirs.push(name);
                } else if !found_configs.contains(&name.as_str()) {
                    files.push(name);
                }
            }

            if !dirs.is_empty() {
                context_parts.push(format!("### Directories\n{}", dirs.join(", ")));
            }
            if !files.is_empty() && files.len() <= 15 {
                context_parts.push(format!("### Other Files\n{}", files.join(", ")));
            } else if !files.is_empty() {
                context_parts.push(format!(
                    "### Other Files\n{} files (not listed)",
                    files.len()
                ));
            }
        }

        if context_parts.is_empty() {
            "Empty directory (greenfield project)".to_string()
        } else {
            context_parts.join("\n\n")
        }
    }

    /// Parse JSON response into TaskPlan
    ///
    /// PSP-5 Phase 4: Uses the provider-neutral normalization layer to extract
    /// JSON from LLM responses regardless of fencing, wrapper text, or provider
    /// formatting quirks. Falls back to raw content parsing if normalization
    /// finds no JSON.
    fn parse_task_plan(&self, content: &str) -> Result<TaskPlan> {
        // PSP-5 Phase 4: Use normalized extraction
        match perspt_core::normalize::extract_and_deserialize::<TaskPlan>(content) {
            Ok((plan, method)) => {
                log::info!("Parsed TaskPlan via normalization ({})", method);
                return Ok(plan);
            }
            Err(e) => {
                log::warn!(
                    "Normalization could not extract TaskPlan: {}. Attempting raw parse.",
                    e
                );
            }
        }

        // Legacy fallback: try direct deserialization of trimmed content
        let trimmed = content.trim();
        log::debug!(
            "Attempting legacy JSON parse: {}...",
            &trimmed[..trimmed.len().min(200)]
        );
        serde_json::from_str(trimmed).context("Failed to parse TaskPlan JSON")
    }

    /// Create SRBN nodes from a parsed TaskPlan
    pub(super) fn create_nodes_from_plan(&mut self, plan: &TaskPlan) -> Result<()> {
        // PSP-5: Validate plan structure including ownership closure before creating nodes
        plan.validate()
            .map_err(|e| anyhow::anyhow!("Plan validation failed: {}", e))?;

        log::info!("Creating {} nodes from plan", plan.len());

        // Create all nodes first
        let mut node_map: HashMap<String, NodeIndex> = HashMap::new();

        for task in &plan.tasks {
            let node = task.to_srbn_node(ModelTier::Actuator);
            let idx = self.add_node(node);
            node_map.insert(task.id.clone(), idx);
            log::info!("  Created node: {} - {}", task.id, task.goal);
        }

        // Wire up dependencies
        for task in &plan.tasks {
            for dep_id in &task.dependencies {
                if let (Some(&from_idx), Some(&to_idx)) =
                    (node_map.get(dep_id), node_map.get(&task.id))
                {
                    self.graph.add_edge(
                        from_idx,
                        to_idx,
                        Dependency {
                            kind: "depends_on".to_string(),
                        },
                    );
                    log::debug!("  Wired dependency: {} -> {}", dep_id, task.id);

                    // PSP-5 Phase 8: Persist graph edge for resume reconstruction
                    if let Err(e) =
                        self.ledger
                            .record_task_graph_edge(dep_id, &task.id, "depends_on")
                    {
                        log::warn!(
                            "Failed to persist graph edge {} -> {}: {}",
                            dep_id,
                            task.id,
                            e
                        );
                    }
                }
            }
        }

        // PSP-5 Phase 2: Build ownership manifest from plan output_files
        self.build_ownership_manifest_from_plan(plan);

        Ok(())
    }

    /// PSP-5 Phase 2: Build ownership manifest from a TaskPlan
    ///
    /// Assigns each task's output_files to the owning node, detecting the
    /// language plugin from file extension via the plugin registry.
    /// Uses majority-vote across ALL output files instead of first-file-only heuristic.
    fn build_ownership_manifest_from_plan(&mut self, plan: &TaskPlan) {
        let registry = perspt_core::plugin::PluginRegistry::new();

        for task in &plan.tasks {
            // Detect plugin via majority vote across ALL output files
            let mut plugin_votes: HashMap<String, usize> = HashMap::new();
            for f in &task.output_files {
                let detected = registry
                    .all()
                    .iter()
                    .find(|p| p.owns_file(f))
                    .map(|p| p.name().to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                *plugin_votes.entry(detected).or_insert(0) += 1;
            }

            let plugin_name = plugin_votes
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(name, _)| name)
                .unwrap_or_else(|| "unknown".to_string());

            // Warn on mixed-plugin tasks (non-Integration nodes)
            if task.node_class != perspt_core::types::NodeClass::Integration {
                let mixed: Vec<String> = task
                    .output_files
                    .iter()
                    .filter_map(|f| {
                        let det = registry
                            .all()
                            .iter()
                            .find(|p| p.owns_file(f))
                            .map(|p| p.name().to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                        if det != plugin_name {
                            Some(format!("'{}' ({})", f, det))
                        } else {
                            None
                        }
                    })
                    .collect();
                if !mixed.is_empty() {
                    log::warn!(
                        "Task '{}' has mixed-plugin outputs (primary: {}): {}",
                        task.id,
                        plugin_name,
                        mixed.join(", ")
                    );
                }
            }

            // Set owner_plugin on the node if we can find it
            if let Some(idx) = self.node_indices.get(&task.id) {
                self.graph[*idx].owner_plugin = plugin_name.clone();
            }

            // Register each output file in the manifest
            for file in &task.output_files {
                // Detect per-file plugin for accurate manifest entries
                let file_plugin = registry
                    .all()
                    .iter()
                    .find(|p| p.owns_file(file))
                    .map(|p| p.name().to_string())
                    .unwrap_or_else(|| plugin_name.clone());

                self.context.ownership_manifest.assign(
                    file.clone(),
                    task.id.clone(),
                    file_plugin,
                    task.node_class,
                );
            }
        }

        log::info!(
            "Built ownership manifest: {} entries",
            self.context.ownership_manifest.len()
        );
    }

    /// Get the Architect model name
    fn get_architect_model(&self) -> String {
        self.architect_model.clone()
    }

    /// PSP-5: Create a deterministic fallback execution graph
    ///
    /// When the Architect fails to produce a valid JSON plan after MAX_ATTEMPTS,
    /// this creates a minimal 3-node graph: scaffold → implement → test.
    fn create_deterministic_fallback_graph(&mut self, task: &str) -> Result<()> {
        log::warn!("Using deterministic fallback graph (PSP-5)");
        self.emit_log("📦 Using deterministic fallback plan");

        // Emit FallbackPlanner event
        self.emit_event(perspt_core::AgentEvent::FallbackPlanner {
            reason: "Architect failed to produce valid JSON plan".to_string(),
        });

        // Detect language for file extensions
        let lang = self.detect_language_from_task(task).unwrap_or("python");
        let ext = match lang {
            "rust" => "rs",
            "javascript" => "js",
            _ => "py",
        };

        // Determine file names based on language
        let (main_file, test_file) = match lang {
            "rust" => (
                "src/main.rs".to_string(),
                "tests/integration_test.rs".to_string(),
            ),
            "javascript" => ("index.js".to_string(), "test/index.test.js".to_string()),
            _ => ("main.py".to_string(), format!("tests/test_main.{}", ext)),
        };

        // Node 1: Scaffold/structure
        let scaffold_task = perspt_core::types::PlannedTask {
            id: "scaffold".to_string(),
            goal: format!("Set up project structure for: {}", task),
            context_files: vec![],
            output_files: vec![main_file.clone()],
            dependencies: vec![],
            task_type: perspt_core::types::TaskType::Code,
            contract: Default::default(),
            command_contract: None,
            node_class: perspt_core::types::NodeClass::Interface,
            dependency_expectations: Default::default(),
        };

        // Node 2: Core implementation
        let impl_task = perspt_core::types::PlannedTask {
            id: "implement".to_string(),
            goal: format!("Implement core logic for: {}", task),
            context_files: vec![main_file.clone()],
            output_files: vec![main_file],
            dependencies: vec!["scaffold".to_string()],
            task_type: perspt_core::types::TaskType::Code,
            contract: Default::default(),
            command_contract: None,
            node_class: perspt_core::types::NodeClass::Implementation,
            dependency_expectations: Default::default(),
        };

        // Node 3: Tests
        let test_task = perspt_core::types::PlannedTask {
            id: "test".to_string(),
            goal: format!("Write tests for: {}", task),
            context_files: vec![],
            output_files: vec![test_file],
            dependencies: vec!["implement".to_string()],
            task_type: perspt_core::types::TaskType::UnitTest,
            contract: Default::default(),
            command_contract: None,
            node_class: perspt_core::types::NodeClass::Integration,
            dependency_expectations: Default::default(),
        };

        // Build plan and emit
        let mut plan = perspt_core::types::TaskPlan::new();
        plan.tasks.push(scaffold_task);
        plan.tasks.push(impl_task);
        plan.tasks.push(test_task);

        self.emit_event(perspt_core::AgentEvent::PlanGenerated(plan.clone()));
        self.create_nodes_from_plan(&plan)?;

        Ok(())
    }
}
