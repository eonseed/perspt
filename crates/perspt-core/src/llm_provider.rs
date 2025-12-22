//! # LLM Provider Module
//!
//! Thread-safe LLM provider abstraction for multi-agent use.
//! Wraps genai::Client with Arc<RwLock<>> for shared state.

use anyhow::{Context, Result};
use futures::StreamExt;
use genai::adapter::AdapterKind;
use genai::chat::{ChatMessage, ChatOptions, ChatRequest, ChatStreamEvent};
use genai::Client;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, RwLock};

/// End of transmission signal
pub const EOT_SIGNAL: &str = "<|EOT|>";

/// Shared state for rate limiting and token counting
struct SharedState {
    total_tokens_used: usize,
    request_count: usize,
}

impl Default for SharedState {
    fn default() -> Self {
        Self {
            total_tokens_used: 0,
            request_count: 0,
        }
    }
}

/// Thread-safe LLM provider implementation using Arc<RwLock<>>.
///
/// This provider can be cheaply cloned and shared across multiple agents.
/// Each clone shares the same underlying client and rate limiting state.
#[derive(Clone)]
pub struct GenAIProvider {
    /// The underlying genai client
    client: Arc<Client>,
    /// Shared state for rate limiting and metrics
    shared: Arc<RwLock<SharedState>>,
}

impl GenAIProvider {
    /// Creates a new GenAI provider with automatic configuration.
    pub fn new() -> Result<Self> {
        let client = Client::default();
        Ok(Self {
            client: Arc::new(client),
            shared: Arc::new(RwLock::new(SharedState::default())),
        })
    }

    /// Creates a new GenAI provider with explicit configuration.
    pub fn new_with_config(provider_type: Option<&str>, api_key: Option<&str>) -> Result<Self> {
        // Set environment variable if API key is provided
        if let (Some(provider), Some(key)) = (provider_type, api_key) {
            let env_var = match provider {
                "openai" => "OPENAI_API_KEY",
                "anthropic" => "ANTHROPIC_API_KEY",
                "gemini" => "GEMINI_API_KEY",
                "groq" => "GROQ_API_KEY",
                "cohere" => "COHERE_API_KEY",
                "xai" => "XAI_API_KEY",
                "deepseek" => "DEEPSEEK_API_KEY",
                "ollama" => {
                    log::info!("Ollama provider detected - no API key required for local setup");
                    return Self::new();
                }
                _ => {
                    log::warn!("Unknown provider type for API key: {provider}");
                    return Self::new();
                }
            };

            log::info!("Setting {env_var} environment variable for genai client");
            std::env::set_var(env_var, key);
        }

        Self::new()
    }

    /// Get total tokens used across all requests
    pub async fn get_total_tokens_used(&self) -> usize {
        self.shared.read().await.total_tokens_used
    }

    /// Get total request count
    pub async fn get_request_count(&self) -> usize {
        self.shared.read().await.request_count
    }

    /// Increment request counter (for metrics)
    async fn increment_request(&self) {
        let mut state = self.shared.write().await;
        state.request_count += 1;
    }

    /// Add tokens to the total count
    pub async fn add_tokens(&self, count: usize) {
        let mut state = self.shared.write().await;
        state.total_tokens_used += count;
    }

    /// Retrieves all available models for a specific provider.
    pub async fn get_available_models(&self, provider: &str) -> Result<Vec<String>> {
        let adapter_kind = str_to_adapter_kind(provider)?;

        let models = self
            .client
            .all_model_names(adapter_kind)
            .await
            .context(format!("Failed to get models for provider: {provider}"))?;

        Ok(models)
    }

    /// Generates a simple text response without streaming.
    pub async fn generate_response_simple(&self, model: &str, prompt: &str) -> Result<String> {
        self.increment_request().await;

        let chat_req = ChatRequest::default().append_message(ChatMessage::user(prompt));

        log::debug!("Sending chat request to model: {model} with prompt: {prompt}");

        let chat_res = self
            .client
            .exec_chat(model, chat_req, None)
            .await
            .context(format!("Failed to execute chat request for model: {model}"))?;

        let content = chat_res
            .first_text()
            .context("No text content in response")?;

        log::debug!("Received response with {} characters", content.len());
        Ok(content.to_string())
    }

