// src/local_llm_provider.rs
use async_trait::async_trait;
use tokio::sync::mpsc;
use std::error::Error;
use std::path::PathBuf;
use std::convert::Infallible; // For llm crate's inference callback

use crate::config::AppConfig;
use crate::llm_provider::LLMProvider;
// Import the llm crate itself
use llm::KnownModel;

pub struct LocalLlmProvider;

impl LocalLlmProvider {
    pub fn new() -> Self {
        LocalLlmProvider
    }
}

#[async_trait]
impl LLMProvider for LocalLlmProvider {
    async fn list_models(&self) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        // For local models, "listing models" is less about an API call
        // and more about what model file is currently specified or available.
        // This could be enhanced later to scan a directory, but for now,
        // it can return a placeholder or expect the model path to be in AppConfig.
        log::info!("LocalLlmProvider: list_models called. Returning placeholder.");
        Ok(vec!["Path specified in config".to_string()])
    }

    async fn send_chat_request(
        &self,
        input: &str,
        model_path_str: &str, // This is the path to the model file from AppConfig.default_model
        _config: &AppConfig,  // AppConfig might be used for other params in future (threads, etc.)
        tx: &mpsc::UnboundedSender<String>
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let model_path = PathBuf::from(model_path_str);
        if !model_path.exists() {
            let err_msg = format!("Model file not found: {}", model_path_str);
            log::error!("{}", err_msg);
            if tx.send(format!("Error: {}", err_msg)).is_err() {
                log::warn!("Failed to send model not found error to UI channel.");
            }
            // Send EOT even if model not found
            if tx.send(crate::EOT_SIGNAL.to_string()).is_err() {
                log::warn!("Failed to send EOT signal to UI after model not found error.");
            }
            return Err(err_msg.into());
        }

        log::info!("LocalLlmProvider: Loading model from path: {:?}", model_path);

        // Determine model architecture from path or use a dynamic approach
        // This is a simplified example. The llm crate might need more info or specific load functions.
        // We'll assume Llama for .gguf files as a common case.
        // The llm crate's `llm::load_dynamic` is better for auto-detection.
        // let architecture = llm::ModelArchitecture::Llama; // Example, try to infer or make configurable - REMOVED as load_dynamic with None is used

        let now = std::time::Instant::now();

        // Using llm::load_dynamic to automatically infer model type
        let model = llm::load_dynamic(
            None, // architecture: None lets llm crate infer it.
            &model_path,
            llm::TokenizerSource::Embedded, // Or specify a tokenizer file
            Default::default(), // ModelParameters
            llm::load_progress_callback_stdout // Progress callback
        )
        .map_err(|e| {
            let err_msg = format!("Failed to load local model: {}", e);
            log::error!("{}", err_msg);
            if tx.send(format!("Error: {}", err_msg)).is_err() {
                log::warn!("Failed to send model load error to UI channel.");
            }
            // Send EOT even if model loading fails
            if tx.send(crate::EOT_SIGNAL.to_string()).is_err() {
                log::warn!("Failed to send EOT signal to UI after model load error.");
            }
            Box::new(e) as Box<dyn Error + Send + Sync>
        })?;
        
        log::info!(
            "Local model loaded successfully. Time taken: {}ms",
            now.elapsed().as_millis()
        );

        let mut session = model.start_session(Default::default()); // InferenceSessionConfig
        let tx_clone = tx.clone(); 
        let inference_input = input.to_string(); // Clone input for the inference closure

        log::info!("Starting inference for local model...");
        
        let res = session.infer::<Infallible>( // Specify Infallible for the callback's error type
            model.as_ref(), 
            &mut rand::thread_rng(), // Ensure rand is in Cargo.toml
            &llm::InferenceRequest {
                prompt: (&inference_input).into(), // Use the cloned input
                parameters: &llm::InferenceParameters::default(), 
                play_back_previous_tokens: false,
                maximum_token_count: Some(1024), // Set a reasonable maximum token count
            },
            &mut Default::default(), // OutputRequest
            move |t| { // move tx_clone and inference_input (if not already .into()) into closure
                match t {
                    llm::InferenceResponse::InferredToken(token) => {
                        if let Err(e) = tx_clone.send(token) {
                            log::error!("Failed to send token from local LLM: {}", e);
                            return Ok(llm::InferenceFeedback::Halt); // Stop if channel is broken
                        }
                    }
                    llm::InferenceResponse::EotToken => {
                        log::info!("Local LLM EOT token received.");
                         return Ok(llm::InferenceFeedback::Halt);
                    }
                    _ => {} // Handle other InferenceResponse types as needed
                }
                Ok(llm::InferenceFeedback::Continue)
            }
        );

        let final_result = match res {
            Ok(_) => {
                log::info!("Local LLM inference completed successfully.");
                Ok(())
            }
            Err(llm::InferenceError::ContextFull) => {
                let err_msg = "Inference context full. Input may be too long.".to_string();
                log::error!("{}", err_msg);
                if tx.send(format!("Error: {}", err_msg)).is_err() {
                    log::warn!("Failed to send context full error to UI: {}", err_msg);
                }
                Err(err_msg.into())
            }
            Err(e) => { // Handles other llm::InferenceError types
                let err_msg = format!("Local LLM inference failed: {}", e);
                log::error!("{}", err_msg);
                if tx.send(format!("Error: {}", err_msg)).is_err() {
                    log::warn!("Failed to send inference error to UI: {}", e);
                }
                Err(err_msg.into())
            }
        };

        // Always send EOT signal, regardless of success or failure of inference itself
        if let Err(e) = tx.send(crate::EOT_SIGNAL.to_string()) {
            log::error!("Failed to send EOT signal: {}", e);
        }
        
        final_result // Return the actual result of the inference
    }
}
