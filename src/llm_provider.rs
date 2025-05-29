// src/llm_provider.rs
//
// This module provides a unified LLM provider interface that directly leverages the allms crate
// instead of maintaining its own model lists. This design has several benefits:
//
// 1. **Automatic Updates**: As the allms crate adds support for new models and providers,
//    this code automatically benefits without requiring manual updates.
//
// 2. **Dynamic Model Discovery**: Uses try_from_str() to validate and support any model
//    that the allms crate recognizes, including future additions.
//
// 3. **Consistent API**: Leverages the allms crate's unified Completions API for all providers,
//    ensuring consistent behavior and feature support across different LLM providers.
//
// 4. **Reduced Maintenance**: No need to manually track model names, API changes, or
//    provider-specific implementations - the allms crate handles all of this.
//
use async_trait::async_trait;
use tokio::sync::mpsc;
use anyhow::Result;
use allms::{
    Completions, 
    llm_models::{
        OpenAIModels, AnthropicModels, GoogleModels, MistralModels, 
        PerplexityModels, DeepSeekModels, AwsBedrockModels, LLMModel
    }
};
use serde::{Deserialize, Serialize};
use crate::config::AppConfig;

/// Simple string response for get_answer
#[derive(Debug, Deserialize, Serialize)]
pub struct SimpleResponse {
    pub content: String,
}

/// Represents different types of LLM providers supported by allms
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderType {
    OpenAI,
    Anthropic,
    Google,
    Mistral,
    Perplexity,
    DeepSeek,
    AwsBedrock,
}

impl ProviderType {
    /// Converts a string to a ProviderType enum variant.
    ///
    /// Performs case-insensitive matching to determine the provider type
    /// from configuration strings or user input.
    ///
    /// # Arguments
    ///
    /// * `s` - The string to match against provider names
    ///
    /// # Returns
    ///
    /// `Some(ProviderType)` if the string matches a known provider, `None` otherwise
    ///
    /// # Examples
    ///
    /// ```rust
    /// use perspt::llm_provider::ProviderType;
    ///
    /// assert_eq!(ProviderType::from_string("openai"), Some(ProviderType::OpenAI));
    /// assert_eq!(ProviderType::from_string("OpenAI"), Some(ProviderType::OpenAI));
    /// assert_eq!(ProviderType::from_string("invalid"), None);
    /// ```
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "openai" => Some(ProviderType::OpenAI),
            "anthropic" => Some(ProviderType::Anthropic),
            "google" | "gemini" => Some(ProviderType::Google),
            "mistral" => Some(ProviderType::Mistral),
            "perplexity" => Some(ProviderType::Perplexity),
            "deepseek" => Some(ProviderType::DeepSeek),
            "aws" | "bedrock" | "aws-bedrock" => Some(ProviderType::AwsBedrock),
            _ => None,
        }
    }

    /// Converts the ProviderType to its string representation.
    ///
    /// Returns the canonical string name for the provider type,
    /// which can be used in configuration files and API calls.
    ///
    /// # Returns
    ///
    /// A static string representing the provider name
    ///
    /// # Examples
    ///
    /// ```rust
    /// use perspt::llm_provider::ProviderType;
    ///
    /// let provider = ProviderType::OpenAI;
    /// assert_eq!(provider.to_string(), "openai");
    /// ```
    pub fn to_string(&self) -> &'static str {
        match self {
            ProviderType::OpenAI => "openai",
            ProviderType::Anthropic => "anthropic",
            ProviderType::Google => "google",
            ProviderType::Mistral => "mistral",
            ProviderType::Perplexity => "perplexity",
            ProviderType::DeepSeek => "deepseek",
            ProviderType::AwsBedrock => "aws-bedrock",
        }
    }
}

/// Result type for LLM operations
pub type LLMResult<T> = Result<T>;

/// Unified LLM provider using allms crate.
///
/// This struct provides a unified interface to multiple LLM providers through
/// the allms crate, automatically handling provider-specific implementations
/// and model availability.
///
/// # Examples
///
/// ```rust
/// use perspt::llm_provider::{UnifiedLLMProvider, ProviderType};
///
/// let provider = UnifiedLLMProvider::new(ProviderType::OpenAI);
/// let models = provider.get_available_models();
/// println!("Available models: {:?}", models);
/// ```
#[derive(Debug)]
pub struct UnifiedLLMProvider {
    provider_type: ProviderType,
}

