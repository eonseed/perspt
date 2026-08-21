//! Chat-local slash commands and conversation export (PSP-1
//! decomposition from `chat_app`): `/save`, `/clear`, and friends never
//! reach the provider — they act on the local message log.

use super::chat_app::{ChatApp, ChatMessage, MessageRole};

impl ChatApp {
    /// Help is assembled in one place so the live loop and event-driven test
    /// path cannot drift apart.
    pub(crate) fn help_text(&self) -> String {
        let tool_count = self
            .chat_tools
            .as_ref()
            .map(|session| session.tool_names().len())
            .unwrap_or(self.mcp_tool_names.len());
        format!(
            "Available Slash Commands:\n\
             \x20 /exit, /quit      - Exit the chat session\n\
             \x20 /clear            - Reset the conversation history\n\
             \x20 /model <name>     - Switch the active model on the fly\n\
             \x20 /save <path>      - Export conversation history to a file\n\
             \x20 /mcp              - Show MCP status and admitted tools\n\
             \x20 /mcp accept [JSON]- Accept the pending MCP elicitation\n\
             \x20 /mcp decline      - Decline the pending MCP elicitation\n\
             \x20 /mcp cancel       - Cancel the pending MCP elicitation\n\
             \x20 Paste shortcut    - Insert Unicode/multiline clipboard text\n\
             \x20 /help             - Show this help menu\n\n{}",
            if self.mcp_configured {
                format!(
                    "MCP is configured for chat; {} read-only tool(s) are admitted. \
                     The model calls them automatically.",
                    tool_count
                )
            } else {
                "MCP is disabled for chat. Add \"chat\" to an external server's modes list to enable it."
                    .to_string()
            }
        )
    }

    /// Local MCP diagnostics. This command never invokes a server or reaches
    /// the model; discovery already happened at chat startup.
    pub(crate) fn mcp_status_text(&self) -> String {
        if !self.mcp_configured {
            return "MCP status: disabled for chat.\n\
                    No chat-enabled server was configured. Set modes = [\"chat\"] or \
                    modes = [\"agent\", \"chat\"] under [[external_tools]]."
                .to_string();
        }

        let tool_names = self
            .chat_tools
            .as_ref()
            .map(|session| session.tool_names())
            .unwrap_or_else(|| self.mcp_tool_names.clone());
        let mut lines = if tool_names.is_empty() {
            vec![
                "MCP status: configured, but no tools were admitted.".to_string(),
                "Declare a local read-only policy for each allowed tool under \
                 [external_tools.tools.<remote-name>], then check the discovery notices below."
                    .to_string(),
            ]
        } else {
            let mut lines = vec![format!(
                "MCP status: {} read-only tool(s) admitted. The model calls them automatically:",
                tool_names.len()
            )];
            lines.extend(tool_names.iter().map(|name| format!("  - {name}")));
            lines
        };

        if !self.mcp_notices.is_empty() {
            lines.push("Discovery notices:".to_string());
            lines.extend(
                self.mcp_notices
                    .iter()
                    .map(|notice| format!("  - {notice}")),
            );
        }
        lines.join("\n")
    }

    pub(crate) fn poll_mcp_elicitation(&mut self) {
        if self.pending_mcp_elicitation.is_some() {
            return;
        }
        let request = self
            .chat_tools
            .as_ref()
            .and_then(|session| session.try_next_elicitation());
        let Some(request) = request else {
            return;
        };
        self.push_message(ChatMessage::system(format!(
            "MCP server {} requests user input (request {}):\n{}\n\
             Reply with /mcp accept {{...}}, /mcp decline, or /mcp cancel. \
             For URL elicitation, open the shown URL yourself before accepting.",
            request.server_id, request.id, request.request
        )));
        self.pending_mcp_elicitation = Some(request);
    }

    pub(crate) fn handle_elicitation(&mut self, text: &str) -> bool {
        let lowered = text.to_ascii_lowercase();
        let action = if lowered == "/mcp decline" {
            Some((perspt_agent::McpElicitationAction::Decline, None))
        } else if lowered == "/mcp cancel" {
            Some((perspt_agent::McpElicitationAction::Cancel, None))
        } else if lowered == "/mcp accept" {
            Some((
                perspt_agent::McpElicitationAction::Accept,
                Some(serde_json::json!({})),
            ))
        } else if lowered.starts_with("/mcp accept ") {
            let raw = text["/mcp accept ".len()..].trim();
            match serde_json::from_str::<serde_json::Value>(raw) {
                Ok(value) if value.is_object() => {
                    Some((perspt_agent::McpElicitationAction::Accept, Some(value)))
                }
                Ok(_) => {
                    self.push_message(ChatMessage::system(
                        "MCP elicitation response must be a JSON object.",
                    ));
                    self.input.clear();
                    return true;
                }
                Err(error) => {
                    self.push_message(ChatMessage::system(format!(
                        "Invalid MCP elicitation JSON: {error}"
                    )));
                    self.input.clear();
                    return true;
                }
            }
        } else {
            return false;
        };

        let Some(pending) = self.pending_mcp_elicitation.take() else {
            self.push_message(ChatMessage::system(
                "There is no pending MCP elicitation request.",
            ));
            self.input.clear();
            return true;
        };
        let (action, content) = action.expect("recognized elicitation command");
        let result = self
            .chat_tools
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("MCP chat session is unavailable"))
            .and_then(|session| session.respond_elicitation(pending.id, action, content));
        match result {
            Ok(()) => self.push_message(ChatMessage::system(format!(
                "MCP elicitation {} answered with {:?}.",
                pending.id, action
            ))),
            Err(error) => self.push_message(ChatMessage::system(format!(
                "MCP elicitation response failed: {error:#}"
            ))),
        }
        self.input.clear();
        true
    }

    /// Save the current conversation history to a file in markdown format
    pub fn save_conversation_to_file(&self, filepath: &str) -> std::io::Result<()> {
        use std::io::Write;

        let path = std::path::Path::new(filepath);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = std::fs::File::create(path)?;
        writeln!(file, "# Perspt Conversation Session")?;
        writeln!(file, "Model: {}\n", self.model)?;

        for msg in &self.messages {
            match msg.role {
                MessageRole::User => {
                    writeln!(file, "## You\n{}\n", msg.content)?;
                }
                MessageRole::Assistant => {
                    writeln!(file, "## Assistant\n")?;
                    if self.show_reasoning {
                        if let Some(ref thought) = msg.reasoning {
                            if !thought.is_empty() {
                                writeln!(file, "> [!NOTE]")?;
                                writeln!(file, "> **Thought Process**")?;
                                for line in thought.lines() {
                                    writeln!(file, "> {}", line)?;
                                }
                                writeln!(file, "\n")?;
                            }
                        }
                    }
                    writeln!(file, "{}\n", msg.content)?;
                }
                MessageRole::System => {
                    writeln!(file, "*System: {}\n*", msg.content)?;
                }
            }
        }

        Ok(())
    }

    /// Handle commands that never enter history or reach the model provider.
    pub(crate) fn handle_local_command(&mut self, text: &str) -> bool {
        if self.love_triggered || perspt_core::local_command::parse_local_command(text).is_none() {
            return false;
        }

        self.love_triggered = true;
        self.input.clear();
        self.push_message(ChatMessage::system(
            perspt_core::local_command::dedication_text(),
        ));
        true
    }
}
