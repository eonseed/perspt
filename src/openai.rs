// src/openai.rs
use reqwest::{Client, header, Response};
use serde_json::json;
use tokio::sync::mpsc::UnboundedSender;
use futures::StreamExt;

#[derive(Debug)]
pub struct OpenAIProvider<'a> {
    api_url: &'a str,
    api_key: String,
}

impl<'a> OpenAIProvider<'a> {
    pub fn new(api_url: &'a str, api_key: String) -> Self {
        OpenAIProvider { api_url, api_key }
    }
    pub async fn list_models(&self) -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new();
        let request_url = format!("{}/models", self.api_url);
        let request = client
            .get(&request_url)
            .header("Authorization", format!("Bearer {}", self.api_key));
        let response = request.send().await?;
        if response.status().is_success() {
            let body = response.text().await?;
            let json_value: serde_json::Value = serde_json::from_str(&body)?;
             if let Some(models) = json_value["data"].as_array() {
                println!("Available models:");
                for model in models {
                    if let Some(id) = model["id"].as_str() {
                         println!("- {}", id);
                    }
                }
             } else {
                println!("No models found");
            }
        }  else {
            println!("Failed to fetch models: {}", response.status());
        }
         Ok(())
    }

    pub async fn send_chat_request(
        &self,
        input: &str,
        model_name: &str,
         tx: &UnboundedSender<String>
    ) -> Result<(), String> {
        let client = Client::new();
        let request_url = format!("{}chat/completions", self.api_url);
        log::info!("Request URL: {}", request_url);
         let request_payload = json!({
             "model": model_name,
             "messages": [{
                "role": "user",
                "content": input
            }],
             "stream": true,
         });
        log::info!("Request Payload: {}", request_payload.to_string());
        let request = client.post(request_url)
            .header(header::CONTENT_TYPE, "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request_payload);

        let response = request.send().await;
        match response {
            Ok(res) => {
                let status = res.status();
                log::info!("Response Status: {}", status);
                if status.is_success() {
                    self.stream_response(res, tx).await
                } else {
                    let body = res.text().await.unwrap_or_else(|_| "No body".to_string());
                    Err(format!("API Error: {} {}", status, body))
                }
            },
            Err(e) => Err(format!("Failed to send request: {}", e)),
        }
    }

    async fn stream_response(&self, response: Response, tx: &UnboundedSender<String>) -> Result<(), String> {
         let mut stream = response.bytes_stream();
        while let Some(item) = stream.next().await {
            match item {
                Ok(bytes) => {
                     let chunk = String::from_utf8_lossy(&bytes).to_string();
                     log::info!("Response Chunk: {}", chunk);
                     let json_val: serde_json::Value = serde_json::from_str(&chunk)
                         .map_err(|e| format!("Failed to parse JSON: {}", e))?;
                     let content = json_val
                        .get("choices")
                        .and_then(|choices| choices.get(0))
                        .and_then(|choice| choice.get("delta"))
                        .and_then(|delta| delta.get("content"))
                         .and_then(|text| text.as_str())
                         .unwrap_or("No response")
                        .to_string();
                     tx.send(content).expect("Failed to send response chunk");
                }
                Err(e) => {
                    return Err(format!("Error reading response: {}", e));
                }
            }
        }
        Ok(())
    }
}