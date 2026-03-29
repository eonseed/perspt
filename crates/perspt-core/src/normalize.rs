//! Provider-Neutral Output Normalization
//!
//! PSP-5 Phase 4: Extracts structured content (JSON objects, JSON arrays) from
//! raw LLM responses regardless of provider-specific formatting quirks.
//!
//! Supported extraction strategies (tried in order):
//! 1. Fenced JSON code block: ```json ... ```
//! 2. Generic fenced code block: ``` ... ``` containing JSON
//! 3. Direct JSON: response body starts with `{` or `[`
//! 4. Embedded JSON: first `{` to last matching `}` in wrapper text
//!
//! The module is provider-agnostic by design. Provider family classification
//! is available for diagnostics and telemetry but does not change extraction
//! behavior.

use serde::de::DeserializeOwned;

/// Provider family for diagnostics and telemetry.
///
/// Does not affect extraction semantics — all providers go through the same
/// normalization pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderFamily {
    OpenAI,
    Anthropic,
    Gemini,
    Groq,
    Cohere,
    XAI,
    DeepSeek,
    Ollama,
    Unknown,
}

impl ProviderFamily {
    /// Classify a provider family from a model name string.
    ///
    /// Uses prefix heuristics; returns `Unknown` when the model name does not
    /// match any known pattern.
    pub fn from_model_name(model: &str) -> Self {
        let lower = model.to_lowercase();
        if lower.starts_with("gpt-")
            || lower.starts_with("o1-")
            || lower.starts_with("o3-")
            || lower.starts_with("o4-")
            || lower.contains("openai")
        {
            ProviderFamily::OpenAI
        } else if lower.starts_with("claude") || lower.contains("anthropic") {
            ProviderFamily::Anthropic
        } else if lower.starts_with("gemini") || lower.contains("google") {
            ProviderFamily::Gemini
        } else if lower.contains("groq")
            || lower.starts_with("llama")
            || lower.starts_with("mixtral")
        {
            ProviderFamily::Groq
        } else if lower.starts_with("command") || lower.contains("cohere") {
            ProviderFamily::Cohere
        } else if lower.starts_with("grok") || lower.contains("xai") {
            ProviderFamily::XAI
        } else if lower.starts_with("deepseek") {
            ProviderFamily::DeepSeek
        } else if lower.contains("ollama") {
            ProviderFamily::Ollama
        } else {
            ProviderFamily::Unknown
        }
    }
}

impl std::fmt::Display for ProviderFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderFamily::OpenAI => write!(f, "openai"),
            ProviderFamily::Anthropic => write!(f, "anthropic"),
            ProviderFamily::Gemini => write!(f, "gemini"),
            ProviderFamily::Groq => write!(f, "groq"),
            ProviderFamily::Cohere => write!(f, "cohere"),
            ProviderFamily::XAI => write!(f, "xai"),
            ProviderFamily::DeepSeek => write!(f, "deepseek"),
            ProviderFamily::Ollama => write!(f, "ollama"),
            ProviderFamily::Unknown => write!(f, "unknown"),
        }
    }
}

/// Which extraction strategy succeeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractionMethod {
    /// Found inside a ```json ... ``` fence.
    FencedJson,
    /// Found inside a generic ``` ... ``` fence containing JSON.
    GenericFence,
    /// Response body started directly with `{` or `[`.
    DirectJson,
    /// Extracted from first `{` to last balanced `}` in wrapper text.
    EmbeddedJson,
}

impl std::fmt::Display for ExtractionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtractionMethod::FencedJson => write!(f, "fenced_json"),
            ExtractionMethod::GenericFence => write!(f, "generic_fence"),
            ExtractionMethod::DirectJson => write!(f, "direct_json"),
            ExtractionMethod::EmbeddedJson => write!(f, "embedded_json"),
        }
    }
}

