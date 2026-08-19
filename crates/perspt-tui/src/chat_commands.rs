//! Chat-local slash commands and conversation export (PSP-1
//! decomposition from `chat_app`): `/save`, `/clear`, and friends never
//! reach the provider — they act on the local message log.

use super::chat_app::{ChatApp, ChatMessage, MessageRole};

impl ChatApp {
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