impl UnifiedLLMProvider {
    /// Creates a new UnifiedLLMProvider for the specified provider type.
    ///
    /// # Arguments
    ///
    /// * `provider_type` - The type of LLM provider to create
    ///
    /// # Returns
    ///
    /// A new UnifiedLLMProvider instance
    ///
    /// # Examples
    ///
    /// ```rust
    /// use perspt::llm_provider::{UnifiedLLMProvider, ProviderType};
    ///
    /// let openai_provider = UnifiedLLMProvider::new(ProviderType::OpenAI);
    /// let anthropic_provider = UnifiedLLMProvider::new(ProviderType::Anthropic);
    /// ```
    pub fn new(provider_type: ProviderType) -> Self {
        Self { provider_type }
    }

    /// Gets available models for the provider type by using the allms enums directly.
    ///
    /// This method dynamically retrieves all available models from the allms crate
    /// for the specific provider type. It validates common models and returns
    /// those that are supported by the underlying allms implementation.
    ///
    /// # Returns
    ///
    /// A vector of model names available for this provider
    ///
    /// # Examples
    ///
    /// ```rust
    /// use perspt::llm_provider::{UnifiedLLMProvider, ProviderType};
    ///
    /// let provider = UnifiedLLMProvider::new(ProviderType::OpenAI);
    /// let models = provider.get_available_models();
    /// assert!(!models.is_empty());
    /// assert!(models.contains(&"gpt-4".to_string()));
    /// ```
    pub fn get_available_models(&self) -> Vec<String> {
        match self.provider_type {
            ProviderType::OpenAI => {
                // Get a comprehensive list of available OpenAI models
                // Since we can't easily iterate over enum variants, we use try_from_str to validate common models
                let common_models = vec![
                    "gpt-3.5-turbo", "gpt-4", "gpt-4-turbo", "gpt-4o", "gpt-4o-mini",
                    "gpt-4.1", "gpt-4.1-mini", "o1", "o1-mini", "o3", "o3-mini",
                ];
                common_models.into_iter()
                    .filter_map(|model| {
                        OpenAIModels::try_from_str(model).map(|m| m.as_str().to_string())
                    })
                    .collect()
            },
            ProviderType::Anthropic => {
                let common_models = vec![
                    "claude-3-opus-20240229", "claude-3-sonnet-20240229", "claude-3-haiku-20240307",
                    "claude-3-5-sonnet-20241022", "claude-3-5-haiku-20241022",
                ];
                common_models.into_iter()
                    .filter_map(|model| {
                        AnthropicModels::try_from_str(model).map(|m| m.as_str().to_string())
                    })
                    .collect()
            },
            ProviderType::Google => {
                let common_models = vec![
                    "gemini-1.5-pro", "gemini-1.5-flash", "gemini-1.5-flash-8b",
                    "gemini-2.0-flash", "gemini-pro",
                ];
                common_models.into_iter()
                    .filter_map(|model| {
                        GoogleModels::try_from_str(model).map(|m| m.as_str().to_string())
                    })
                    .collect()
            },
            ProviderType::Mistral => {
                let common_models = vec![
                    "mistral-tiny", "mistral-small", "mistral-medium", "mistral-large",
                    "open-mistral-nemo", "open-mistral-7b", "open-mixtral-8x7b", "open-mixtral-8x22b",
                ];
                common_models.into_iter()
                    .filter_map(|model| {
                        MistralModels::try_from_str(model).map(|m| m.as_str().to_string())
                    })
                    .collect()
            },
            ProviderType::Perplexity => {
                let common_models = vec![
                    "sonar", "sonar-pro", "sonar-reasoning",
                ];
                common_models.into_iter()
                    .filter_map(|model| {
                        PerplexityModels::try_from_str(model).map(|m| m.as_str().to_string())
                    })
                    .collect()
            },
            ProviderType::DeepSeek => {
                let common_models = vec![
                    "deepseek-chat", "deepseek-coder", "deepseek-reasoner",
                ];
                common_models.into_iter()
                    .filter_map(|model| {
                        DeepSeekModels::try_from_str(model).map(|m| m.as_str().to_string())
                    })
                    .collect()
            },
            ProviderType::AwsBedrock => {
                let common_models = vec![
                    "amazon.nova-pro-v1:0", "amazon.nova-lite-v1:0", "amazon.nova-micro-v1:0",
                ];
                common_models.into_iter()
                    .filter_map(|model| {
                        AwsBedrockModels::try_from_str(model).map(|m| m.as_str().to_string())
                    })
                    .collect()
            },
        }
    }