/// Result of a successful normalization.
#[derive(Debug, Clone)]
pub struct NormalizedOutput {
    /// The extracted JSON body (trimmed, ready for `serde_json::from_str`).
    pub json_body: String,
    /// How the JSON was extracted.
    pub method: ExtractionMethod,
}

/// Error returned when normalization cannot extract a JSON body.
#[derive(Debug, Clone)]
pub struct NormalizationError {
    /// Human-readable reason.
    pub reason: String,
    /// Byte length of the raw input that was inspected.
    pub input_len: usize,
}

impl std::fmt::Display for NormalizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "normalization failed (input {} bytes): {}",
            self.input_len, self.reason
        )
    }
}

impl std::error::Error for NormalizationError {}

/// Extract a JSON body from a raw LLM response.
///
/// Tries extraction strategies in order of specificity:
/// 1. Fenced JSON (`\`\`\`json`)
/// 2. Generic fence (`\`\`\``) whose content parses as JSON
/// 3. Direct JSON (trimmed input starts with `{` or `[`)
/// 4. Embedded JSON (first `{` to last balanced `}`)
///
/// Returns the extracted body and the method used, or an error if no JSON
/// could be found.
pub fn extract_json(raw: &str) -> Result<NormalizedOutput, NormalizationError> {
    let trimmed = raw.trim();

    if trimmed.is_empty() {
        return Err(NormalizationError {
            reason: "empty input".to_string(),
            input_len: 0,
        });
    }

    // Strategy 1: fenced JSON code block
    if let Some(body) = extract_fenced_json(trimmed) {
        return Ok(NormalizedOutput {
            json_body: body,
            method: ExtractionMethod::FencedJson,
        });
    }

    // Strategy 2: generic fenced code block containing JSON
    if let Some(body) = extract_generic_fence_json(trimmed) {
        return Ok(NormalizedOutput {
            json_body: body,
            method: ExtractionMethod::GenericFence,
        });
    }

    // Strategy 3: direct JSON
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return Ok(NormalizedOutput {
            json_body: trimmed.to_string(),
            method: ExtractionMethod::DirectJson,
        });
    }

    // Strategy 4: embedded JSON via balanced brace matching
    if let Some(body) = extract_embedded_json(trimmed) {
        return Ok(NormalizedOutput {
            json_body: body,
            method: ExtractionMethod::EmbeddedJson,
        });
    }

    Err(NormalizationError {
        reason: "no JSON object or array found in response".to_string(),
        input_len: raw.len(),
    })
}

/// Convenience: extract JSON and deserialize into `T` in one step.
pub fn extract_and_deserialize<T: DeserializeOwned>(
    raw: &str,
) -> Result<(T, ExtractionMethod), NormalizationError> {
    let output = extract_json(raw)?;
    match serde_json::from_str::<T>(&output.json_body) {
        Ok(value) => Ok((value, output.method)),
        Err(e) => Err(NormalizationError {
            reason: format!(
                "JSON extracted via {} but deserialization failed: {}",
                output.method, e
            ),
            input_len: raw.len(),
        }),
    }
}

// ---------------------------------------------------------------------------
// Internal extraction helpers
// ---------------------------------------------------------------------------

/// Extract content from a ```json ... ``` fence.
fn extract_fenced_json(input: &str) -> Option<String> {
    let marker = "```json";
    let start_idx = input.find(marker)?;
    let body_start = start_idx + marker.len();

    // Skip optional whitespace/newline after ```json
    let remaining = &input[body_start..];
    let remaining = remaining.strip_prefix('\n').unwrap_or(remaining);

    let end_offset = remaining.find("```")?;
    let body = remaining[..end_offset].trim();
    if body.is_empty() {
        return None;
    }
    Some(body.to_string())
}

