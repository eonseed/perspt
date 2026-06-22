//! # LLM Provider Module
//!
//! Thread-safe LLM provider abstraction for multi-agent use.
//! Wraps genai::Client with Arc<RwLock<>> for shared state.

use anyhow::{Context, Result};
use futures::StreamExt;
use genai::adapter::AdapterKind;
use genai::chat::{ChatMessage, ChatRequest, ChatStreamEvent};
use genai::resolver::{AuthData, AuthResolver, Endpoint, ProviderConfig, ServiceTargetResolver};
use genai::Client;
use genai::ModelIden;
use genai::ServiceTarget;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, RwLock};

use crate::config::Config;

/// End of transmission signal
pub const EOT_SIGNAL: &str = "<|EOT|>";

/// Effective provider id and model after merging config, CLI flags, and env.
#[derive(Debug, Clone)]
pub struct ResolvedProvider {
    /// Provider id, e.g. `openai`, `ollama`.
    pub provider: String,
    /// Model name to use (passed to genai verbatim so namespacing works).
    pub model: String,
}

/// Detect the provider id and a sensible default model from environment keys.
///
/// Used as the fallback when no provider is configured. Falls back to a local
/// Ollama setup when no API keys are present.
pub fn detect_provider_from_env() -> (&'static str, &'static str) {
    if vertex_project_from_env().is_some() {
        // Google Vertex AI (Agent/AI Platform). Models are namespace-routed as
        // `vertex::<model>`; auth is an OAuth2 Bearer token from ADC or
        // VERTEX_API_KEY.
        ("vertex", "vertex::gemini-2.5-flash")
    } else if std::env::var("GEMINI_API_KEY").is_ok() {
        ("gemini", "gemini-3.1-flash-lite-preview")
    } else if std::env::var("OPENAI_API_KEY").is_ok() {
        ("openai", "gpt-4o-mini")
    } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        ("anthropic", "claude-3-5-sonnet-20241022")
    } else if std::env::var("GROQ_API_KEY").is_ok() {
        ("groq", "llama-3.1-8b-instant")
    } else if std::env::var("COHERE_API_KEY").is_ok() {
        ("cohere", "command-r-plus")
    } else if std::env::var("XAI_API_KEY").is_ok() {
        ("xai", "grok-beta")
    } else if std::env::var("DEEPSEEK_API_KEY").is_ok() {
        ("deepseek", "deepseek-chat")
    } else {
        // Default to Ollama for local usage
        ("ollama", "llama3.2")
    }
}

/// Response from a non-streaming LLM call, carrying text and token usage.
#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub text: String,
    pub tokens_in: Option<i32>,
    pub tokens_out: Option<i32>,
}

/// Shared state for rate limiting and token counting
#[derive(Default)]
struct SharedState {
    total_tokens_used: usize,
    request_count: usize,
}

/// Thread-safe LLM provider implementation using Arc<RwLock<>>.
///
/// This provider can be cheaply cloned and shared across multiple agents.
/// Each clone shares the same underlying client and rate limiting state.
#[derive(Clone)]
pub struct GenAIProvider {
    /// The underlying genai client
    client: Arc<Client>,
    /// Shared state for rate limiting and metrics
    shared: Arc<RwLock<SharedState>>,
}

impl GenAIProvider {
    /// Creates a new GenAI provider with automatic configuration.
    pub fn new() -> Result<Self> {
        let client = Client::default();
        Ok(Self::from_client(client))
    }

    /// Creates a new GenAI provider with explicit configuration.
    pub fn new_with_config(provider_type: Option<&str>, api_key: Option<&str>) -> Result<Self> {
        let adapter_kind = provider_type.and_then(|provider| match str_to_adapter_kind(provider) {
            Ok(adapter_kind) => Some(adapter_kind),
            Err(_) => {
                log::warn!("Unknown provider type for genai client: {provider}");
                None
            }
        });

        // Set environment variable if API key is provided
        if let (Some(provider), Some(key)) = (provider_type, api_key) {
            if let Some(env_var) = provider_api_key_env_var(provider) {
                log::info!("Setting {env_var} environment variable for genai client");
                std::env::set_var(env_var, key);
            } else if provider.eq_ignore_ascii_case("ollama") {
                log::info!("Ollama provider detected - no API key required for local setup");
            } else {
                log::warn!("Unknown provider type for API key: {provider}");
            }
        }

        let is_vertex = provider_type
            .map(|p| p.eq_ignore_ascii_case("vertex"))
            .unwrap_or(false);

        let client = if is_vertex {
            // Vertex AI authenticates with an OAuth2 Bearer token from ADC; no
            // static API key is required when ADC is configured.
            build_vertex_client()
        } else {
            match adapter_kind {
                Some(adapter_kind) => build_bound_client(adapter_kind, provider_type),
                None => Client::default(),
            }
        };

        Ok(Self::from_client(client))
    }

