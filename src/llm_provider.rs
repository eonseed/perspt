// src/llm_provider.rs
use std::error::Error;
use tokio::sync::mpsc;
use async_trait::async_trait; // Add this to Cargo.toml if not already present
use crate::config::AppConfig; // Assuming AppConfig will be needed

#[async_trait]
pub trait LLMProvider {
    /// Lists available models.
    /// This might return a specific model name for local LLMs or a list for API-based providers.
    async fn list_models(&self) -> Result<Vec<String>, Box<dyn Error + Send + Sync>>;

    /// Sends a chat request to the LLM.
    /// - `input`: The user's message.
    /// - `model_name`: Identifier for the model (e.g., name for API, path for local).
    /// - `config`: Application configuration, potentially holding API keys, URLs, or model paths.
    /// - `tx`: Sender channel to stream back text responses or errors.
    async fn send_chat_request(
        &self,
        input: &str,
        model_name: &str,
        config: &AppConfig, // Pass AppConfig by reference
        tx: &mpsc::UnboundedSender<String>
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}
