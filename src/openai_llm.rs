// src/openai_llm.rs
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::{header, Client, Response};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::config::AppConfig;
use crate::llm_provider::{LLMProvider, LLMResult, ProviderType};

/// OpenAI API provider with modern async implementation
pub struct OpenAIProviderLlm {
    client: Client,
}

impl OpenAIProviderLlm {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300)) // 5 minute timeout
            .build()
            .expect("Failed to create HTTP client");
        
        Self { client }
    }

    /// Parse streaming SSE response from OpenAI
    async fn stream_response(
        &self,
        response: Response,
        tx: &mpsc::UnboundedSender<String>,
    ) -> LLMResult<()> {
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            let chunk_str = String::from_utf8_lossy(&chunk);
            buffer.push_str(&chunk_str);

            // Process complete lines
            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim().to_string();
                buffer.drain(..=line_end);

                if line.is_empty() || !line.starts_with("data: ") {
                    continue;
                }

                let data = &line[6..]; // Remove "data: " prefix
                
                if data == "[DONE]" {
                    log::info!("OpenAI stream completed");
                    return Ok(());
                }

                // Parse JSON response
                match serde_json::from_str::<Value>(data) {
                    Ok(json) => {
                        if let Some(choices) = json.get("choices").and_then(|c| c.as_array()) {
                            if let Some(choice) = choices.first() {
                                if let Some(delta) = choice.get("delta") {
                                    if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                        if let Err(e) = tx.send(content.to_string()) {
                                            log::error!("Failed to send token: {}", e);
                                            return Err(anyhow::anyhow!("Channel send failed"));
                                        }
                                    }
                                }
                            }
                        }
                        
                        // Check for errors in the response
                        if let Some(error) = json.get("error") {
                            let error_msg = error.get("message")
                                .and_then(|m| m.as_str())
                                .unwrap_or("Unknown OpenAI error");
                            return Err(anyhow::anyhow!("OpenAI API error: {}", error_msg));
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to parse JSON chunk: {} - Error: {}", data, e);
                        // Continue processing other chunks
                    }
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl LLMProvider for OpenAIProviderLlm {
    async fn list_models(&self) -> LLMResult<Vec<String>> {
        // Return commonly available OpenAI models
        Ok(vec![
            "gpt-4o".to_string(),
            "gpt-4o-mini".to_string(),
            "gpt-4-turbo".to_string(),
            "gpt-4".to_string(),
            "gpt-3.5-turbo".to_string(),
        ])
    }

    async fn send_chat_request(
        &self,
        input: &str,
        model_name: &str,
        config: &AppConfig,
        tx: &mpsc::UnboundedSender<String>,
    ) -> LLMResult<()> {
        // Get API key
        let api_key = config.api_key.as_ref()
            .ok_or_else(|| anyhow::anyhow!("API key not provided for OpenAI"))?;

        // Get base URL
        let base_url = config.providers.get("openai")
            .ok_or_else(|| anyhow::anyhow!("OpenAI provider URL not configured"))?;

        let request_url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
        
        log::info!("OpenAI request: {} with model {}", request_url, model_name);

        // Create request payload
        let payload = json!({
            "model": model_name,
            "messages": [
                {"role": "user", "content": input}
            ],
            "stream": true,
            "max_tokens": 2048,
            "temperature": 0.7,
            "top_p": 0.9,
        });

        // Send request
        let response = self.client
            .post(&request_url)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {}", api_key))
            .json(&payload)
            .send()
            .await?;

        let status = response.status();
        log::info!("OpenAI response status: {}", status);

        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            let error_msg = format!("OpenAI API error: {} - {}", status, error_body);
            log::error!("{}", error_msg);
            let _ = tx.send(format!("Error: {}", error_msg));
            return Err(anyhow::anyhow!(error_msg));
        }

        // Stream the response
        let result = self.stream_response(response, tx).await;

        // Always send EOT signal
        if let Err(e) = tx.send(crate::EOT_SIGNAL.to_string()) {
            log::error!("Failed to send EOT signal: {}", e);
        }

        result
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::OpenAI
    }

    async fn validate_config(&self, config: &AppConfig) -> LLMResult<()> {
        if config.api_key.is_none() {
            return Err(anyhow::anyhow!("API key is required for OpenAI provider"));
        }

        if !config.providers.contains_key("openai") {
            return Err(anyhow::anyhow!("OpenAI provider URL not configured"));
        }

        Ok(())
    }
}
