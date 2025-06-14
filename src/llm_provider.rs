//! # LLM Provider Module (llm_provider.rs)
//!
//! This module provides a unified interface for interacting with various Large Language Model (LLM)
//! providers using the modern `genai` crate. It abstracts away provider-specific implementations
//! and provides a consistent API for chat functionality, streaming responses, and model management.
//!
//! ## Supported Providers
//!
//! The module supports multiple LLM providers through the genai crate (v0.3.5):
//! - **OpenAI**: GPT-4, GPT-3.5, GPT-4o, o1-mini, o1-preview, o3-mini, o4-mini models
//! - **Anthropic**: Claude 3 (Opus, Sonnet, Haiku), Claude 3.5 models
//! - **Google**: Gemini Pro, Gemini 1.5 Pro/Flash, Gemini 2.0 models
//! - **Groq**: Llama 3.x models with ultra-fast inference
//! - **Cohere**: Command R/R+ models
//! - **XAI**: Grok models (grok-3-beta, grok-3-fast-beta, etc.)
//! - **DeepSeek**: DeepSeek chat and reasoning models (deepseek-chat, deepseek-reasoner)
//! - **Ollama**: Local model hosting (requires local setup)
//!
//! ## Features
//!
//! - **Unified API**: Single interface across all providers
//! - **Streaming Support**: Real-time response streaming with proper event handling
//! - **Auto Configuration**: Automatic environment variable detection and setup
//! - **Model Validation**: Pre-flight model availability checking
//! - **Error Handling**: Comprehensive error categorization and recovery
//! - **Async/Await**: Full async support with tokio integration
//!
//! ## Architecture
//!
//! The provider uses the genai crate's `Client` as the underlying interface, which handles:
//! - Authentication via environment variables
//! - Provider-specific API endpoints and protocols
//! - Request/response serialization
//! - Rate limiting and retry logic
//!
//! ## Example Usage
//!
//! ```rust
//! use perspt::llm_provider::GenAIProvider;
//! use tokio::sync::mpsc;
//!
//! // Create provider with auto-configuration
//! let provider = GenAIProvider::new()?;
//!
//! // Or create with explicit configuration
//! let provider = GenAIProvider::new_with_config(
//!     Some("openai"),
//!     Some("sk-your-api-key")
//! )?;
//!
//! // Simple text generation
//! let response = provider.generate_response_simple(
//!     "gpt-4o-mini",
//!     "Hello, how are you?"
//! ).await?;
//!
//! // Streaming generation
//! let (tx, rx) = mpsc::unbounded_channel();
//! provider.generate_response_stream_to_channel(
//!     "gpt-4o-mini",
//!     "Tell me a story",
//!     tx
//! ).await?;
//! ```

use anyhow::{Context, Result};
use futures::StreamExt;
use genai::adapter::AdapterKind;
use genai::chat::{ChatMessage, ChatOptions, ChatRequest, ChatStreamEvent};
use genai::Client;
use std::time::Instant;
use tokio::sync::mpsc;

/// A comprehensive LLM provider implementation using the modern genai crate.
///
/// This struct provides a unified interface for interacting with multiple LLM providers
/// through a single API. It handles authentication, model validation, streaming responses,
/// and error management across different providers.
///
/// ## Design Philosophy
///
/// The provider is designed around the principle of "configure once, use everywhere".
/// It automatically handles provider-specific authentication requirements, API endpoints,
/// and response formats while presenting a consistent interface to the application.
///
/// ## Configuration
///
/// The provider supports multiple configuration methods:
/// 1. **Auto-configuration**: Uses environment variables (recommended)
/// 2. **Explicit configuration**: API keys and provider types via constructor
/// 3. **Runtime configuration**: Dynamic provider switching (future enhancement)
///
/// ## Thread Safety
///
/// The provider is thread-safe and can be shared across async tasks using `Arc<GenAIProvider>`.
/// The underlying genai client handles concurrent requests efficiently.
///
/// ## Error Handling
///
/// All methods return `Result<T>` with detailed error contexts. Network errors, authentication
/// failures, and API limits are properly categorized and can be handled appropriately by
/// the calling application.
pub struct GenAIProvider {
    /// The underlying genai client that handles provider-specific implementations.
    /// This client is configured during construction and handles authentication,
    /// request routing, and response processing for all supported providers.
    client: Client,
}

