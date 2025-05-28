// src/llm_provider.rs
use async_trait::async_trait;
use tokio::sync::mpsc;
use anyhow::Result;
use allms::{Completions, llm_models::{OpenAIModels, AnthropicModels, GoogleModels, MistralModels, PerplexityModels, DeepSeekModels, AwsBedrockModels}};
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

/// Unified LLM provider using allms crate
#[derive(Debug)]
pub struct UnifiedLLMProvider {
    provider_type: ProviderType,
}

impl UnifiedLLMProvider {
    pub fn new(provider_type: ProviderType) -> Self {
        Self { provider_type }
    }

    /// Get available models for the provider type
    pub fn get_available_models(&self) -> Vec<String> {
        match self.provider_type {
            ProviderType::OpenAI => vec![
                "gpt-4".to_string(),
                "gpt-4-turbo".to_string(),
                "gpt-3.5-turbo".to_string(),
                "gpt-4o".to_string(),
                "gpt-4o-mini".to_string(),
                "gpt-4.1-mini".to_string(),
            ],
            ProviderType::Anthropic => vec![
                "claude-3-opus-20240229".to_string(),
                "claude-3-sonnet-20240229".to_string(),
                "claude-3-haiku-20240307".to_string(),
                "claude-3-5-sonnet-20241022".to_string(),
                "claude-3-5-haiku-latest".to_string(),
            ],
            ProviderType::Google => vec![
                "gemini-1.5-pro".to_string(),
                "gemini-1.5-flash".to_string(),
                "gemini-1.5-flash-8b".to_string(),
                "gemini-2.0-flash".to_string(),
                "gemini-2.0-flash-lite".to_string(),
            ],
            ProviderType::Mistral => vec![
                "mistral-tiny".to_string(),
                "mistral-small".to_string(),
                "mistral-medium".to_string(),
                "mistral-large".to_string(),
                "open-mistral-nemo".to_string(),
            ],
            ProviderType::Perplexity => vec![
                "sonar".to_string(),
                "sonar-pro".to_string(),
                "sonar-reasoning".to_string(),
            ],
            ProviderType::DeepSeek => vec![
                "deepseek-chat".to_string(),
                "deepseek-coder".to_string(),
                "deepseek-reasoner".to_string(),
            ],
            ProviderType::AwsBedrock => vec![
                "anthropic.claude-v2".to_string(),
                "anthropic.claude-3-sonnet-20240229-v1:0".to_string(),
                "amazon.titan-text-express-v1".to_string(),
                "amazon.nova-lite-v1:0".to_string(),
            ],
        }
    }

