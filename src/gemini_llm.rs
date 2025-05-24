// src/gemini_llm.rs
use async_trait::async_trait;
use reqwest::{Client, header, Response};
use serde_json::json; // For request, though Gemini uses specific structs too
use tokio::sync::mpsc;
use futures::StreamExt; // Ensure this is in Cargo.toml
use std::error::Error;

use crate::config::AppConfig;
use crate::llm_provider::LLMProvider;

// Re-define or import necessary structs from old gemini.rs if they are simple enough
// For brevity, the example below will simplify them or use serde_json::Value
// Ideally, you'd move the GeminiPart, GeminiContent, GeminiRequest structs here.

pub struct GeminiProviderLlm;

impl GeminiProviderLlm {
    pub fn new() -> Self {
        GeminiProviderLlm
    }

    // Helper method for streaming, adapted from old gemini.rs
    async fn stream_response(&self, response: Response, tx: &mpsc::UnboundedSender<String>) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut stream = response.bytes_stream();
        let mut error_occurred_in_stream = false;

        while let Some(item) = stream.next().await {
            match item {
                Ok(bytes) => {
                    let chunk = String::from_utf8_lossy(&bytes).to_string();
                    log::debug!("Gemini Response Chunk: {}", chunk);

                    for line in chunk.lines() {
                        if line.starts_with("data:") {
                            let json_str = line[5..].trim();
                            if json_str.is_empty() {
                                continue;
                            }
                            match serde_json::from_str::<serde_json::Value>(json_str) {
                                Ok(json_value) => {
                                    if let Some(candidates) = json_value.get("candidates").and_then(|v| v.as_array()) {
                                        for candidate in candidates {
                                            if let Some(content_obj) = candidate.get("content") {
                                                if let Some(parts) = content_obj.get("parts").and_then(|v| v.as_array()) {
                                                    for part in parts {
                                                        if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                                                            if !text.is_empty() {
                                                                if let Err(e) = tx.send(text.to_string()) {
                                                                    log::error!("Failed to send Gemini response chunk to UI: {}", e);
                                                                    error_occurred_in_stream = true;
                                                                    break; // Break inner part loop
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            if error_occurred_in_stream { break; } // Break candidate loop
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("Failed to parse JSON from Gemini stream: {}. Chunk: '{}'", e, json_str);
                                }
                            }
                        }
                        if error_occurred_in_stream { break; } // Break line loop
                    }
                }
                Err(e) => {
                    log::error!("Error reading Gemini response stream: {}", e);
                    // This error is from the stream itself (e.g. network issue)
                    // We return it, and the caller (send_chat_request) will handle EOT.
                    return Err(Box::new(e)); 
                }
            }
            if error_occurred_in_stream { break; } // Break main while loop
        }
        
        if error_occurred_in_stream {
             return Err("Failed to send data to UI channel during Gemini streaming.".into());
        }
        // Gemini stream ends when no more data is sent. Unlike OpenAI, no explicit [DONE].
        Ok(())
    }
}

#[async_trait]
impl LLMProvider for GeminiProviderLlm {
    async fn list_models(&self) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        // Placeholder, similar to OpenAIProviderLlm.
        // Proper implementation would require AppConfig for API key and URL.
        log::info!("GeminiProviderLlm: list_models called. Returning placeholder.");
        Ok(vec!["gemini-pro".to_string(), "gemini-1.5-flash".to_string()]) // Placeholder
    }

    async fn send_chat_request(
        &self,
        input: &str,
        model_name: &str, // This should be just the model name like "gemini-pro"
        config: &AppConfig,
        tx: &mpsc::UnboundedSender<String>
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let api_key = match config.api_key.as_ref() {
            Some(k) => k,
            None => {
                let err_msg = "API key not provided for Gemini".to_string();
                log::error!("{}", err_msg);
                if tx.send(format!("Error: {}", err_msg)).is_err() {
                    log::warn!("Failed to send API key error to UI.");
                }
                // EOT is sent by the final block
                return Err(err_msg.into());
            }
        };
        let base_url = match config.providers.get("gemini") {
            Some(url) => url,
            None => {
                let err_msg = "Gemini provider URL not configured".to_string();
                log::error!("{}", err_msg);
                if tx.send(format!("Error: {}", err_msg)).is_err() {
                    log::warn!("Failed to send URL config error to UI.");
                }
                // EOT is sent by the final block
                return Err(err_msg.into());
            }
        };
        
        let client = Client::new();
        let request_url = format!("{}models/{}:streamGenerateContent?alt=sse", base_url.trim_end_matches('/'), model_name);
        log::info!("Gemini Request URL: {}", request_url);

        let request_payload = json!({
            "contents": [{
                "parts": [{"text": input}]
            }]
            // TODO: Add generationConfig if needed (temperature, maxOutputTokens, etc.)
            // "generationConfig": {
            //   "temperature": 0.7,
            //   "maxOutputTokens": 1024,
            // }
        });
        log::debug!("Gemini Request Payload: {}", request_payload.to_string());

        let final_result = async {
            let request = client.post(&request_url)
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-goog-api-key", api_key)
                .json(&request_payload);

            let response = request.send().await.map_err(|e| {
                log::error!("Failed to send Gemini request: {}", e);
                Box::new(e) as Box<dyn Error + Send + Sync>
            })?;

            let status = response.status();
            log::info!("Gemini Response Status: {}", status);

            if status.is_success() {
                self.stream_response(response, tx).await
            } else {
                let error_body = response.text().await.unwrap_or_else(|_| "Unknown error body".to_string());
                log::error!("Gemini API Error: {} - {}", status, error_body);
                let err_msg = format!("Gemini API Error: {} - {}", status, error_body);
                if tx.send(format!("Error: {}", err_msg)).is_err() {
                     log::warn!("Failed to send API error to UI.");
                }
                Err(err_msg.into())
            }
        }.await;

        // Always send EOT signal after processing is done or an error occurred.
        if let Err(e) = tx.send(crate::EOT_SIGNAL.to_string()) {
            log::error!("Failed to send EOT signal for Gemini: {}", e);
        }

        final_result
    }
}
