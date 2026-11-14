//! Scoped `genai::Client` construction and provider endpoint resolution.
//!
//! Resolves per-provider credentials, custom base URLs, and Vertex AI routing
//! directly on each `genai::Client` instance via `AuthResolver` and
//! `ServiceTargetResolver`, avoiding process-global `std::env` mutation.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use anyhow::Result;
use genai::adapter::AdapterKind;
use genai::resolver::{AuthData, AuthResolver, Endpoint, ServiceTargetResolver};
use genai::{Client, ModelIden, ServiceTarget};

/// Per-client configuration used to construct an isolated `genai::Client`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderClientOptions {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub vertex_project_id: Option<String>,
    pub vertex_location: Option<String>,
    pub vertex_api_version: Option<String>,
}

/// Build an isolated `genai::Client` for a configured provider adapter.
pub(crate) fn build_client_for_provider(
    provider_type: Option<&str>,
    options: &ProviderClientOptions,
) -> Result<Client> {
    let adapter_kind = provider_type.and_then(|provider| match str_to_adapter_kind(provider) {
        Ok(adapter_kind) => Some(adapter_kind),
        Err(_) => {
            log::warn!("Unknown provider type for genai client: {provider}");
            None
        }
    });

    if provider_type.is_some_and(|p| p.eq_ignore_ascii_case("vertex")) {
        return build_vertex_client(options);
    }
    build_bound_client(adapter_kind, provider_type, options)
}

fn build_bound_client(
    adapter_kind: Option<AdapterKind>,
    provider_type: Option<&str>,
    options: &ProviderClientOptions,
) -> Result<Client> {
    let mut builder = Client::builder();
    if let Some(kind) = adapter_kind {
        builder = builder.with_adapter_kind(kind);
    }

    if let Some(key) = non_empty_trimmed(options.api_key.as_deref()) {
        let key = key.to_string();
        let auth_resolver = AuthResolver::from_resolver_fn(
            move |_model: ModelIden| -> genai::resolver::Result<Option<AuthData>> {
                Ok(Some(AuthData::from_single(key.clone())))
            },
        );
        builder = builder.with_auth_resolver(auth_resolver);
    }

    let base_url = non_empty_trimmed(options.base_url.as_deref())
        .map(str::to_string)
        .or_else(|| provider_type.and_then(provider_base_url_from_env));
    if let (Some(kind), Some(url)) = (adapter_kind, base_url) {
        let endpoint = normalize_base_url(&url);
        let target_resolver = ServiceTargetResolver::from_resolver_fn(
            move |mut target: ServiceTarget| -> genai::resolver::Result<ServiceTarget> {
                if target.model.adapter_kind == kind {
                    target.endpoint = Endpoint::from_owned(endpoint.clone());
                }
                Ok(target)
            },
        );
        builder = builder.with_service_target_resolver(target_resolver);
    }

    Ok(builder.build()?)
}

fn build_vertex_client(options: &ProviderClientOptions) -> Result<Client> {
    let explicit_token = non_empty_trimmed(options.api_key.as_deref()).map(str::to_string);
    let resolver = AuthResolver::from_resolver_async_fn(
        move |_model: ModelIden| -> Pin<
            Box<dyn Future<Output = genai::resolver::Result<Option<AuthData>>> + Send>,
        > {
            let explicit_token = explicit_token.clone();
            Box::pin(async move {
                if let Some(token) = explicit_token {
                    return Ok(Some(AuthData::from_single(token)));
                }
                if let Ok(token) = std::env::var("VERTEX_API_KEY")
                    && !token.trim().is_empty()
                {
                    return Ok(Some(AuthData::from_single(token)));
                }
                let provider = gcp_auth::provider().await.map_err(|e| {
                    genai::resolver::Error::Custom(format!(
                        "Vertex ADC provider init failed \
                         (run `gcloud auth application-default login`): {e}"
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

    if let Some(endpoint) = resolve_vertex_endpoint(options) {
        builder = builder.with_service_target_resolver_fn(move |mut target: ServiceTarget| {
            target.endpoint = Endpoint::from_owned(endpoint.clone());
            Ok(target)
        });
    }

    Ok(builder.build()?)
}

pub(crate) fn resolve_vertex_endpoint(options: &ProviderClientOptions) -> Option<String> {
    let project_raw = non_empty_trimmed(options.vertex_project_id.as_deref())
        .map(str::to_string)
        .or_else(vertex_project_from_env)
        .or_else(read_gcloud_project)?;
    let Some(project) = valid_vertex_segment(&project_raw).map(str::to_string) else {
        log::warn!(
            "Ignoring Vertex project ID for endpoint construction: must be non-empty and \
             contain only ASCII letters, digits, and hyphens"
        );
        return None;
    };

    let raw_location = non_empty_trimmed(options.vertex_location.as_deref())
        .map(str::to_string)
        .or_else(|| {
            std::env::var("VERTEX_LOCATION")
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        });
    let location = match raw_location {
        Some(raw) => match valid_vertex_segment(&raw) {
            Some(valid) => valid.to_string(),
            None => {
                log::warn!(
                    "Ignoring invalid Vertex location (must contain only ASCII letters, digits, \
                     and hyphens); falling back to 'global'"
                );
                "global".to_string()
            }
        },
        None => "global".to_string(),
    };

    let api_version = resolve_vertex_api_version(options);
    Some(vertex_endpoint_base(&project, &location, &api_version))
}

fn resolve_vertex_api_version(options: &ProviderClientOptions) -> String {
    non_empty_trimmed(options.vertex_api_version.as_deref())
        .and_then(valid_vertex_segment)
        .map(str::to_string)
        .or_else(|| {
            std::env::var("VERTEX_API_VERSION")
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty() && valid_vertex_segment(v).is_some())
        })
        .unwrap_or_else(|| "v1beta1".to_string())
}

pub(crate) fn vertex_endpoint_base(project: &str, location: &str, version: &str) -> String {
    let project = project.trim();
    let location = location.trim();
    if location.eq_ignore_ascii_case("global") {
        format!("https://aiplatform.googleapis.com/{version}/projects/{project}/locations/global/")
    } else {
        format!(
            "https://{location}-aiplatform.googleapis.com/{version}/\
             projects/{project}/locations/{location}/"
        )
    }
}

pub(crate) fn vertex_project_from_env() -> Option<String> {
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
    if let Ok(dir) = std::env::var("CLOUDSDK_CONFIG")
        && !dir.trim().is_empty()
    {
        return Some(PathBuf::from(dir.trim()));
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

pub(crate) fn parse_gcloud_project(content: &str) -> Option<String> {
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

/// Validate a Vertex project/location segment before interpolation into the URL.
pub(crate) fn valid_vertex_segment(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-'))
    .then_some(trimmed)
}

pub(crate) fn provider_base_url_env_var(provider: &str) -> Option<&'static str> {
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

pub(crate) fn provider_base_url_from_env(provider: &str) -> Option<String> {
    let env_var = provider_base_url_env_var(provider)?;
    std::env::var(env_var)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn normalize_base_url(base_url: &str) -> String {
    if base_url.ends_with('/') {
        base_url.to_string()
    } else {
        format!("{base_url}/")
    }
}

/// Convert a provider string to `genai::adapter::AdapterKind`.
pub(crate) fn str_to_adapter_kind(provider: &str) -> Result<AdapterKind> {
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
        _ => Err(anyhow::anyhow!("Unsupported provider: {provider}")),
    }
}

fn non_empty_trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|s| !s.is_empty())
}