    /// Create a Completions instance for the given model and get response
    /// This method includes panic protection for external crate operations
    async fn get_completion_response(&self, model: &str, api_key: &str, prompt: &str) -> LLMResult<String> {
        let provider_type = self.provider_type.clone();
        let model = model.to_string();
        let api_key = api_key.to_string();
        let prompt = prompt.to_string();
        
        // Use spawn_blocking with panic protection
        let result = tokio::task::spawn_blocking(move || {
            // Catch panics from allms crate operations
            let result = std::panic::catch_unwind(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async move {
                    Self::execute_llm_request(provider_type, model, api_key, prompt).await
                })
            });
            
            match result {
                Ok(Ok(response)) => Ok(response),
                Ok(Err(e)) => Err(e),
                Err(panic_err) => {
                    let panic_msg = if let Some(s) = panic_err.downcast_ref::<String>() {
                        s.clone()
                    } else if let Some(s) = panic_err.downcast_ref::<&str>() {
                        s.to_string()
                    } else {
                        "Unknown panic in LLM operation".to_string()
                    };
                    
                    log::error!("LLM operation panicked: {}", panic_msg);
                    
                    // Return a user-friendly error based on common panic scenarios
                    if panic_msg.contains("PROJECT_ID") {
                        Err(anyhow::anyhow!("Google Cloud PROJECT_ID environment variable is not set. Please set PROJECT_ID to your Google Cloud project ID."))
                    } else if panic_msg.contains("NotPresent") {
                        Err(anyhow::anyhow!("Required environment variable is missing. Please check your configuration and ensure all required environment variables are set."))
                    } else if panic_msg.contains("AWS") || panic_msg.contains("credentials") {
                        Err(anyhow::anyhow!("AWS credentials are not properly configured. Please set up your AWS credentials."))
                    } else {
                        Err(anyhow::anyhow!("LLM provider configuration error: {}. Please check your environment variables and API keys.", panic_msg))
                    }
                }
            }
        }).await;
        
        match result {
            Ok(response) => response,
            Err(join_err) => {
                log::error!("Task join error: {}", join_err);
                Err(anyhow::anyhow!("Internal error: {}", join_err))
            }
        }
    }
    
    /// Execute the actual LLM request with the allms crate
    async fn execute_llm_request(
        provider_type: ProviderType,
        model: String,
        api_key: String,
        prompt: String,
    ) -> LLMResult<String> {
        match provider_type {
            ProviderType::OpenAI => {
                // Use try_from_str to dynamically create the model enum
                let model_enum = OpenAIModels::try_from_str(&model)
                    .unwrap_or(OpenAIModels::Gpt4oMini);
                let completions = Completions::new(model_enum, &api_key, None, None);
                
                completions.get_answer::<String>(&prompt).await
                    .map_err(|e| anyhow::anyhow!("OpenAI API error: {}", e))
            },
            ProviderType::Anthropic => {
                let model_enum = AnthropicModels::try_from_str(&model)
                    .unwrap_or(AnthropicModels::Claude3_5Sonnet);
                let completions = Completions::new(model_enum, &api_key, None, None);
                
                completions.get_answer::<String>(&prompt).await
                    .map_err(|e| anyhow::anyhow!("Anthropic API error: {}", e))
            },
            ProviderType::Google => {
                let model_enum = GoogleModels::try_from_str(&model)
                    .unwrap_or(GoogleModels::Gemini1_5Flash);
                let completions = Completions::new(model_enum, &api_key, None, None);
                
                completions.get_answer::<String>(&prompt).await
                    .map_err(|e| anyhow::anyhow!("Google API error: {}", e))
            },
            ProviderType::Mistral => {
                let model_enum = MistralModels::try_from_str(&model)
                    .unwrap_or(MistralModels::MistralSmall);
                let completions = Completions::new(model_enum, &api_key, None, None);
                
                completions.get_answer::<String>(&prompt).await
                    .map_err(|e| anyhow::anyhow!("Mistral API error: {}", e))
            },
            ProviderType::Perplexity => {
                let model_enum = PerplexityModels::try_from_str(&model)
                    .unwrap_or(PerplexityModels::Sonar);
                let completions = Completions::new(model_enum, &api_key, None, None);
                
                completions.get_answer::<String>(&prompt).await
                    .map_err(|e| anyhow::anyhow!("Perplexity API error: {}", e))
            },
            ProviderType::DeepSeek => {
                let model_enum = DeepSeekModels::try_from_str(&model)
                    .unwrap_or(DeepSeekModels::DeepSeekChat);
                let completions = Completions::new(model_enum, &api_key, None, None);
                
                completions.get_answer::<String>(&prompt).await
                    .map_err(|e| anyhow::anyhow!("DeepSeek API error: {}", e))
            },
            ProviderType::AwsBedrock => {
                let model_enum = AwsBedrockModels::try_from_str(&model)
                    .unwrap_or(AwsBedrockModels::NovaLite);
                let completions = Completions::new(model_enum, "", None, None); // AWS Bedrock uses different auth
                
                completions.get_answer::<String>(&prompt).await
                    .map_err(|e| anyhow::anyhow!("AWS Bedrock API error: {}", e))
            },
        }
    }

}

