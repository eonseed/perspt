//! Sandboxed, long-lived `lsp_query` sessions against the candidate overlay.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};

use super::{CandidateHandlerRegistry, CandidateToolHandler};
use crate::candidate::CandidateWorkspace;
use crate::toolloop::EffectOutcome;

/// One live language-server session and the versions of documents it has
/// been shown.
pub(crate) struct LspSession {
    client: crate::lsp::LspClient,
    versions: HashMap<String, i32>,
}

/// Per-plugin LSP sessions, keyed by plugin name and shared across a
/// candidate's lifetime.
pub(crate) type LspSessions = tokio::sync::Mutex<HashMap<String, LspSession>>;

struct LspQuery;

#[async_trait::async_trait]
impl CandidateToolHandler for LspQuery {
    async fn apply(
        &self,
        workspace: &CandidateWorkspace,
        call: &perspt_sdk::ProviderToolCall,
        _entry: &perspt_sdk::ToolEntry,
    ) -> Result<EffectOutcome> {
        let relative = call
            .arguments
            .get("path")
            .and_then(serde_json::Value::as_str)
            .context("lsp_query requires path")?;
        let relative = workspace.validate_relative(relative)?;
        let overlay_root = workspace.overlay_root().to_path_buf();
        let path = overlay_root.join(&relative);
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading LSP document {relative}"))?;
        let registry = perspt_core::PluginRegistry::new();
        let plugin = registry
            .detect_all(&overlay_root)
            .into_iter()
            .find(|plugin| plugin.owns_file(&relative))
            .context("no language plugin owns the LSP document")?;
        let config = plugin.get_lsp_config();
        let mut sessions = workspace.lsp_sessions().lock().await;
        if !sessions.contains_key(plugin.name()) {
            let mut client = crate::lsp::LspClient::from_config(&config);
            client.start_with_config(&config, &overlay_root).await?;
            sessions.insert(
                plugin.name().to_string(),
                LspSession {
                    client,
                    versions: HashMap::new(),
                },
            );
        }
        let session = sessions
            .get_mut(plugin.name())
            .context("LSP session disappeared after insertion")?;
        if let Some(version) = session.versions.get_mut(&relative) {
            *version += 1;
            session.client.did_change(&path, &content, *version).await?;
        } else {
            session.versions.insert(relative.clone(), 1);
            session.client.did_open(&path, &content).await?;
        }
        let kind = call
            .arguments
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .context("lsp_query requires kind")?;
        let output = dispatch_lsp_query(session, kind, call, &path, &relative, &content).await?;
        Ok(EffectOutcome {
            output,
            mutated: false,
            completed: true,
        })
    }
}

async fn dispatch_lsp_query(
    session: &mut LspSession,
    kind: &str,
    call: &perspt_sdk::ProviderToolCall,
    path: &Path,
    relative: &str,
    content: &str,
) -> Result<String> {
    if kind == "diagnostics" {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        return Ok(serde_json::to_string(
            &session.client.get_diagnostics(relative).await,
        )?);
    }
    let symbol = call
        .arguments
        .get("symbol")
        .and_then(serde_json::Value::as_str)
        .context("definition, references, and hover queries require symbol")?;
    let (line, character) = symbol_position(content, symbol)
        .with_context(|| format!("symbol {symbol:?} not found in {relative}"))?;
    Ok(match kind {
        "definition" => {
            serde_json::to_string(&session.client.goto_definition(path, line, character).await)?
        }
        "references" => serde_json::to_string(
            &session
                .client
                .find_references(path, line, character, true)
                .await,
        )?,
        "hover" => serde_json::to_string(&session.client.hover(path, line, character).await)?,
        other => anyhow::bail!("unknown lsp_query kind {other:?}"),
    })
}

fn symbol_position(content: &str, symbol: &str) -> Option<(u32, u32)> {
    for (line, text) in content.lines().enumerate() {
        if let Some(column) = text.find(symbol) {
            let utf16_column = text[..column].encode_utf16().count();
            return Some((u32::try_from(line).ok()?, u32::try_from(utf16_column).ok()?));
        }
    }
    None
}

pub(super) fn register(registry: &mut CandidateHandlerRegistry) {
    registry
        .register("lsp_query", Arc::new(LspQuery))
        .expect("builtin lsp handler is registered once");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsp_symbol_columns_use_utf16_units() {
        assert_eq!(symbol_position("let cafe = 1;", "cafe"), Some((0, 4)));
        assert_eq!(symbol_position("let s = \"😀\"; y", "y"), Some((0, 14)));
    }
}
