// src/gemini_llm.rs
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::{header, Client, Response};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::config::AppConfig;
use crate::llm_provider::{LLMProvider, LLMResult, ProviderType};

/// Google Gemini API provider with modern async implementation
pub struct GeminiProviderLlm {
    client: Client,
}

impl GeminiProviderLlm {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300)) // 5 minute timeout
            .build()
            .expect("Failed to create HTTP client");
        
        Self { client }
    }

    /// Parse streaming SSE response from Gemini
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
                
                // Parse JSON response
                match serde_json::from_str::<Value>(data) {
                    Ok(json) => {
                        if let Some(candidates) = json.get("candidates").and_then(|c| c.as_array()) {
                            if let Some(candidate) = candidates.first() {
                                if let Some(content) = candidate.get("content") {
                                    if let Some(parts) = content.get("parts").and_then(|p| p.as_array()) {
                                        if let Some(part) = parts.first() {
                                            if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                                                if let Err(e) = tx.send(text.to_string()) {
                                                    log::error!("Failed to send token: {}", e);
                                                    return Err(anyhow::anyhow!("Channel send failed"));
                                                }
                                            }
                                        }
                                    }
                                }
                                
                                // Check if generation is finished
                                if let Some(finish_reason) = candidate.get("finishReason") {
                                    log::info!("Gemini generation finished: {}", finish_reason);
                                    return Ok(());
                                }
                            }
                        }
                        
                        // Check for errors in the response
                        if let Some(error) = json.get("error") {
                            let error_msg = error.get("message")
                                .and_then(|m| m.as_str())
                                .unwrap_or("Unknown Gemini error");
                            return Err(anyhow::anyhow!("Gemini API error: {}", error_msg));
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
impl LLMProvider for GeminiProviderLlm {
    async fn list_models(&self) -> LLMResult<Vec<String>> {
        // Return commonly available Gemini models
        Ok(vec![
            "gemini-1.5-pro".to_string(),
            "gemini-1.5-flash".to_string(),
            "gemini-pro".to_string(),
            "gemini-pro-vision".to_string(),
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
            .ok_or_else(|| anyhow::anyhow!("API key not provided for Gemini"))?;

        // Get base URL
        let base_url = config.providers.get("gemini")
            .ok_or_else(|| anyhow::anyhow!("Gemini provider URL not configured"))?;

        let request_url = format!(
            "{}/models/{}:streamGenerateContent?alt=sse",
            base_url.trim_end_matches('/'),
            model_name
        );
        
        log::info!("Gemini request: {} with model {}", request_url, model_name);

        // Create request payload
        let payload = json!({
            "contents": [{
                "parts": [{"text": input}]
            }],
            "generationConfig": {
                "temperature": 0.7,
                "topP": 0.9,
                "topK": 40,
                "maxOutputTokens": 2048,
                "candidateCount": 1
            },
            "safetySettings": [
                {
                    "category": "HARM_CATEGORY_HARASSMENT",
                    "threshold": "BLOCK_MEDIUM_AND_ABOVE"
                },
                {
                    "category": "HARM_CATEGORY_HATE_SPEECH",
                    "threshold": "BLOCK_MEDIUM_AND_ABOVE"
                },
                {
                    "category": "HARM_CATEGORY_SEXUALLY_EXPLICIT",
                    "threshold": "BLOCK_MEDIUM_AND_ABOVE"
                },
                {
                    "category": "HARM_CATEGORY_DANGEROUS_CONTENT",
                    "threshold": "BLOCK_MEDIUM_AND_ABOVE"
                }
            ]
        });

        // Send request
        let response = self.client
            .post(&request_url)
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-goog-api-key", api_key)
            .json(&payload)
            .send()
            .await?;

        let status = response.status();
        log::info!("Gemini response status: {}", status);

        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            let error_msg = format!("Gemini API error: {} - {}", status, error_body);
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
        ProviderType::Gemini
    }

    async fn validate_config(&self, config: &AppConfig) -> LLMResult<()> {
        if config.api_key.is_none() {
            return Err(anyhow::anyhow!("API key is required for Gemini provider"));
        }

        if !config.providers.contains_key("gemini") {
            return Err(anyhow::anyhow!("Gemini provider URL not configured"));
        }

        Ok(())
    }
}
