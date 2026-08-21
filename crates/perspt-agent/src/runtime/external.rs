//! MCP edge-adapter wiring (PSP-9 system 13, Gates K, L, U).
//!
//! Servers come from `[[external_tools]]`; with none configured nothing
//! changes. Latest-only discovery (`server/discover` + `tools/list`) happens at
//! node assembly, every listed tool passes `admit_external_tool` against
//! the session's *derived grant surface* (a server can never exceed what
//! the user could grant), and admitted entries enter the assembled catalog
//! like any registered family. Execution dispatches through the handler
//! registry's fallback; the loop brackets each call in the external-effect
//! log because admitted entries are durable.

use super::*;

/// Build the agent-mode external runtime when servers are configured.
pub(super) fn external_runtime_from(
    config: &perspt_core::Config,
    sampling: Option<Arc<dyn crate::external_tools::McpSamplingProvider>>,
) -> Result<Option<Arc<tokio::sync::Mutex<crate::external_tools::ExternalToolRuntime>>>> {
    let has_agent_servers = config
        .external_tools
        .iter()
        .any(|server| server.supports(perspt_core::ExternalToolMode::Agent));
    if !has_agent_servers {
        return Ok(None);
    }
    let mut runtime = crate::external_tools::ExternalToolRuntime::from_config(
        config,
        perspt_core::ExternalToolMode::Agent,
        Vec::new(),
    )?;
    runtime.set_client_services(crate::external_tools::McpClientServices {
        sampling,
        elicitation: Some(Arc::new(
            crate::external_tools::DecliningMcpElicitationProvider,
        )),
    });
    Ok(Some(Arc::new(tokio::sync::Mutex::new(runtime))))
}

/// The builtin handler registry with the MCP dispatcher as fallback when
/// servers are configured.
pub(super) fn registry_with_external(
    external: &Option<Arc<tokio::sync::Mutex<crate::external_tools::ExternalToolRuntime>>>,
) -> crate::tools::handlers::CandidateHandlerRegistry {
    let mut handlers = crate::tools::handlers::CandidateHandlerRegistry::with_builtins();
    if let Some(external) = external {
        handlers.set_fallback(Arc::new(
            crate::tools::handlers::external::ExternalDispatcher::new(external.clone()),
        ));
    }
    handlers
}

impl Psp9AgentRuntime {
    /// The domain + registered-family + external catalog for one node.
    pub(super) fn assemble_catalog(
        &self,
        label: &str,
        external_entries: &[perspt_sdk::ToolEntry],
    ) -> Result<StaticCatalog> {
        let scope = perspt_sdk::DomainScope {
            label: label.to_string(),
            paths: Vec::new(),
        };
        let mut entries = self.domain.tool_entries(&scope);
        entries.extend(self.extra_tool_entries.iter().cloned());
        entries.extend(external_entries.iter().cloned());
        StaticCatalog::with_base(entries).map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    /// Discover admitted external entries once per session. A failing
    /// server is recorded and skipped — a broken MCP server degrades the
    /// surface, never the run.
    pub(super) async fn external_tool_entries(
        &self,
        recorder: &Psp9Recorder,
    ) -> Result<Vec<perspt_sdk::ToolEntry>> {
        let Some(external) = &self.external else {
            return Ok(Vec::new());
        };
        if let Some(cached) = self.external_entries.lock().unwrap().clone() {
            return Ok(cached);
        }
        // Admission runs against the session's derived grant surface — the
        // same catalog-derived effects the worker capability will carry,
        // computed WITHOUT external entries (no self-admission).
        let admission_catalog = self.assemble_catalog("external-admission", &[])?;
        let capability = worker_capability(
            "external-admission",
            "external-admission",
            0,
            0,
            &admission_catalog,
            &self.opted_in_effects(),
        );
        let mut guard = external.lock().await;
        guard.set_capabilities(vec![capability]);
        let servers: Vec<String> = guard.server_ids().into_iter().map(str::to_string).collect();
        for server_id in servers {
            match guard.discover_server(&server_id).await {
                Ok(admitted) => {
                    let rejected = guard.admission_rejections(&server_id);
                    recorder.record_custom(
                        "external_server_discovered",
                        serde_json::json!({
                            "server": server_id,
                            "admitted": admitted.len(),
                            "rejected": rejected.len(),
                        }),
                    )?;
                    for rejection in rejected {
                        recorder.record_custom(
                            "external_tool_rejected",
                            serde_json::json!({
                                "server": rejection.server_id,
                                "tool": rejection.remote_tool,
                                "reason": rejection.reason,
                            }),
                        )?;
                    }
                }
                Err(error) => recorder.record_custom(
                    "external_server_failed",
                    serde_json::json!({
                        "server": server_id,
                        "error": error.to_string(),
                    }),
                )?,
            }
        }
        let entries = guard.admitted_entries();
        *self.external_entries.lock().unwrap() = Some(entries.clone());
        Ok(entries)
    }
}