impl GenAIProvider {
    /// Creates a new GenAI provider with automatic configuration.
    ///
    /// This constructor creates a provider instance using the genai client's default
    /// configuration, which automatically detects and uses environment variables for
    /// authentication. This is the recommended approach for production use.
    ///
    /// ## Environment Variables
    ///
    /// The client will automatically detect and use these environment variables:
    /// - `OPENAI_API_KEY`: For OpenAI models
    /// - `ANTHROPIC_API_KEY`: For Anthropic Claude models
    /// - `GEMINI_API_KEY`: For Google Gemini models
    /// - `GROQ_API_KEY`: For Groq models
    /// - `COHERE_API_KEY`: For Cohere models
    /// - `XAI_API_KEY`: For XAI Grok models
    /// - `DEEPSEEK_API_KEY`: For DeepSeek models
    /// - (Ollama requires local setup, no API key needed)
    ///
    /// ## Returns
    ///
    /// * `Result<Self>` - A configured provider instance or configuration error
    ///
    /// ## Errors
    ///
    /// This method can fail if:
    /// - The genai client cannot be initialized
    /// - Required system dependencies are missing
    /// - Network configuration prevents client creation
    ///
    /// ## Example
    ///
    /// ```rust
    /// // Set environment variable first
    /// std::env::set_var("OPENAI_API_KEY", "sk-your-key");
    ///
    /// // Create provider with auto-configuration
    /// let provider = GenAIProvider::new()?;
    /// ```
    pub fn new() -> Result<Self> {
        let client = Client::default();
        Ok(Self { client })
    }