    /// Generates a streaming response and sends chunks via mpsc channel.
    pub async fn generate_response_stream_to_channel(
        &self,
        model: &str,
        prompt: &str,
        tx: mpsc::UnboundedSender<String>,
    ) -> Result<()> {
        self.increment_request().await;

        let chat_req = ChatRequest::default().append_message(ChatMessage::user(prompt));

        log::debug!("Sending streaming chat request to model: {model} with prompt: {prompt}");

        let chat_res_stream = self
            .client
            .exec_chat_stream(model, chat_req, None)
            .await
            .context(format!(
                "Failed to execute streaming chat request for model: {model}"
            ))?;

        let mut stream = chat_res_stream.stream;
        let mut chunk_count = 0;
        let mut total_content_length = 0;
        let mut stream_ended_explicitly = false;
        let start_time = Instant::now();

        log::info!(
            "=== STREAM START === Model: {}, Prompt length: {} chars",
            model,
            prompt.len()
        );

        while let Some(chunk_result) = stream.next().await {
            let elapsed = start_time.elapsed();

            match chunk_result {
                Ok(ChatStreamEvent::Start) => {
                    log::info!(">>> STREAM STARTED for model: {model} at {elapsed:?}");
                }
                Ok(ChatStreamEvent::Chunk(chunk)) => {
                    chunk_count += 1;
                    total_content_length += chunk.content.len();

                    if chunk_count % 10 == 0 || chunk.content.len() > 100 {
                        log::info!(
                            "CHUNK #{}: {} chars, total: {} chars, elapsed: {:?}",
                            chunk_count,
                            chunk.content.len(),
                            total_content_length,
                            elapsed
                        );
                    }

                    if !chunk.content.is_empty() {
                        if tx.send(chunk.content.clone()).is_err() {
                            log::error!(
                                "!!! CHANNEL SEND FAILED for chunk #{chunk_count} - STOPPING STREAM !!!"
                            );
                            break;
                        }
                    }
                }
                Ok(ChatStreamEvent::ReasoningChunk(chunk)) => {
                    log::info!(
                        "REASONING CHUNK: {} chars at {:?}",
                        chunk.content.len(),
                        elapsed
                    );
                }
                Ok(ChatStreamEvent::End(_)) => {
                    log::info!(">>> STREAM ENDED EXPLICITLY for model: {model} after {chunk_count} chunks, {total_content_length} chars, {elapsed:?} elapsed");
                    stream_ended_explicitly = true;
                    break;
                }
                Ok(ChatStreamEvent::ToolCallChunk(_)) => {
                    log::debug!("Tool call chunk received (ignored)");
                }
                Err(e) => {
                    log::error!(
                        "!!! STREAM ERROR after {chunk_count} chunks at {elapsed:?}: {e} !!!"
                    );
                    let error_msg = format!("Stream error: {e}");
                    let _ = tx.send(error_msg);
                    return Err(e.into());
                }
            }
        }

        let final_elapsed = start_time.elapsed();
        if !stream_ended_explicitly {
            log::warn!("!!! STREAM ENDED IMPLICITLY (exhausted) for model: {model} after {chunk_count} chunks, {total_content_length} chars, {final_elapsed:?} elapsed !!!");
        }

        log::info!(
            "=== STREAM COMPLETE === Model: {model}, Final: {chunk_count} chunks, {total_content_length} chars, {final_elapsed:?} elapsed"
        );

        // Add approximate token count
        self.add_tokens(total_content_length / 4).await; // Rough estimate

        if tx.send(EOT_SIGNAL.to_string()).is_err() {
            log::error!("!!! FAILED TO SEND EOT SIGNAL - channel may be closed !!!");
            return Err(anyhow::anyhow!("Channel closed during EOT signal send"));
        }

        log::info!(">>> EOT SIGNAL SENT for model: {model} <<<");
        Ok(())
    }

