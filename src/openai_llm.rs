// src/openai_llm.rs
use async_trait::async_trait;
use reqwest::{Client, header, Response};
use serde_json::json;
use tokio::sync::mpsc;
use futures::StreamExt; // Ensure this is in Cargo.toml, reqwest might bring it
use std::error::Error;

use crate::config::AppConfig;
use crate::llm_provider::LLMProvider;

pub struct OpenAIProviderLlm;

impl OpenAIProviderLlm {
    pub fn new() -> Self {
        OpenAIProviderLlm
    }

    // Helper method for streaming, adapted from old openai.rs
    async fn stream_response(&self, response: Response, tx: &mpsc::UnboundedSender<String>) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut stream = response.bytes_stream();
        let mut error_occurred_in_stream = false;

        while let Some(item) = stream.next().await {
            match item {
                Ok(bytes) => {
                    let chunk = String::from_utf8_lossy(&bytes).to_string();
                    log::debug!("OpenAI Response Chunk: {}", chunk);

                    for line in chunk.lines() {
                        let line = line.trim();
                        if line.starts_with("data: ") {
                            let json_data = &line[6..];
                            if json_data == "[DONE]" {
                                log::info!("OpenAI stream reported [DONE].");
                                // The [DONE] message means stream is finished from provider side.
                                // We will send our EOT_SIGNAL after this loop naturally finishes.
                                return Ok(()); // Exit the while loop and function.
                            }
                            if json_data.is_empty() {
                                continue;
                            }
                            match serde_json::from_str::<serde_json::Value>(json_data) {
                                Ok(json_val) => {
                                    let content = json_val
                                        .get("choices")
                                        .and_then(|choices| choices.get(0))
                                        .and_then(|choice| choice.get("delta"))
                                        .and_then(|delta| delta.get("content"))
                                        .and_then(|text| text.as_str())
                                        .unwrap_or("");
                                    if !content.is_empty() {
                                        if let Err(e) = tx.send(content.to_string()) {
                                            log::error!("Failed to send OpenAI response chunk to UI: {}", e);
                                            error_occurred_in_stream = true;
                                            break; // Break inner loop
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("Failed to parse JSON from OpenAI stream: {}. Chunk: '{}'", e, json_data);
                                }
                            }
                        }
                    }
                    if error_occurred_in_stream { break; } // Break outer loop
                }
                Err(e) => {
                    log::error!("Error reading OpenAI response stream: {}", e);
                    // This error is from the stream itself (e.g. network issue)
                    // We return it, and the caller (send_chat_request) will handle EOT.
                    return Err(Box::new(e));
                }
            }
        }
        
        if error_occurred_in_stream {
            // If error occurred due to tx.send failure, create a generic error.
            return Err("Failed to send data to UI channel during streaming.".into());
        }

        Ok(())
    }
}

#[async_trait]
impl LLMProvider for OpenAIProviderLlm {
    async fn list_models(&self) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        // For now, let's return a fixed list or an empty list.
        // Implementing full model listing is a bit out of scope for the immediate refactor
        // if the old provider didn't use it extensively in the chat loop.
        // The old code printed to stdout, which is not ideal for a library function.
        // Let's keep it simple or fetch from a known popular model.
        // To do it properly, we'd need AppConfig here to get API key and URL.
        log::info!("OpenAIProviderLlm: list_models called. Returning placeholder.");
        Ok(vec!["gpt-3.5-turbo".to_string(), "gpt-4".to_string()]) // Placeholder
    }

    async fn send_chat_request(
        &self,
        input: &str,
        model_name: &str,
        config: &AppConfig,
        tx: &mpsc::UnboundedSender<String>
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let api_key = match config.api_key.as_ref() {
            Some(k) => k,
            None => {
                let err_msg = "API key not provided for OpenAI".to_string();
                log::error!("{}", err_msg);
                if tx.send(format!("Error: {}", err_msg)).is_err() {
                    log::warn!("Failed to send API key error to UI.");
                }
                // EOT is sent by the final block
                return Err(err_msg.into());
            }
        };
        let base_url = match config.providers.get("openai") {
             Some(url) => url,
             None => {
                let err_msg = "OpenAI provider URL not configured".to_string();
                log::error!("{}", err_msg);
                if tx.send(format!("Error: {}", err_msg)).is_err() {
                    log::warn!("Failed to send URL config error to UI.");
                }
                 // EOT is sent by the final block
                return Err(err_msg.into());
             }
        };

        let client = Client::new();
        let request_url = format!("{}chat/completions", base_url.trim_end_matches('/'));

        log::info!("OpenAI Request URL: {}, Model: {}", request_url, model_name);

        let request_payload = json!({
            "model": model_name,
            "messages": [{"role": "user", "content": input}],
            "stream": true,
        });

        log::debug!("OpenAI Request Payload: {}", request_payload.to_string());

        let final_result = async {
            let request = client.post(&request_url)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {}", api_key))
                .json(&request_payload);

            let response = request.send().await.map_err(|e| {
                log::error!("Failed to send OpenAI request: {}", e);
                Box::new(e) as Box<dyn Error + Send + Sync>
            })?;

            let status = response.status();
            log::info!("OpenAI Response Status: {}", status);

            if status.is_success() {
                self.stream_response(response, tx).await
            } else {
                let error_body = response.text().await.unwrap_or_else(|_| "Unknown error body".to_string());
                log::error!("OpenAI API Error: {} - {}", status, error_body);
                let err_msg = format!("OpenAI API Error: {} - {}", status, error_body);
                if tx.send(format!("Error: {}", err_msg)).is_err() {
                    log::warn!("Failed to send API error to UI.");
                }
                Err(err_msg.into())
            }
        }.await;

        // Always send EOT signal after processing is done or an error occurred.
        if let Err(e) = tx.send(crate::EOT_SIGNAL.to_string()) {
            log::error!("Failed to send EOT signal for OpenAI: {}", e);
            // If sending EOT fails, the original error (if any) is more important to return.
            // If final_result was Ok, but EOT send failed, we might consider this an error.
            // For now, we prioritize returning the outcome of the API call.
        }
        
        final_result
    }
}