    /// Build a provider from a `Config`, merging in environment detection and an
    /// optional CLI model override, and return the effective provider/model.
    ///
    /// Precedence:
    ///   - provider: `config.provider` > environment detection
    ///   - model:    `cli_model` > `config.model` > provider default
    ///   - api_key:  `config.api_key` > ambient environment
    ///   - base_url: `config.base_url` > ambient environment
    ///
    /// The returned client is bound to the resolved adapter, so custom/local
    /// OpenAI-compatible model names (e.g. `phi-4-npu-ov`) route correctly while
    /// recognized names still resolve by prefix. Model names are passed through
    /// verbatim so genai namespacing (`openai::model`) keeps working.
    pub fn from_config(
        config: &Config,
        cli_model: Option<&str>,
    ) -> Result<(Self, ResolvedProvider)> {
        let (env_provider, env_model) = detect_provider_from_env();

        let env_model_override = std::env::var("OPENAI_MODEL")
            .or_else(|_| std::env::var("MODEL"))
            .ok();

        let model = cli_model
            .map(str::to_string)
            .or_else(|| config.model.clone())
            .or(env_model_override)
            .unwrap_or_else(|| env_model.to_string());

        let provider = config
            .provider
            .clone()
            .or_else(|| provider_from_model_namespace(&model).map(str::to_string))
            .unwrap_or_else(|| env_provider.to_string());

        // Propagate a configured base URL into the env var that build_bound_client
        // reads, without clobbering an explicit ambient override.
        if let Some(base_url) = config.base_url.as_deref() {
            if let Some(env_var) = provider_base_url_env_var(&provider) {
                if std::env::var(env_var).is_err() {
                    std::env::set_var(env_var, base_url);
                }
            }
        }

        if provider.eq_ignore_ascii_case("vertex") {
            configure_vertex_environment(config);
        }

        let provider_obj = Self::new_with_config(Some(&provider), config.api_key.as_deref())?;
        Ok((provider_obj, ResolvedProvider { provider, model }))
    }

    fn from_client(client: Client) -> Self {
        Self {
            client: Arc::new(client),
            shared: Arc::new(RwLock::new(SharedState::default())),
        }
    }

    /// Get total tokens used across all requests
    pub async fn get_total_tokens_used(&self) -> usize {
        self.shared.read().await.total_tokens_used
    }

    /// Get total request count
    pub async fn get_request_count(&self) -> usize {
        self.shared.read().await.request_count
    }

    /// Increment request counter (for metrics)
    async fn increment_request(&self) {
        let mut state = self.shared.write().await;
        state.request_count += 1;
    }

    /// Add tokens to the total count
    pub async fn add_tokens(&self, count: usize) {
        let mut state = self.shared.write().await;
        state.total_tokens_used += count;
    }

    /// Retrieves all available models for a specific provider.
    pub async fn get_available_models(&self, provider: &str) -> Result<Vec<String>> {
        let adapter_kind = str_to_adapter_kind(provider)?;
        let provider_config = provider_base_url_from_env(provider)
            .map(|base_url| {
                ProviderConfig::from_endpoint(Endpoint::from_owned(normalize_base_url(&base_url)))
            })
            .unwrap_or_default();

        let models = self
            .client
            .all_model_names(adapter_kind, provider_config)
            .await
            .context(format!("Failed to get models for provider: {provider}"))?;

        Ok(models)
    }

    /// Generates a simple text response without streaming.
    /// Includes exponential backoff retry for rate limits and transient errors.
    pub async fn generate_response_simple(&self, model: &str, prompt: &str) -> Result<LlmResponse> {
        self.generate_response_with_retry(model, prompt, 3).await
    }

