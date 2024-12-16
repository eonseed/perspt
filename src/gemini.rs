use reqwest::{Client, header, Response};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;
use futures::StreamExt;
use serde_json::Value;
use tokio::sync::watch;

#[derive(Debug)]
pub struct GeminiProvider<'a> {
    api_url: &'a str,
    api_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
}
#[derive(Debug, Serialize, Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiCandidate {
   content: GeminiContent,
}
impl<'a> GeminiProvider<'a> {
    pub fn new(api_url: &'a str, api_key: String) -> Self {
        GeminiProvider { api_url, api_key }
    }


    pub async fn list_models(&self) -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new();
        let request_url = format!("{}models", self.api_url);
        let request = client
            .get(&request_url)
            .header("x-goog-api-key",  self.api_key.clone());
        let response = request.send().await?;
         if response.status().is_success() {
            let body = response.text().await?;
            let json_value: serde_json::Value = serde_json::from_str(&body)?;
            if let Some(models) = json_value["models"].as_array() {
                println!("Available models:");
                for model in models {
                    if let Some(id) = model["name"].as_str() {
                        println!("- {}", id);
                    }
                }
            } else {
                println!("No models found");
            }
        } else {
            println!("Failed to fetch models: {}", response.status());
        }
        Ok(())
    }


    pub async fn send_chat_request(
        &self,
        input: &str,
        model_name: &str,
        tx: &UnboundedSender<String>,
        interrupt_rx: &watch::Receiver<bool>
    ) -> Result<(), String> {
         let client = Client::new();
        let request_url = format!("{}models/{}:streamGenerateContent?alt=sse", self.api_url, model_name);
        log::info!("Request URL: {}", request_url);

          let gemini_request = GeminiRequest {
            contents: vec![
                GeminiContent {
                    parts: vec![
                       GeminiPart {text: input.to_string()}
                    ]
                }
            ]
         };

        log::info!("Request Payload: {}", serde_json::to_string(&gemini_request).unwrap());

        let request = client.post(request_url)
            .header(header::CONTENT_TYPE, "application/json")
             .header("x-goog-api-key",  self.api_key.clone())
            .json(&gemini_request);


        let response = request.send().await;
        match response {
            Ok(res) => {
                let status = res.status();
                log::info!("Response Status: {}", status);
                if status.is_success() {
                    self.stream_response(res, tx, interrupt_rx).await
                } else {
                    let body = res.text().await.unwrap_or_else(|_| "No body".to_string());
                    Err(format!("API Error: {} {}", status, body))
                }
            },
            Err(e) => Err(format!("Failed to send request: {}", e)),
        }
    }


   async fn stream_response(&self, response: Response, tx: &UnboundedSender<String>, interrupt_rx: &watch::Receiver<bool>) -> Result<(), String> {
        let mut stream = response.bytes_stream();
        while let Some(item) = stream.next().await {
            if *interrupt_rx.borrow() {
                return Err("Request interrupted by user".to_string());
            }
            match item {
                Ok(bytes) => {
                    let chunk = String::from_utf8_lossy(&bytes).to_string();
                    log::info!("Response Chunk: {}", chunk);

                    let mut lines = chunk.lines();
                    while let Some(line) = lines.next() {
                         if line.starts_with("data:") {
                            let json_str = line[5..].trim();
                            if json_str.is_empty(){
                                continue;
                            }
                            let json_val: Result<Value, serde_json::Error> = serde_json::from_str(json_str);
                            match json_val {
                                Ok(json_value) => {
                                    if let Some(candidates) = json_value.get("candidates").and_then(|v| v.as_array()) {
                                        for candidate in candidates {
                                            if let Some(content) = candidate.get("content") {
                                                if let Some(parts) = content.get("parts").and_then(|v| v.as_array()){
                                                    for part in parts {
                                                        if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                                                            tx.send(text.to_string()).expect("Failed to send response chunk");
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                 }
                                Err(e) => {
                                    log::error!("Failed to parse JSON: {}, chunk: {}", e, json_str);
                                     return Err(format!("Failed to parse JSON: {}", e));
                                }
                            }
                        }
                    }

                }
                Err(e) => {
                    return Err(format!("Error reading response: {}", e));
                }
            }
        }
        Ok(())
    }
}