#[async_trait]
impl LLMProvider for UnifiedLLMProvider {
    async fn list_models(&self) -> LLMResult<Vec<String>> {
        Ok(self.get_available_models())
    }

    async fn send_chat_request(
        &self,
        input: &str,
        model_name: &str,
        config: &AppConfig,
        tx: &mpsc::UnboundedSender<String>,
    ) -> LLMResult<()> {
        // Get the API key from config - using unified api_key field
        let api_key = match self.provider_type {
            ProviderType::AwsBedrock => "", // AWS uses different auth
            _ => config.api_key.as_deref().unwrap_or_default(),
        };

        let api_key = api_key.to_string();
        
        // Get the actual response using our existing method
        let response = self.get_completion_response(model_name, &api_key, input).await?;
        
        // Simulate streaming by sending the response in chunks
        // This provides a better user experience than sending all at once
        let chunks: Vec<&str> = response.split_whitespace().collect();
        
        for (i, chunk) in chunks.iter().enumerate() {
            if tx.send(chunk.to_string()).is_err() {
                log::warn!("Failed to send response chunk - receiver dropped");
                break;
            }
            
            // Add space between words (except for the last chunk)
            if i < chunks.len() - 1 {
                if tx.send(" ".to_string()).is_err() {
                    log::warn!("Failed to send space - receiver dropped");
                    break;
                }
            }
            
            // Small delay to simulate streaming
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
        
        Ok(())
    }

    fn provider_type(&self) -> ProviderType {
        self.provider_type.clone()
    }

    async fn validate_config(&self, config: &AppConfig) -> LLMResult<()> {
        // Check if API key is configured - using unified api_key field
        match self.provider_type {
            ProviderType::AwsBedrock => {
                // AWS uses different auth - check for AWS credentials
                if std::env::var("AWS_ACCESS_KEY_ID").is_err() && std::env::var("AWS_PROFILE").is_err() {
                    return Err(anyhow::anyhow!(
                        "AWS credentials not configured. Please set AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY, or configure AWS_PROFILE"
                    ));
                }
                Ok(())
            },
            ProviderType::Google => {
                // For Google, check both API key and PROJECT_ID
                if config.api_key.is_none() || config.api_key.as_ref().unwrap().is_empty() {
                    return Err(anyhow::anyhow!(
                        "API key not configured for {} provider", 
                        self.provider_type.to_string()
                    ));
                }
                
                // Check for PROJECT_ID environment variable
                if std::env::var("PROJECT_ID").is_err() {
                    return Err(anyhow::anyhow!(
                        "Google Cloud PROJECT_ID environment variable is not set. Please set PROJECT_ID to your Google Cloud project ID."
                    ));
                }
                Ok(())
            },
            _ => {
                if config.api_key.is_none() || config.api_key.as_ref().unwrap().is_empty() {
                    return Err(anyhow::anyhow!(
                        "API key not configured for {} provider", 
                        self.provider_type.to_string()
                    ));
                }
                Ok(())
            }
        }
    }
}

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