    /// Generates a response with configurable retry count and exponential backoff.
    pub async fn generate_response_with_retry(
        &self,
        model: &str,
        prompt: &str,
        max_retries: usize,
    ) -> Result<LlmResponse> {
        self.increment_request().await;

        let chat_req = ChatRequest::default().append_message(ChatMessage::user(prompt));

        log::debug!(
            "Sending chat request to model: {model} with prompt length: {} chars",
            prompt.len()
        );

        let start_time = Instant::now();
        let mut last_error: Option<anyhow::Error> = None;
        let mut retry_count = 0;

        while retry_count <= max_retries {
            if retry_count > 0 {
                // Exponential backoff: 1s, 2s, 4s, 8s, ... (capped at 16s)
                let delay_secs = std::cmp::min(1u64 << (retry_count - 1), 16);
                log::warn!(
                    "Retry {}/{} for model {} after {}s delay (previous error: {:?})",
                    retry_count,
                    max_retries,
                    model,
                    delay_secs,
                    last_error.as_ref().map(|e| e.to_string())
                );
                println!(
                    "   ⏳ Rate limited, retrying in {}s (attempt {}/{})",
                    delay_secs, retry_count, max_retries
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(delay_secs)).await;
            }

            match self.client.exec_chat(model, chat_req.clone(), None).await {
                Ok(chat_res) => {
                    let tokens_in = chat_res.usage.prompt_tokens;
                    let tokens_out = chat_res.usage.completion_tokens;
                    let content = chat_res
                        .first_text()
                        .context("No text content in response")?;
                    log::debug!(
                        "Received response with {} characters in {}ms (tokens: in={:?}, out={:?})",
                        content.len(),
                        start_time.elapsed().as_millis(),
                        tokens_in,
                        tokens_out,
                    );

                    // Update shared token counter with real values when available
                    let total = tokens_in.unwrap_or(0) + tokens_out.unwrap_or(0);
                    if total > 0 {
                        self.add_tokens(total as usize).await;
                    }

                    return Ok(LlmResponse {
                        text: content.to_string(),
                        tokens_in,
                        tokens_out,
                    });
                }
                Err(e) => {
                    let err_str = e.to_string();

                    // Check if it's a retryable error (rate limit, server error, network)
                    let is_retryable = err_str.contains("429")
                        || err_str.contains("rate limit")
                        || err_str.contains("Rate limit")
                        || err_str.contains("RESOURCE_EXHAUSTED")
                        || err_str.contains("500")
                        || err_str.contains("502")
                        || err_str.contains("503")
                        || err_str.contains("504")
                        || err_str.contains("timeout")
                        || err_str.contains("connection");

                    if is_retryable && retry_count < max_retries {
                        log::warn!("Retryable error for model {}: {}", model, err_str);
                        last_error = Some(anyhow::anyhow!("{}", err_str));
                        retry_count += 1;
                        continue;
                    } else {
                        return Err(anyhow::anyhow!(
                            "Failed to execute chat request for model {}: {}",
                            model,
                            err_str
                        ));
                    }
                }
            }
        }

        // Should not reach here, but handle gracefully
        Err(last_error
            .unwrap_or_else(|| anyhow::anyhow!("Unknown error after {} retries", max_retries)))
    }

