// src/llm_provider.rs
use async_trait::async_trait;
use tokio::sync::mpsc;
use anyhow::Result;
use crate::config::AppConfig;

/// Represents different types of LLM providers
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderType {
    Local,
    OpenAI,
    Gemini,
}

/// Result type for LLM operations
pub type LLMResult<T> = Result<T>;

/// Trait for LLM providers with modern async interface
#[async_trait]
pub trait LLMProvider {
    /// Lists available models for this provider
    async fn list_models(&self) -> LLMResult<Vec<String>>;

    /// Sends a chat request to the LLM with streaming response
    /// 
    /// # Arguments
    /// * `input` - The user's message/prompt
    /// * `model_name` - Model identifier (name for API, path for local)
    /// * `config` - Application configuration
    /// * `tx` - Channel sender for streaming responses
    async fn send_chat_request(
        &self,
        input: &str,
        model_name: &str,
        config: &AppConfig,
        tx: &mpsc::UnboundedSender<String>,
    ) -> LLMResult<()>;

    /// Returns the provider type
    fn provider_type(&self) -> ProviderType;

    /// Validates if the provider can be used with the given configuration
    async fn validate_config(&self, config: &AppConfig) -> LLMResult<()>;
}