    /// Create a Completions instance for the given model and get response
    async fn get_completion_response(&self, model: &str, api_key: &str, prompt: &str) -> LLMResult<String> {
        let provider_type = self.provider_type.clone();
        let model = model.to_string();
        let api_key = api_key.to_string();
        let prompt = prompt.to_string();
        
        // Use spawn_blocking to handle the non-Send future from allms
        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                match provider_type {
                    ProviderType::OpenAI => {
                        let model_enum = match model.as_str() {
                            "gpt-4" => OpenAIModels::Gpt4,
                            "gpt-4-turbo" => OpenAIModels::Gpt4Turbo,
                            "gpt-3.5-turbo" => OpenAIModels::Gpt3_5Turbo,
                            "gpt-4o" => OpenAIModels::Gpt4o,
                            "gpt-4o-mini" => OpenAIModels::Gpt4oMini,
                            "gpt-4.1-mini" => OpenAIModels::Gpt4_1Mini,
                            _ => OpenAIModels::Gpt3_5Turbo,
                        };
                        let completions = Completions::new(model_enum, &api_key, None, None);
                        
                        completions.get_answer::<String>(&prompt).await
                            .map_err(|e| anyhow::anyhow!("OpenAI API error: {}", e))
                    },
                    ProviderType::Anthropic => {
                        let model_enum = match model.as_str() {
                            "claude-3-opus-latest" => AnthropicModels::Claude3Opus,
                            "claude-3-sonnet-20240229" => AnthropicModels::Claude3Sonnet,
                            "claude-3-haiku-20240307" => AnthropicModels::Claude3Haiku,
                            "claude-3-5-sonnet-latest" => AnthropicModels::Claude3_5Sonnet,
                            "claude-3-5-haiku-latest" => AnthropicModels::Claude3_5Haiku,
                            "claude-3-7-sonnet-latest" => AnthropicModels::Claude3_7Sonnet,
                            _ => AnthropicModels::Claude3_5Sonnet,
                        };
                        let completions = Completions::new(model_enum, &api_key, None, None);
                        
                        completions.get_answer::<String>(&prompt).await
                            .map_err(|e| anyhow::anyhow!("Anthropic API error: {}", e))
                    },
                    ProviderType::Google => {
                        let model_enum = match model.as_str() {
                            "gemini-1.5-pro" => GoogleModels::Gemini1_5Pro,
                            "gemini-1.5-flash" => GoogleModels::Gemini1_5Flash,
                            "gemini-1.5-flash-8b" => GoogleModels::Gemini1_5Flash8B,
                            "gemini-2.0-flash" => GoogleModels::Gemini2_0Flash,
                            "gemini-2.0-flash-lite" => GoogleModels::Gemini2_0FlashLite,
                            _ => GoogleModels::Gemini1_5Flash,
                        };
                        let completions = Completions::new(model_enum, &api_key, None, None);
                        
                        completions.get_answer::<String>(&prompt).await
                            .map_err(|e| anyhow::anyhow!("Google API error: {}", e))
                    },
                    ProviderType::Mistral => {
                        let model_enum = match model.as_str() {
                            "mistral-tiny" => MistralModels::MistralTiny,
                            "mistral-small" => MistralModels::MistralSmall,
                            "mistral-medium" => MistralModels::MistralMedium,
                            "mistral-large-latest" => MistralModels::MistralLarge,
                            "open-mistral-nemo" => MistralModels::MistralNemo,
                            "open-mistral-7b" => MistralModels::Mistral7B,
                            "open-mixtral-8x7b" => MistralModels::Mixtral8x7B,
                            "open-mixtral-8x22b" => MistralModels::Mixtral8x22B,
                            _ => MistralModels::MistralSmall,
                        };
                        let completions = Completions::new(model_enum, &api_key, None, None);
                        
                        completions.get_answer::<String>(&prompt).await
                            .map_err(|e| anyhow::anyhow!("Mistral API error: {}", e))
                    },
                    ProviderType::Perplexity => {
                        let model_enum = match model.as_str() {
                            "sonar" => PerplexityModels::Sonar,
                            "sonar-pro" => PerplexityModels::SonarPro,
                            "sonar-reasoning" => PerplexityModels::SonarReasoning,
                            _ => PerplexityModels::Sonar,
                        };
                        let completions = Completions::new(model_enum, &api_key, None, None);
                        
                        completions.get_answer::<String>(&prompt).await
                            .map_err(|e| anyhow::anyhow!("Perplexity API error: {}", e))
                    },
                    ProviderType::DeepSeek => {
                        let model_enum = match model.as_str() {
                            "deepseek-chat" => DeepSeekModels::DeepSeekChat,
                            "deepseek-reasoner" => DeepSeekModels::DeepSeekReasoner,
                            _ => DeepSeekModels::DeepSeekChat,
                        };
                        let completions = Completions::new(model_enum, &api_key, None, None);
                        
                        completions.get_answer::<String>(&prompt).await
                            .map_err(|e| anyhow::anyhow!("DeepSeek API error: {}", e))
                    },
                    ProviderType::AwsBedrock => {
                        let model_enum = match model.as_str() {
                            "amazon.nova-pro-v1:0" => AwsBedrockModels::NovaPro,
                            "amazon.nova-lite-v1:0" => AwsBedrockModels::NovaLite,
                            "amazon.nova-micro-v1:0" => AwsBedrockModels::NovaMicro,
                            _ => AwsBedrockModels::NovaLite,
                        };
                        let completions = Completions::new(model_enum, "", None, None); // AWS Bedrock uses different auth
                        
                        completions.get_answer::<String>(&prompt).await
                            .map_err(|e| anyhow::anyhow!("AWS Bedrock API error: {}", e))
                    },
                }
            })
        }).await?
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
            ProviderType::AwsBedrock => Ok(()), // AWS uses different auth validation
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