    /// Generates a streaming response and sends chunks via mpsc channel.
    pub async fn generate_response_stream_to_channel(
        &self,
        model: &str,
        prompt: &str,
        tx: mpsc::UnboundedSender<String>,
    ) -> Result<()> {
        self.increment_request().await;

        let chat_req = ChatRequest::default().append_message(ChatMessage::user(prompt));

        log::debug!("Sending streaming chat request to model: {model} with prompt: {prompt}");

        let chat_res_stream = self
            .client
            .exec_chat_stream(model, chat_req, None)
            .await
            .context(format!(
                "Failed to execute streaming chat request for model: {model}"
            ))?;

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

        while let Some(chunk_result) = stream.next().await {
            let elapsed = start_time.elapsed();

            match chunk_result {
                Ok(ChatStreamEvent::Start) => {
                    log::info!(">>> STREAM STARTED for model: {model} at {elapsed:?}");
                }
                Ok(ChatStreamEvent::Chunk(chunk)) => {
                    chunk_count += 1;
                    total_content_length += chunk.content.len();

                    if chunk_count % 10 == 0 || chunk.content.len() > 100 {
                        log::info!(
                            "CHUNK #{}: {} chars, total: {} chars, elapsed: {:?}",
                            chunk_count,
                            chunk.content.len(),
                            total_content_length,
                            elapsed
                        );
                    }

                    if !chunk.content.is_empty() && tx.send(chunk.content.clone()).is_err() {
                        log::error!(
                            "!!! CHANNEL SEND FAILED for chunk #{chunk_count} - STOPPING STREAM !!!"
                        );
                        break;
                    }
                }
                Ok(ChatStreamEvent::ReasoningChunk(chunk)) => {
                    log::info!(
                        "REASONING CHUNK: {} chars at {:?}",
                        chunk.content.len(),
                        elapsed
                    );
                    if !chunk.content.is_empty() {
                        let _ = tx.send(format!("__PERSPT_REASONING__:{}", chunk.content));
                    }
                }
                Ok(ChatStreamEvent::End(_)) => {
                    log::info!(">>> STREAM ENDED EXPLICITLY for model: {model} after {chunk_count} chunks, {total_content_length} chars, {elapsed:?} elapsed");
                    stream_ended_explicitly = true;
                    break;
                }
                Ok(ChatStreamEvent::ToolCallChunk(_)) => {
                    log::debug!("Tool call chunk received (ignored)");
                }
                Ok(ChatStreamEvent::ThoughtSignatureChunk(_)) => {
                    log::debug!("Thought signature chunk received (ignored)");
                }
                Err(e) => {
                    log::error!(
                        "!!! STREAM ERROR after {chunk_count} chunks at {elapsed:?}: {e} !!!"
                    );
                    let error_msg = format!("Stream error: {e}");
                    let _ = tx.send(error_msg);
                    return Err(e.into());
                }
            }
        }

        let final_elapsed = start_time.elapsed();
        if !stream_ended_explicitly {
            log::warn!("!!! STREAM ENDED IMPLICITLY (exhausted) for model: {model} after {chunk_count} chunks, {total_content_length} chars, {final_elapsed:?} elapsed !!!");
        }

        log::info!(
            "=== STREAM COMPLETE === Model: {model}, Final: {chunk_count} chunks, {total_content_length} chars, {final_elapsed:?} elapsed"
        );

        // Add approximate token count
        self.add_tokens(total_content_length / 4).await; // Rough estimate

        if tx.send(EOT_SIGNAL.to_string()).is_err() {
            log::error!("!!! FAILED TO SEND EOT SIGNAL - channel may be closed !!!");
            return Err(anyhow::anyhow!("Channel closed during EOT signal send"));
        }

        log::info!(">>> EOT SIGNAL SENT for model: {model} <<<");
        Ok(())
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
            "vertex",
            "xai",
            "deepseek",
        ]
    }

    /// Get all available providers
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
                log::info!("Model {model} is available and working");
                Ok(true)
            }
            Err(e) => {
                log::warn!("Model {model} test failed: {e}");
                Ok(false)
            }
        }
    }

    /// Validate and get the best available model for a provider
    pub async fn validate_model(&self, model: &str, provider_type: Option<&str>) -> Result<String> {
        if self.test_model(model).await? {
            return Ok(model.to_string());
        }

        if let Some(provider) = provider_type {
            if let Ok(models) = self.get_available_models(provider).await {
                if !models.is_empty() {
                    log::info!("Model {} not available, using {} instead", model, models[0]);
                    return Ok(models[0].clone());
                }
            }
        }

        log::warn!("Could not validate model {model}, proceeding anyway");
        Ok(model.to_string())
    }
}