    /// Creates a new GenAI provider with explicit configuration.
    ///
    /// This constructor allows explicit specification of provider type and API key,
    /// which is useful for CLI applications, testing, or when configuration needs
    /// to be provided at runtime rather than through environment variables.
    ///
    /// The method automatically sets the appropriate environment variable based on
    /// the provider type before creating the genai client. This ensures compatibility
    /// with the genai crate's authentication system.
    ///
    /// ## Arguments
    ///
    /// * `provider_type` - Optional provider identifier (e.g., "openai", "anthropic")
    /// * `api_key` - Optional API key for authentication
    ///
    /// ## Provider Type Mapping
    ///
    /// The following provider types are supported:
    /// - `"openai"` → Sets `OPENAI_API_KEY`
    /// - `"anthropic"` → Sets `ANTHROPIC_API_KEY`
    /// - `"gemini"` → Sets `GEMINI_API_KEY`
    /// - `"groq"` → Sets `GROQ_API_KEY`
    /// - `"cohere"` → Sets `COHERE_API_KEY`
    /// - `"xai"` → Sets `XAI_API_KEY`
    /// - `"deepseek"` → Sets `DEEPSEEK_API_KEY`
    /// - `"ollama"` → No API key required (local setup)
    ///
    /// ## Returns
    ///
    /// * `Result<Self>` - A configured provider instance or configuration error
    ///
    /// ## Errors
    ///
    /// This method can fail if:
    /// - The genai client cannot be initialized
    /// - An unknown provider type is specified
    /// - System environment cannot be modified
    ///
    /// ## Example
    ///
    /// ```rust
    /// // Create provider with explicit configuration
    /// let provider = GenAIProvider::new_with_config(
    ///     Some("openai"),
    ///     Some("sk-your-api-key")
    /// )?;
    ///
    /// // Or use for testing with no authentication
    /// let provider = GenAIProvider::new_with_config(None, None)?;
    /// ```
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
                    return Ok(Self::new()?);
                }
                _ => {
                    log::warn!("Unknown provider type for API key: {}", provider);
                    return Ok(Self::new()?);
                }
            };

            log::info!("Setting {} environment variable for genai client", env_var);
            std::env::set_var(env_var, key);
        }

        let client = Client::default();
        Ok(Self { client })
    }

    /// Retrieves all available models for a specific provider.
    ///
    /// This method queries the specified provider's API to get a list of all available
    /// models that can be used for chat completion. The list includes both current and
    /// legacy models, allowing users to choose the most appropriate model for their needs.
    ///
    /// ## Arguments
    ///
    /// * `provider` - The provider identifier (e.g., "openai", "anthropic", "google")
    ///
    /// ## Provider Support
    ///
    /// Model listing is supported for:
    /// - **OpenAI**: GPT-4, GPT-3.5, GPT-4o, o1 series models
    /// - **Anthropic**: Claude 3/3.5 series (Opus, Sonnet, Haiku)
    /// - **Google**: Gemini Pro, Gemini 1.5/2.0 series
    /// - **Groq**: Llama 3.x series with various sizes
    /// - **Cohere**: Command R/R+ models
    /// - **XAI**: Grok models
    /// - **DeepSeek**: DeepSeek chat and reasoning models
    /// - **Ollama**: Requires local setup and running instance
    ///
    /// ## Returns
    ///
    /// * `Result<Vec<String>>` - List of model identifiers or error
    ///
    /// ## Errors
    ///
    /// This method can fail if:
    /// - The provider name is not recognized by genai
    /// - Network connectivity issues prevent API access
    /// - Authentication credentials are invalid or missing
    /// - The provider's API is temporarily unavailable
    /// - Rate limits are exceeded
    ///
    /// ## Example
    ///
    /// ```rust
    /// let provider = GenAIProvider::new()?;
    ///
    /// // Get OpenAI models
    /// let openai_models = provider.get_available_models("openai").await?;
    /// for model in openai_models {
    ///     println!("Available: {}", model);
    /// }
    ///
    /// // Get Anthropic models
    /// let claude_models = provider.get_available_models("anthropic").await?;
    /// ```
    pub async fn get_available_models(&self, provider: &str) -> Result<Vec<String>> {
        let adapter_kind = str_to_adapter_kind(provider)?;

        let models = self
            .client
            .all_model_names(adapter_kind)
            .await
            .context(format!("Failed to get models for provider: {}", provider))?;

        Ok(models)
    }

    /// Generates a simple text response without streaming.
    ///
    /// This method provides a straightforward way to get a complete response from an LLM
    /// without the complexity of streaming. It's ideal for simple Q&A scenarios, testing,
    /// or when the entire response is needed before processing.
    ///
    /// ## Arguments
    ///
    /// * `model` - The model identifier (e.g., "gpt-4o-mini", "claude-3-5-sonnet-20241022")
    /// * `prompt` - The user's message or prompt text
    ///
    /// ## Model Compatibility
    ///
    /// Supports all models available through the genai crate:
    /// - OpenAI: `gpt-4o`, `gpt-4o-mini`, `gpt-3.5-turbo`, `o1-mini`, `o1-preview`, `o3-mini`, `o4-mini`
    /// - Anthropic: `claude-3-5-sonnet-20241022`, `claude-3-opus-20240229`, etc.
    /// - Google: `gemini-1.5-pro`, `gemini-1.5-flash`, `gemini-2.0-flash`
    /// - Groq: `llama-3.1-70b-versatile`, `mixtral-8x7b-32768`, etc.
    /// - Cohere: `command-r`, `command-r-plus`, `command-light`
    /// - XAI: `grok-3-beta`, `grok-3-fast-beta`, etc.
    /// - DeepSeek: `deepseek-chat`, `deepseek-reasoner`
    /// - Ollama: Any locally installed model
    ///
    /// ## Returns
    ///
    /// * `Result<String>` - The complete response text or error
    ///
    /// ## Errors
    ///
    /// This method can fail if:
    /// - The model name is invalid or not available
    /// - Authentication fails (missing or invalid API key)
    /// - The prompt violates the provider's content policy
    /// - Network connectivity issues
    /// - The provider returns an error (rate limits, server errors, etc.)
    ///
    /// ## Example
    ///
    /// ```rust
    /// let provider = GenAIProvider::new_with_config(
    ///     Some("openai"),
    ///     Some("sk-your-key")
    /// )?;
    ///
    /// let response = provider.generate_response_simple(
    ///     "gpt-4o-mini",
    ///     "What is the capital of France?"
    /// ).await?;
    ///
    /// println!("AI: {}", response);
    /// ```
    pub async fn generate_response_simple(&self, model: &str, prompt: &str) -> Result<String> {
        let chat_req = ChatRequest::default().append_message(ChatMessage::user(prompt));

        log::debug!(
            "Sending chat request to model: {} with prompt: {}",
            model,
            prompt
        );

        let chat_res = self
            .client
            .exec_chat(model, chat_req, None)
            .await
            .context(format!(
                "Failed to execute chat request for model: {}",
                model
            ))?;

        let content = chat_res
            .content_text_as_str()
            .context("No text content in response")?;

        log::debug!("Received response with {} characters", content.len());
        Ok(content.to_string())
    }

    /// Generates a streaming response and sends chunks via mpsc channel.
    ///
    /// This is the core streaming method that provides real-time response generation,
    /// essential for creating responsive chat interfaces. It properly handles the genai
    /// crate's streaming events and manages the async communication with the UI layer.
    ///
    /// ## Streaming Architecture
    ///
    /// The method uses an async stream from the genai crate and processes different
    /// types of events:
    /// - **Start**: Indicates the beginning of response generation
    /// - **Chunk**: Contains incremental text content (main response text)
    /// - **ReasoningChunk**: Contains reasoning steps (for models like o1)
    /// - **End**: Indicates completion of response generation
    ///
    /// ## Arguments
    ///
    /// * `model` - The model identifier to use for generation
    /// * `prompt` - The user's input prompt or message
    /// * `tx` - Unbounded mpsc sender for streaming response chunks to the UI
    ///
    /// ## Channel Communication
    ///
    /// The method sends content chunks through the provided channel as they arrive.
    /// The receiving end (typically the UI) should listen for messages and handle:
    /// - Regular text chunks for incremental display
    /// - End-of-transmission signal (`EOT_SIGNAL`) indicating completion
    /// - Error messages prefixed with "Error: " for failure cases
    ///
    /// ## Event Processing
    ///
    /// 1. **ChatStreamEvent::Start** - Logs stream initiation, no content sent
    /// 2. **ChatStreamEvent::Chunk** - Sends content immediately to channel
    /// 3. **ChatStreamEvent::ReasoningChunk** - Logs reasoning (future: may send to channel)
    /// 4. **ChatStreamEvent::End** - Logs completion, caller should send EOT signal
    ///
    /// ## Error Handling
    ///
    /// Stream errors are handled gracefully:
    /// - Errors are logged with full context
    /// - Error messages are sent through the channel
    /// - The method returns the error for caller handling
    /// - Channel send failures are logged but don't halt processing
    ///
    /// ## Returns
    ///
    /// * `Result<()>` - Success (content sent via channel) or error details
    ///
    /// ## Errors
    ///
    /// This method can fail if:
    /// - Model is invalid or not available for the authenticated provider
    /// - Authentication credentials are missing or invalid
    /// - Network connectivity issues prevent streaming
    /// - The provider's streaming endpoint is unavailable
    /// - Content policy violations in the prompt
    /// - API rate limits or quota exceeded
    ///
    /// ## Example
    ///
    /// ```rust
    /// use tokio::sync::mpsc;
    /// use perspt::EOT_SIGNAL;
    ///
    /// let provider = GenAIProvider::new()?;
    /// let (tx, mut rx) = mpsc::unbounded_channel();
    ///
    /// // Start streaming in background task
    /// let provider_clone = provider.clone();
    /// tokio::spawn(async move {
    ///     match provider_clone.generate_response_stream_to_channel(
    ///         "gpt-4o-mini",
    ///         "Tell me about Rust programming",
    ///         tx.clone()
    ///     ).await {
    ///         Ok(()) => {
    ///             let _ = tx.send(EOT_SIGNAL.to_string());
    ///         }
    ///         Err(e) => {
    ///             let _ = tx.send(format!("Error: {}", e));
    ///             let _ = tx.send(EOT_SIGNAL.to_string());
    ///         }
    ///     }
    /// });
    ///
    /// // Receive and process chunks
    /// while let Some(chunk) = rx.recv().await {
    ///     if chunk == EOT_SIGNAL {
    ///         break;
    ///     } else if chunk.starts_with("Error: ") {
    ///         eprintln!("Stream error: {}", chunk);
    ///         break;
    ///     } else {
    ///         print!("{}", chunk); // Display incremental content
    ///     }
    /// }
    /// ```
    ///
    /// ## Performance Considerations
    ///
    /// - Uses async streams for efficient memory usage
    /// - Processes chunks immediately to minimize latency
    /// - Unbounded channels prevent backpressure but require careful memory management
    /// - Logs are at debug level to avoid performance impact in production
    pub async fn generate_response_stream_to_channel(
        &self,
        model: &str,
        prompt: &str,
        tx: mpsc::UnboundedSender<String>,
    ) -> Result<()> {
        let chat_req = ChatRequest::default().append_message(ChatMessage::user(prompt));

        log::debug!(
            "Sending streaming chat request to model: {} with prompt: {}",
            model,
            prompt
        );

        let chat_res_stream = self
            .client
            .exec_chat_stream(model, chat_req, None)
            .await
            .context(format!(
                "Failed to execute streaming chat request for model: {}",
                model
            ))?;

        // Process the stream with enhanced debugging to identify truncation issues
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

        // CRITICAL FIX: Only rely on genai's ChatStreamEvent signals, not custom timeouts
        // This prevents premature stream termination that causes content spillover
        while let Some(chunk_result) = stream.next().await {
            let elapsed = start_time.elapsed();

            match chunk_result {
                Ok(ChatStreamEvent::Start) => {
                    log::info!(">>> STREAM STARTED for model: {} at {:?}", model, elapsed);
                }
                Ok(ChatStreamEvent::Chunk(chunk)) => {
                    chunk_count += 1;
                    total_content_length += chunk.content.len();

                    // Enhanced logging: log every 10 chunks AND any large chunks
                    if chunk_count % 10 == 0 || chunk.content.len() > 100 {
                        log::info!("CHUNK #{}: {} chars, total: {} chars, elapsed: {:?}, content preview: '{}'", 
                                  chunk_count, chunk.content.len(), total_content_length, elapsed,
                                  chunk.content.chars().take(50).collect::<String>().replace('\n', "\\n"));
                    }

                    if !chunk.content.is_empty() {
                        // Send content immediately without batching to prevent content loss
                        if tx.send(chunk.content.clone()).is_err() {
                            log::error!(
                                "!!! CHANNEL SEND FAILED for chunk #{} - STOPPING STREAM !!!",
                                chunk_count
                            );
                            break;
                        }

                        // Additional detailed logging every 25 chunks
                        if chunk_count % 25 == 0 {
                            log::info!(
                                "=== PROGRESS: {} chunks, {} chars, {:?} elapsed ===",
                                chunk_count,
                                total_content_length,
                                elapsed
                            );
                        }
                    } else {
                        log::debug!("Empty chunk #{} received", chunk_count);
                    }
                }
                Ok(ChatStreamEvent::ReasoningChunk(chunk)) => {
                    log::info!(
                        "REASONING CHUNK: {} chars at {:?}",
                        chunk.content.len(),
                        elapsed
                    );
                    // For now, just log reasoning chunks. In future versions we might display them differently.
                }
                Ok(ChatStreamEvent::End(_)) => {
                    log::info!(">>> STREAM ENDED EXPLICITLY for model: {} after {} chunks, {} chars, {:?} elapsed", 
                               model, chunk_count, total_content_length, elapsed);
                    stream_ended_explicitly = true;
                    break;
                }
                Err(e) => {
                    log::error!(
                        "!!! STREAM ERROR after {} chunks at {:?}: {} !!!",
                        chunk_count,
                        elapsed,
                        e
                    );
                    let error_msg = format!("Stream error: {}", e);
                    let _ = tx.send(error_msg);
                    return Err(e.into());
                }
            }
        }

        // Stream ended - either explicitly via End event or stream exhaustion
        let final_elapsed = start_time.elapsed();
        if !stream_ended_explicitly {
            log::warn!("!!! STREAM ENDED IMPLICITLY (exhausted) for model: {} after {} chunks, {} chars, {:?} elapsed !!!", 
                       model, chunk_count, total_content_length, final_elapsed);
        }

        log::info!(
            "=== STREAM COMPLETE === Model: {}, Final: {} chunks, {} chars, {:?} elapsed",
            model,
            chunk_count,
            total_content_length,
            final_elapsed
        );

        // CRITICAL: Send single EOT signal to indicate completion
        // Remove duplicate/backup EOT signals that can cause confusion in the UI
        if tx.send(crate::EOT_SIGNAL.to_string()).is_err() {
            log::error!("!!! FAILED TO SEND EOT SIGNAL - channel may be closed !!!");
            return Err(anyhow::anyhow!("Channel closed during EOT signal send"));
        }

        log::info!(">>> EOT SIGNAL SENT for model: {} <<<", model);
        Ok(())
    }

    /// Generate response with conversation history
    pub async fn generate_response_with_history(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<String> {
        let chat_req = ChatRequest::new(messages);

        log::debug!(
            "Sending chat request to model: {} with conversation history",
            model
        );

        let chat_res = self
            .client
            .exec_chat(model, chat_req, None)
            .await
            .context(format!(
                "Failed to execute chat request for model: {}",
                model
            ))?;

        let content = chat_res
            .content_text_as_str()
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
        let chat_req = ChatRequest::default().append_message(ChatMessage::user(prompt));

        log::debug!(
            "Sending chat request to model: {} with custom options",
            model
        );

        let chat_res = self
            .client
            .exec_chat(model, chat_req, Some(&options))
            .await
            .context(format!(
                "Failed to execute chat request for model: {}",
                model
            ))?;

        let content = chat_res
            .content_text_as_str()
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
        ]
    }

    /// Get all available providers (for compatibility with main.rs)
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
                log::info!("Model {} is available and working", model);
                Ok(true)
            }
            Err(e) => {
                log::warn!("Model {} test failed: {}", model, e);
                Ok(false)
            }
        }
    }

    /// Validate and get the best available model for a provider
    pub async fn validate_model(&self, model: &str, provider_type: Option<&str>) -> Result<String> {
        // First try the specified model
        if self.test_model(model).await? {
            return Ok(model.to_string());
        }

        // If that fails, try to get available models and find a suitable one
        if let Some(provider) = provider_type {
            if let Ok(models) = self.get_available_models(provider).await {
                if !models.is_empty() {
                    log::info!("Model {} not available, using {} instead", model, models[0]);
                    return Ok(models[0].clone());
                }
            }
        }

        // If all else fails, use the original model and let it fail with a proper error
        log::warn!("Could not validate model {}, proceeding anyway", model);
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
}
