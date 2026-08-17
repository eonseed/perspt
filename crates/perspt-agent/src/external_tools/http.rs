//! MCP Streamable HTTP transport (JSON and SSE POST responses).

use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE};
use reqwest::{Method, StatusCode, Url};

use super::protocol::MCP_PROTOCOL_VERSION;
use super::protocol::{notification_value, request_value, response_result, McpTransport};
use perspt_core::ExternalToolConfig;

const SESSION_HEADER: &str = "mcp-session-id";
const PROTOCOL_HEADER: &str = "mcp-protocol-version";

#[derive(Debug)]
pub struct HttpTransport {
    client: reqwest::Client,
    url: Url,
    header_sources: std::collections::BTreeMap<String, String>,
    session_id: Option<HeaderValue>,
    next_id: u64,
}

impl HttpTransport {
    pub fn new(config: &ExternalToolConfig) -> Result<Self> {
        let url = Url::parse(config.url.as_deref().context("MCP HTTP URL missing")?)?;
        let loopback = url
            .host_str()
            .map(|host| matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "::1"))
            .unwrap_or(false);
        anyhow::ensure!(
            url.scheme() == "https" || (url.scheme() == "http" && loopback),
            "MCP Streamable HTTP requires HTTPS except for explicit loopback fixtures"
        );
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            client,
            url,
            header_sources: config.headers_from_env.clone(),
            session_id: None,
            next_id: 1,
        })
    }

    fn headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/event-stream"),
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            HeaderName::from_static(PROTOCOL_HEADER),
            HeaderValue::from_static(MCP_PROTOCOL_VERSION),
        );
        if let Some(session_id) = &self.session_id {
            headers.insert(HeaderName::from_static(SESSION_HEADER), session_id.clone());
        }
        for (destination, source) in &self.header_sources {
            let name = HeaderName::from_bytes(destination.as_bytes())?;
            let secret = std::env::var(source)
                .with_context(|| format!("MCP HTTP header source {source:?} is unavailable"))?;
            headers.insert(name, HeaderValue::from_str(&secret)?);
        }
        Ok(headers)
    }

    async fn post(
        &mut self,
        body: serde_json::Value,
        timeout: Duration,
        max_bytes: usize,
    ) -> Result<(StatusCode, Option<serde_json::Value>)> {
        let response = self
            .client
            .post(self.url.clone())
            .headers(self.headers()?)
            .json(&body)
            .timeout(timeout)
            .send()
            .await?;
        if let Some(session) = response.headers().get(SESSION_HEADER) {
            let bytes = session.as_bytes();
            anyhow::ensure!(
                !bytes.is_empty() && bytes.iter().all(|byte| (0x21..=0x7e).contains(byte)),
                "MCP server returned an invalid session identifier"
            );
            self.session_id = Some(session.clone());
        }
        let status = response.status();
        anyhow::ensure!(status.is_success(), "MCP HTTP response status {status}");
        if status == StatusCode::ACCEPTED || status == StatusCode::NO_CONTENT {
            return Ok((status, None));
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        let bytes = read_bounded(response, max_bytes).await?;
        let value = if content_type.starts_with("text/event-stream") {
            parse_sse(&bytes)?
        } else {
            serde_json::from_slice(&bytes).context("invalid MCP HTTP JSON response")?
        };
        Ok((status, Some(value)))
    }
}

async fn read_bounded(mut response: reqwest::Response, maximum: usize) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum as u64)
    {
        anyhow::bail!("MCP HTTP response exceeded byte cap");
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        anyhow::ensure!(
            bytes.len().saturating_add(chunk.len()) <= maximum,
            "MCP HTTP response exceeded byte cap"
        );
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn parse_sse(bytes: &[u8]) -> Result<serde_json::Value> {
    let text = std::str::from_utf8(bytes).context("MCP SSE response is not UTF-8")?;
    let mut data = String::new();
    for line in text.lines().chain(std::iter::once("")) {
        if let Some(value) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(value.trim_start());
        } else if line.is_empty() && !data.is_empty() {
            let value: serde_json::Value =
                serde_json::from_str(&data).context("invalid JSON in MCP SSE data event")?;
            data.clear();
            if value.get("method").is_some() && value.get("id").is_some() {
                anyhow::bail!("unsupported server-initiated MCP request on HTTP stream");
            }
            if value.get("result").is_some() || value.get("error").is_some() {
                return Ok(value);
            }
        }
    }
    anyhow::bail!("MCP SSE response contained no JSON-RPC response")
}

#[async_trait]
impl McpTransport for HttpTransport {
    async fn request(
        &mut self,
        method: &str,
        params: serde_json::Value,
        timeout: Duration,
        max_bytes: usize,
    ) -> Result<serde_json::Value> {
        let id = self.next_id;
        self.next_id += 1;
        let (_, response) = self
            .post(request_value(id, method, params), timeout, max_bytes)
            .await?;
        response_result(
            response.context("MCP HTTP request returned no response")?,
            id,
        )
    }

    async fn notify(&mut self, method: &str, params: serde_json::Value) -> Result<()> {
        let body = notification_value(method, params);
        let (status, _) = self.post(body, Duration::from_secs(30), 65_536).await?;
        anyhow::ensure!(
            matches!(status, StatusCode::ACCEPTED | StatusCode::NO_CONTENT),
            "MCP HTTP notification returned unexpected status {status}"
        );
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        if self.session_id.is_none() {
            return Ok(());
        }
        let response = self
            .client
            .request(Method::DELETE, self.url.clone())
            .headers(self.headers()?)
            .send()
            .await?;
        anyhow::ensure!(
            response.status().is_success()
                || matches!(
                    response.status(),
                    StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
                ),
            "MCP HTTP session shutdown failed with {}",
            response.status()
        );
        self.session_id = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_from_sse_data() {
        let value =
            parse_sse(b"event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n\n")
                .unwrap();
        assert_eq!(value["id"], 1);
    }

    #[test]
    fn skips_priming_and_notification_events_before_response() {
        let stream = concat!(
            "id: prime\ndata:\n\n",
            "data: {\"jsonrpc\":\"2.0\",",
            "\"method\":\"notifications/progress\"}\n\n",
            "data: {\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{}}\n\n"
        );
        let value = parse_sse(stream.as_bytes()).unwrap();
        assert_eq!(value["id"], 2);
    }

    #[test]
    fn rejects_plaintext_non_loopback() {
        let config = ExternalToolConfig {
            id: "remote".into(),
            transport: perspt_core::ExternalToolTransport::StreamableHttp,
            url: Some("http://example.com/mcp".into()),
            ..ExternalToolConfig::default()
        };
        assert!(HttpTransport::new(&config).is_err());
    }
}