/// Build a genai client for Google Vertex AI authenticated via Application
/// Default Credentials (ADC).
///
/// genai's Vertex adapter reads `VERTEX_PROJECT_ID` (required) and
/// `VERTEX_LOCATION` to construct the request
/// URL, and expects an OAuth2 Bearer token from an [`AuthResolver`]. This
/// resolver fetches that token from ADC (gcloud login, a service account, or the
/// metadata server) on each request, so no static API key is needed. If a
/// `VERTEX_API_KEY` bearer token is explicitly set, it is used as an override.
fn build_vertex_client() -> Client {
    let resolver = AuthResolver::from_resolver_async_fn(
        |_model: ModelIden| -> Pin<
            Box<dyn Future<Output = genai::resolver::Result<Option<AuthData>>> + Send>,
        > {
            Box::pin(async move {
                // Explicit bearer-token override wins when present.
                if let Ok(token) = std::env::var("VERTEX_API_KEY") {
                    if !token.trim().is_empty() {
                        return Ok(Some(AuthData::from_single(token)));
                    }
                }
                // Otherwise resolve an access token from ADC.
                let provider = gcp_auth::provider().await.map_err(|e| {
                    genai::resolver::Error::Custom(format!(
                        "Vertex ADC provider init failed (run `gcloud auth application-default login`): {e}"
                    ))
                })?;
                let scopes = ["https://www.googleapis.com/auth/cloud-platform"];
                let token = provider.token(&scopes).await.map_err(|e| {
                    genai::resolver::Error::Custom(format!("Vertex ADC token fetch failed: {e}"))
                })?;
                Ok(Some(AuthData::from_single(token.as_str())))
            })
        },
    );

    let mut builder = Client::builder()
        .with_adapter_kind(AdapterKind::Vertex)
        .with_auth_resolver(resolver);

    if let Some(endpoint) = resolved_vertex_endpoint() {
        builder = builder.with_service_target_resolver_fn(move |mut target: ServiceTarget| {
            target.endpoint = Endpoint::from_owned(endpoint.clone());
            Ok(target)
        });
    }

    builder.build()
}

fn build_bound_client(adapter_kind: AdapterKind, provider_type: Option<&str>) -> Client {
    let mut builder = Client::builder().with_adapter_kind(adapter_kind);

    if let Some(base_url) = provider_type.and_then(provider_base_url_from_env) {
        let endpoint = normalize_base_url(&base_url);
        let target_resolver = ServiceTargetResolver::from_resolver_fn(
            move |mut service_target: ServiceTarget| -> genai::resolver::Result<ServiceTarget> {
                if service_target.model.adapter_kind == adapter_kind {
                    service_target.endpoint = Endpoint::from_owned(endpoint.clone());
                }
                Ok(service_target)
            },
        );
        builder = builder.with_service_target_resolver(target_resolver);
    }

    builder.build()
}

fn provider_from_model_namespace(model: &str) -> Option<&'static str> {
    let lower = model.to_ascii_lowercase();
    lower.split_once("::").and_then(|(prefix, _)| match prefix {
        "openai" => Some("openai"),
        "anthropic" => Some("anthropic"),
        "gemini" | "google" => Some("gemini"),
        "vertex" => Some("vertex"),
        "groq" => Some("groq"),
        "cohere" => Some("cohere"),
        "ollama" => Some("ollama"),
        "xai" => Some("xai"),
        "deepseek" => Some("deepseek"),
        _ => None,
    })
}

fn configure_vertex_environment(config: &Config) {
    if std::env::var("VERTEX_PROJECT_ID").is_err() {
        if let Some(project) = config
            .vertex_project_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
            .or_else(vertex_project_from_env)
            .or_else(read_gcloud_project)
        {
            // Only propagate a syntactically valid project into the process env;
            // genai reads VERTEX_PROJECT_ID to build the request URL, so an
            // invalid value should not be planted there from config discovery.
            match valid_vertex_segment(&project) {
                Some(valid) => std::env::set_var("VERTEX_PROJECT_ID", valid),
                None => log::warn!(
                    "Ignoring discovered Vertex project ID (must contain only ASCII letters, \
                     digits, and hyphens)"
                ),
            }
        }
    }

    if std::env::var("VERTEX_LOCATION").is_err() {
        if let Some(location) = config
            .vertex_location
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            // The location is interpolated into the endpoint host, so never
            // plant an invalid value into the env from a config file.
            match valid_vertex_segment(location) {
                Some(valid) => std::env::set_var("VERTEX_LOCATION", valid),
                None => log::warn!(
                    "Ignoring invalid vertex_location from config (must contain only ASCII \
                     letters, digits, and hyphens)"
                ),
            }
        }
    }
}

fn vertex_project_from_env() -> Option<String> {
    [
        "VERTEX_PROJECT_ID",
        "GOOGLE_CLOUD_PROJECT",
        "GCLOUD_PROJECT",
        "CLOUDSDK_CORE_PROJECT",
    ]
    .into_iter()
    .filter_map(|key| std::env::var(key).ok())
    .map(|value| value.trim().to_string())
    .find(|value| !value.is_empty())
}

fn gcloud_config_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("CLOUDSDK_CONFIG") {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }
    dirs::home_dir().map(|home| home.join(".config").join("gcloud"))
}