    /// Generate response with conversation history
    pub async fn generate_response_with_history(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<String> {
        self.increment_request().await;

        let chat_req = ChatRequest::new(messages);

        log::debug!("Sending chat request to model: {model} with conversation history");

        let chat_res = self
            .client
            .exec_chat(model, chat_req, None)
            .await
            .context(format!("Failed to execute chat request for model: {model}"))?;

        let content = chat_res
            .first_text()
            .context("No text content in response")?;

        log::debug!("Received response with {} characters", content.len());
        Ok(content.to_string())
    }

    /// Generate response with custom chat options
    pub async fn generate_response_with_options(
        &self,
        model: &str,
        prompt: &str,
        options: ChatOptions,
    ) -> Result<String> {
        self.increment_request().await;

        let chat_req = ChatRequest::default().append_message(ChatMessage::user(prompt));

        log::debug!("Sending chat request to model: {model} with custom options");

        let chat_res = self
            .client
            .exec_chat(model, chat_req, Some(&options))
            .await
            .context(format!("Failed to execute chat request for model: {model}"))?;

        let content = chat_res
            .first_text()
            .context("No text content in response")?;

        log::debug!("Received response with {} characters", content.len());
        Ok(content.to_string())
    }

    /// Get a list of supported providers
    pub fn get_supported_providers() -> Vec<&'static str> {
        vec![
            "openai",
            "anthropic",
            "gemini",
            "groq",
            "cohere",
            "ollama",
            "xai",
            "deepseek",
        ]
    }

    /// Get all available providers
    pub async fn get_available_providers(&self) -> Result<Vec<String>> {
        Ok(Self::get_supported_providers()
            .iter()
            .map(|s| s.to_string())
            .collect())
    }

    /// Test if a model is available and working
    pub async fn test_model(&self, model: &str) -> Result<bool> {
        match self.generate_response_simple(model, "Hello").await {
            Ok(_) => {
                log::info!("Model {model} is available and working");
                Ok(true)
            }
            Err(e) => {
                log::warn!("Model {model} test failed: {e}");
                Ok(false)
            }
        }
    }

    /// Validate and get the best available model for a provider
    pub async fn validate_model(&self, model: &str, provider_type: Option<&str>) -> Result<String> {
        if self.test_model(model).await? {
            return Ok(model.to_string());
        }

        if let Some(provider) = provider_type {
            if let Ok(models) = self.get_available_models(provider).await {
                if !models.is_empty() {
                    log::info!("Model {} not available, using {} instead", model, models[0]);
                    return Ok(models[0].clone());
                }
            }
        }

        log::warn!("Could not validate model {model}, proceeding anyway");
        Ok(model.to_string())
    }
}

/// Convert a provider string to genai AdapterKind
fn str_to_adapter_kind(provider: &str) -> Result<AdapterKind> {
    match provider.to_lowercase().as_str() {
        "openai" => Ok(AdapterKind::OpenAI),
        "anthropic" => Ok(AdapterKind::Anthropic),
        "gemini" | "google" => Ok(AdapterKind::Gemini),
        "groq" => Ok(AdapterKind::Groq),
        "cohere" => Ok(AdapterKind::Cohere),
        "ollama" => Ok(AdapterKind::Ollama),
        "xai" => Ok(AdapterKind::Xai),
        "deepseek" => Ok(AdapterKind::DeepSeek),
        _ => Err(anyhow::anyhow!("Unsupported provider: {}", provider)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_str_to_adapter_kind() {
        assert!(str_to_adapter_kind("openai").is_ok());
        assert!(str_to_adapter_kind("anthropic").is_ok());
        assert!(str_to_adapter_kind("gemini").is_ok());
        assert!(str_to_adapter_kind("google").is_ok());
        assert!(str_to_adapter_kind("groq").is_ok());
        assert!(str_to_adapter_kind("cohere").is_ok());
        assert!(str_to_adapter_kind("ollama").is_ok());
        assert!(str_to_adapter_kind("xai").is_ok());
        assert!(str_to_adapter_kind("deepseek").is_ok());
        assert!(str_to_adapter_kind("invalid").is_err());
    }

    #[tokio::test]
    async fn test_provider_creation() {
        let provider = GenAIProvider::new();
        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn test_provider_is_clonable() {
        let provider = GenAIProvider::new().unwrap();
        let _clone1 = provider.clone();
        let _clone2 = provider.clone();
        // All clones share the same underlying state
    }
}