/// Extract content from a generic ``` ... ``` fence that looks like JSON.
fn extract_generic_fence_json(input: &str) -> Option<String> {
    let marker = "```";
    let start_idx = input.find(marker)?;
    let after_marker = start_idx + marker.len();

    // Skip language identifier if present (anything until the next newline)
    let remaining = &input[after_marker..];
    let body_start = remaining.find('\n').map(|n| n + 1).unwrap_or(0);
    let remaining = &remaining[body_start..];

    let end_offset = remaining.find("```")?;
    let body = remaining[..end_offset].trim();

    // Only return if it plausibly starts with JSON
    if body.starts_with('{') || body.starts_with('[') {
        Some(body.to_string())
    } else {
        None
    }
}

/// Extract the outermost balanced `{ ... }` from text that may have wrapper
/// prose before and/or after the JSON object.
fn extract_embedded_json(input: &str) -> Option<String> {
    let open = input.find('{')?;
    // Walk forward with a brace‐depth counter to find the matching close
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape_next = false;
    let mut close = None;

    for (i, ch) in input[open..].char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match ch {
            '\\' if in_string => {
                escape_next = true;
            }
            '"' => {
                in_string = !in_string;
            }
            '{' if !in_string => {
                depth += 1;
            }
            '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    close = Some(open + i);
                    break;
                }
            }
            _ => {}
        }
    }

    let close = close?;
    let body = &input[open..=close];
    Some(body.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- extract_json --------------------------------------------------------

    #[test]
    fn test_direct_json_object() {
        let raw = r#"{"tasks": [{"id": "1"}]}"#;
        let out = extract_json(raw).unwrap();
        assert_eq!(out.method, ExtractionMethod::DirectJson);
        assert_eq!(out.json_body, raw);
    }

    #[test]
    fn test_direct_json_array() {
        let raw = r#"[{"id": 1}]"#;
        let out = extract_json(raw).unwrap();
        assert_eq!(out.method, ExtractionMethod::DirectJson);
    }

    #[test]
    fn test_fenced_json() {
        let raw = "Here is the plan:\n```json\n{\"tasks\": []}\n```\nDone.";
        let out = extract_json(raw).unwrap();
        assert_eq!(out.method, ExtractionMethod::FencedJson);
        assert_eq!(out.json_body, "{\"tasks\": []}");
    }

    #[test]
    fn test_generic_fence_with_json() {
        let raw = "Result:\n```\n{\"artifacts\": []}\n```";
        let out = extract_json(raw).unwrap();
        assert_eq!(out.method, ExtractionMethod::GenericFence);
        assert_eq!(out.json_body, "{\"artifacts\": []}");
    }

    #[test]
    fn test_generic_fence_with_language_hint() {
        let raw = "```rust\nfn main() {}\n```";
        // Not JSON — should fall through to embedded, which also won't match a valid JSON object
        // because the braces are inside a Rust function, not a JSON root.
        // Expect failure.
        let result = extract_json(raw);
        // It may extract the embedded braces; the important thing is that
        // generic_fence_json correctly rejected non-JSON content.
        if let Ok(out) = &result {
            assert_ne!(out.method, ExtractionMethod::GenericFence);
        }
    }

    #[test]
    fn test_embedded_json_with_wrapper_text() {
        let raw = "Sure! Here is the bundle:\n{\"artifacts\": [{\"path\": \"main.rs\", \"operation\": \"write\", \"content\": \"fn main() {}\"}]}\nLet me know if you need changes.";
        let out = extract_json(raw).unwrap();
        assert_eq!(out.method, ExtractionMethod::EmbeddedJson);
        assert!(out.json_body.starts_with('{'));
        assert!(out.json_body.ends_with('}'));
    }

    #[test]
    fn test_embedded_json_with_nested_braces() {
        let raw = "Plan: {\"a\": {\"b\": {\"c\": 1}}} end";
        let out = extract_json(raw).unwrap();
        assert_eq!(out.method, ExtractionMethod::EmbeddedJson);
        assert_eq!(out.json_body, "{\"a\": {\"b\": {\"c\": 1}}}");
    }

    #[test]
    fn test_embedded_json_with_strings_containing_braces() {
        let raw = r#"Output: {"msg": "hello { world }"} done"#;
        let out = extract_json(raw).unwrap();
        assert_eq!(out.method, ExtractionMethod::EmbeddedJson);
        assert_eq!(out.json_body, r#"{"msg": "hello { world }"}"#);
    }

    #[test]
    fn test_empty_input() {
        let result = extract_json("");
        assert!(result.is_err());
    }

    #[test]
    fn test_no_json_at_all() {
        let result = extract_json("This is just a plain text response with no JSON.");
        assert!(result.is_err());
    }

    #[test]
    fn test_fenced_json_takes_priority_over_embedded() {
        let raw = "Preamble {\"stray\": 1}\n```json\n{\"real\": 2}\n```";
        let out = extract_json(raw).unwrap();
        assert_eq!(out.method, ExtractionMethod::FencedJson);
        assert_eq!(out.json_body, "{\"real\": 2}");
    }

    // -- extract_and_deserialize ---------------------------------------------

    #[test]
    fn test_extract_and_deserialize_ok() {
        #[derive(serde::Deserialize)]
        struct Simple {
            value: i32,
        }
        let raw = "```json\n{\"value\": 42}\n```";
        let (obj, method): (Simple, _) = extract_and_deserialize(raw).unwrap();
        assert_eq!(obj.value, 42);
        assert_eq!(method, ExtractionMethod::FencedJson);
    }

    #[test]
    fn test_extract_and_deserialize_bad_schema() {
        #[derive(Debug, serde::Deserialize)]
        struct Strict {
            #[allow(dead_code)]
            required_field: String,
        }
        let raw = "{\"other\": 1}";
        let result: Result<(Strict, _), _> = extract_and_deserialize(raw);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.reason.contains("deserialization failed"));
    }

    // -- ProviderFamily ------------------------------------------------------

    #[test]
    fn test_provider_family_classification() {
        assert_eq!(
            ProviderFamily::from_model_name("gpt-4o"),
            ProviderFamily::OpenAI
        );
        assert_eq!(
            ProviderFamily::from_model_name("claude-opus-4-20250514"),
            ProviderFamily::Anthropic
        );
        assert_eq!(
            ProviderFamily::from_model_name("gemini-2.5-pro"),
            ProviderFamily::Gemini
        );
        assert_eq!(
            ProviderFamily::from_model_name("deepseek-r1"),
            ProviderFamily::DeepSeek
        );
        assert_eq!(
            ProviderFamily::from_model_name("my-custom-model"),
            ProviderFamily::Unknown
        );
    }

    #[test]
    fn test_extract_json_with_nested_code_fence() {
        // LLMs often wrap JSON in markdown code fences with extra prose
        let raw = r#"
Here is the plan I've created for you:

```json
{
  "steps": [
    {"id": "s1", "action": "create_file", "path": "src/lib.rs"},
    {"id": "s2", "action": "run_tests", "path": "."}
  ],
  "description": "Create and verify a new library"
}
```

Let me know if you'd like any changes.
"#;
        let output = extract_json(raw).unwrap();
        assert_eq!(output.method, ExtractionMethod::FencedJson);
        assert!(output.json_body.contains("create_file"));
        assert!(output.json_body.contains("run_tests"));
    }

    #[test]
    fn test_extract_and_deserialize_realistic_plan() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct Step {
            id: String,
            action: String,
        }
        #[derive(Debug, serde::Deserialize)]
        struct Plan {
            steps: Vec<Step>,
        }

        let raw = r#"Sure! ```json
{"steps": [{"id": "1", "action": "lint"}, {"id": "2", "action": "test"}]}
```"#;

        let (plan, method): (Plan, _) = extract_and_deserialize(raw).unwrap();
        assert_eq!(method, ExtractionMethod::FencedJson);
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].action, "lint");
        assert_eq!(plan.steps[1].action, "test");
    }
}