fn read_gcloud_project() -> Option<String> {
    let config_dir = gcloud_config_dir()?;
    let active_config = std::fs::read_to_string(config_dir.join("active_config"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "default".to_string());
    let config_path = config_dir
        .join("configurations")
        .join(format!("config_{active_config}"));
    let content = std::fs::read_to_string(config_path).ok()?;
    parse_gcloud_project(&content)
}

fn parse_gcloud_project(content: &str) -> Option<String> {
    let mut in_core = false;
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_core = line.eq_ignore_ascii_case("[core]");
            continue;
        }
        if !in_core {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == "project" {
            let project = value.trim();
            if !project.is_empty() {
                return Some(project.to_string());
            }
        }
    }
    None
}

/// Validate a Vertex project/location segment before it is interpolated into the
/// request endpoint URL.
///
/// GCP project IDs and Vertex locations are limited to ASCII letters, digits,
/// and hyphens. Rejecting anything else prevents a crafted value (e.g. one
/// containing `/`, `.`, `:`, or `@`) from altering the endpoint *host* — the
/// location is interpolated into `{location}-aiplatform.googleapis.com`, so an
/// unvalidated value could otherwise redirect the OAuth bearer token to an
/// attacker-controlled host. Returns the trimmed value when valid.
fn valid_vertex_segment(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-')
        .then_some(trimmed)
}

fn resolved_vertex_endpoint() -> Option<String> {
    let project_raw = std::env::var("VERTEX_PROJECT_ID").ok()?;
    let project = match valid_vertex_segment(&project_raw) {
        Some(p) => p.to_string(),
        None => {
            log::warn!(
                "Ignoring VERTEX_PROJECT_ID for endpoint construction: must be non-empty and \
                 contain only ASCII letters, digits, and hyphens"
            );
            return None;
        }
    };
    let location = match std::env::var("VERTEX_LOCATION") {
        Ok(raw) if !raw.trim().is_empty() => match valid_vertex_segment(&raw) {
            Some(l) => l.to_string(),
            None => {
                log::warn!(
                    "Ignoring invalid VERTEX_LOCATION (must contain only ASCII letters, digits, \
                     and hyphens); falling back to 'global'"
                );
                "global".to_string()
            }
        },
        _ => "global".to_string(),
    };
    Some(vertex_endpoint_base(&project, &location))
}

fn vertex_endpoint_base(project: &str, location: &str) -> String {
    let project = project.trim();
    let location = location.trim();
    if location.eq_ignore_ascii_case("global") {
        format!("https://aiplatform.googleapis.com/v1/projects/{project}/locations/global/")
    } else {
        format!(
            "https://{location}-aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/"
        )
    }
}

fn provider_base_url_env_var(provider: &str) -> Option<&'static str> {
    match provider.to_lowercase().as_str() {
        "openai" => Some("OPENAI_BASE_URL"),
        "anthropic" => Some("ANTHROPIC_BASE_URL"),
        "gemini" | "google" => Some("GEMINI_BASE_URL"),
        "groq" => Some("GROQ_BASE_URL"),
        "cohere" => Some("COHERE_BASE_URL"),
        "ollama" => Some("OLLAMA_BASE_URL"),
        "xai" => Some("XAI_BASE_URL"),
        "deepseek" => Some("DEEPSEEK_BASE_URL"),
        _ => None,
    }
}

fn provider_base_url_from_env(provider: &str) -> Option<String> {
    let env_var = provider_base_url_env_var(provider)?;

    std::env::var(env_var)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn provider_api_key_env_var(provider: &str) -> Option<&'static str> {
    match provider.to_lowercase().as_str() {
        "openai" => Some("OPENAI_API_KEY"),
        "anthropic" => Some("ANTHROPIC_API_KEY"),
        "gemini" | "google" => Some("GEMINI_API_KEY"),
        "vertex" => Some("VERTEX_API_KEY"),
        "groq" => Some("GROQ_API_KEY"),
        "cohere" => Some("COHERE_API_KEY"),
        "xai" => Some("XAI_API_KEY"),
        "deepseek" => Some("DEEPSEEK_API_KEY"),
        _ => None,
    }
}

fn normalize_base_url(base_url: &str) -> String {
    if base_url.ends_with('/') {
        base_url.to_string()
    } else {
        format!("{base_url}/")
    }
}

/// Convert a provider string to genai AdapterKind
fn str_to_adapter_kind(provider: &str) -> Result<AdapterKind> {
    match provider.to_lowercase().as_str() {
        "openai" => Ok(AdapterKind::OpenAI),
        "anthropic" => Ok(AdapterKind::Anthropic),
        "gemini" | "google" => Ok(AdapterKind::Gemini),
        "vertex" => Ok(AdapterKind::Vertex),
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
        assert!(str_to_adapter_kind("vertex").is_ok());
        assert!(str_to_adapter_kind("xai").is_ok());
        assert!(str_to_adapter_kind("deepseek").is_ok());
        assert!(str_to_adapter_kind("invalid").is_err());
    }

    #[tokio::test]
    async fn test_provider_creation() {
        let provider = GenAIProvider::new();
        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn test_configured_provider_binds_adapter_for_custom_model_names() {
        let provider = GenAIProvider::new_with_config(Some("openai"), None).unwrap();
        let target = provider
            .client
            .resolve_service_target("gemma4-32b-it")
            .await
            .unwrap();

        assert_eq!(target.model.adapter_kind, AdapterKind::OpenAI);
    }

    #[tokio::test]
    async fn test_namespaced_model_resolves_on_unbound_client() {
        // genai-native namespacing must work without a bound client.
        let provider = GenAIProvider::new().unwrap();
        let target = provider
            .client
            .resolve_service_target("openai::phi-4-npu-ov")
            .await
            .unwrap();

        assert_eq!(target.model.adapter_kind, AdapterKind::OpenAI);
    }

    #[tokio::test]
    async fn test_from_config_binds_adapter_for_custom_model() {
        let config = Config {
            provider: Some("openai".to_string()),
            model: Some("phi-4-npu-ov".to_string()),
            ..Default::default()
        };
        let (provider, resolved) = GenAIProvider::from_config(&config, None).unwrap();
        assert_eq!(resolved.provider, "openai");
        assert_eq!(resolved.model, "phi-4-npu-ov");

        let target = provider
            .client
            .resolve_service_target(&resolved.model)
            .await
            .unwrap();
        assert_eq!(target.model.adapter_kind, AdapterKind::OpenAI);
    }

    #[test]
    fn test_from_config_model_precedence() {
        let config = Config {
            provider: Some("openai".to_string()),
            model: Some("config-model".to_string()),
            ..Default::default()
        };
        // CLI override wins over config model.
        let (_p, resolved) = GenAIProvider::from_config(&config, Some("cli-model")).unwrap();
        assert_eq!(resolved.model, "cli-model");
    }

    #[test]
    fn test_provider_from_model_namespace_detects_vertex() {
        assert_eq!(
            provider_from_model_namespace("vertex::gemini-2.5-flash"),
            Some("vertex")
        );
        assert_eq!(provider_from_model_namespace("gemini-2.5-flash"), None);
    }

    #[tokio::test]
    async fn test_from_config_uses_namespaced_vertex_model_when_provider_absent() {
        let previous_project = std::env::var("VERTEX_PROJECT_ID").ok();
        let previous_location = std::env::var("VERTEX_LOCATION").ok();
        std::env::set_var("VERTEX_PROJECT_ID", "unit-test-project");
        std::env::remove_var("VERTEX_LOCATION");

        let config = Config::default();
        let (_provider, resolved) =
            GenAIProvider::from_config(&config, Some("vertex::gemini-2.5-flash")).unwrap();
        assert_eq!(resolved.provider, "vertex");
        assert_eq!(resolved.model, "vertex::gemini-2.5-flash");
        assert!(std::env::var("VERTEX_LOCATION").is_err());
        assert_eq!(
            resolved_vertex_endpoint().as_deref(),
            Some(
                "https://aiplatform.googleapis.com/v1/projects/unit-test-project/locations/global/"
            )
        );

        match previous_project {
            Some(value) => std::env::set_var("VERTEX_PROJECT_ID", value),
            None => std::env::remove_var("VERTEX_PROJECT_ID"),
        }
        match previous_location {
            Some(value) => std::env::set_var("VERTEX_LOCATION", value),
            None => std::env::remove_var("VERTEX_LOCATION"),
        }
    }

    #[test]
    fn test_valid_vertex_segment_accepts_real_values() {
        assert_eq!(valid_vertex_segment("perspt"), Some("perspt"));
        assert_eq!(valid_vertex_segment("us-central1"), Some("us-central1"));
        assert_eq!(valid_vertex_segment("global"), Some("global"));
        assert_eq!(valid_vertex_segment("europe-west4"), Some("europe-west4"));
        assert_eq!(valid_vertex_segment("  perspt  "), Some("perspt")); // trimmed
    }

    #[test]
    fn test_valid_vertex_segment_rejects_host_redirection() {
        // Values that could alter the endpoint host or path must be rejected.
        assert_eq!(valid_vertex_segment("evil.com/"), None);
        assert_eq!(valid_vertex_segment("evil.com"), None); // '.'
        assert_eq!(valid_vertex_segment("a/b"), None);
        assert_eq!(valid_vertex_segment("a:b"), None);
        assert_eq!(valid_vertex_segment("a@b"), None);
        assert_eq!(valid_vertex_segment("a b"), None);
        assert_eq!(valid_vertex_segment(""), None);
        assert_eq!(valid_vertex_segment("   "), None);
    }

    #[test]
    fn test_resolved_vertex_endpoint_rejects_malicious_location() {
        let prev_project = std::env::var("VERTEX_PROJECT_ID").ok();
        let prev_location = std::env::var("VERTEX_LOCATION").ok();

        // A crafted location must not be interpolated into the host; it falls
        // back to the safe global endpoint instead.
        std::env::set_var("VERTEX_PROJECT_ID", "perspt");
        std::env::set_var("VERTEX_LOCATION", "evil.com/");
        assert_eq!(
            resolved_vertex_endpoint().as_deref(),
            Some("https://aiplatform.googleapis.com/v1/projects/perspt/locations/global/"),
            "malicious location must fall back to global, never redirect the host"
        );

        // An invalid project yields no endpoint override at all.
        std::env::set_var("VERTEX_PROJECT_ID", "bad/project");
        std::env::set_var("VERTEX_LOCATION", "us-central1");
        assert_eq!(resolved_vertex_endpoint(), None);

        match prev_project {
            Some(v) => std::env::set_var("VERTEX_PROJECT_ID", v),
            None => std::env::remove_var("VERTEX_PROJECT_ID"),
        }
        match prev_location {
            Some(v) => std::env::set_var("VERTEX_LOCATION", v),
            None => std::env::remove_var("VERTEX_LOCATION"),
        }
    }

    #[test]
    fn test_vertex_endpoint_base_matches_genai_vertex_shape() {
        assert_eq!(
            vertex_endpoint_base("test-project", "global"),
            "https://aiplatform.googleapis.com/v1/projects/test-project/locations/global/"
        );
        assert_eq!(
            vertex_endpoint_base("test-project", "test-location"),
            "https://test-location-aiplatform.googleapis.com/v1/projects/test-project/locations/test-location/"
        );
    }

    #[test]
    fn test_parse_gcloud_project_reads_core_project() {
        let content = r#"
        [compute]
        region = ignored-location

        [core]
        account = user@example.com
        project = test-project
        "#;
        assert_eq!(
            parse_gcloud_project(content).as_deref(),
            Some("test-project")
        );
    }

    #[tokio::test]
    async fn test_openai_base_url_overrides_bound_provider_endpoint() {
        let previous = std::env::var("OPENAI_BASE_URL").ok();
        std::env::set_var("OPENAI_BASE_URL", "https://custom.example/v1");

        let provider = GenAIProvider::new_with_config(Some("openai"), None).unwrap();
        let target = provider
            .client
            .resolve_service_target("gemma4-32b-it")
            .await
            .unwrap();

        assert_eq!(target.endpoint.base_url(), "https://custom.example/v1/");

        match previous {
            Some(value) => std::env::set_var("OPENAI_BASE_URL", value),
            None => std::env::remove_var("OPENAI_BASE_URL"),
        }
    }

    #[test]
    fn test_normalize_base_url() {
        assert_eq!(
            normalize_base_url("https://custom.example/v1"),
            "https://custom.example/v1/"
        );
        assert_eq!(
            normalize_base_url("https://custom.example/v1/"),
            "https://custom.example/v1/"
        );
    }

    #[tokio::test]
    async fn test_provider_is_clonable() {
        let provider = GenAIProvider::new().unwrap();
        let _clone1 = provider.clone();
        let _clone2 = provider.clone();
        // All clones share the same underlying state
    }
}
