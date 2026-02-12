use crate::{error::Result, types::StageOutput, PipelineError};
use futures::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};

/// Configuration for LLM requests.
#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// Temperature (0.0 = deterministic, 1.0 = creative).
    pub temperature: f64,

    /// Maximum tokens to generate.
    pub max_tokens: u32,

    /// Enable extended thinking mode (DeepSeek R1 style `<think>` tags).
    pub thinking: bool,

    /// Request JSON format output from the model.
    pub json_mode: bool,

    /// Custom options merged into the Ollama options object.
    pub options: Option<Value>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: 2048,
            thinking: false,
            json_mode: false,
            options: None,
        }
    }
}

impl LlmConfig {
    pub fn with_temperature(mut self, temp: f64) -> Self {
        self.temperature = temp;
        self
    }

    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = tokens;
        self
    }

    pub fn with_thinking(mut self, enabled: bool) -> Self {
        self.thinking = enabled;
        self
    }

    pub fn with_json_mode(mut self, enabled: bool) -> Self {
        self.json_mode = enabled;
        self
    }
}

/// Call LLM with `/api/generate` and parse the response into `T`.
pub async fn call_llm<T>(
    client: &Client,
    endpoint: &str,
    model: &str,
    prompt: &str,
    config: &LlmConfig,
) -> Result<StageOutput<T>>
where
    T: serde::de::DeserializeOwned,
{
    let mut body = json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
        "options": {
            "temperature": config.temperature,
            "num_predict": config.max_tokens,
        },
    });

    if config.thinking {
        body["options"]["extended_thinking"] = json!(true);
    }

    if config.json_mode {
        body["format"] = json!("json");
    }

    merge_custom_options(&mut body, config);

    let url = format!("{}/api/generate", endpoint.trim_end_matches('/'));
    let resp =
        client.post(&url).json(&body).send().await.map_err(|e| {
            PipelineError::Other(format!("Failed to connect to LLM at {}: {}", url, e))
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(PipelineError::Other(format!(
            "LLM returned error {}: {}",
            status, text
        )));
    }

    let json_response: Value = resp.json().await?;
    let raw_response = json_response
        .get("response")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let (thinking, cleaned_response) = extract_thinking(&raw_response);
    let output: T = parse_output(&cleaned_response)?;

    Ok(StageOutput {
        output,
        thinking,
        raw_response,
    })
}

/// Call LLM with `/api/chat` (supports system messages) and parse the response.
pub async fn call_llm_chat<T>(
    client: &Client,
    endpoint: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    config: &LlmConfig,
) -> Result<StageOutput<T>>
where
    T: serde::de::DeserializeOwned,
{
    let mut messages = vec![];
    if !system_prompt.is_empty() {
        messages.push(json!({"role": "system", "content": system_prompt}));
    }
    messages.push(json!({"role": "user", "content": user_prompt}));

    let mut body = json!({
        "model": model,
        "messages": messages,
        "stream": false,
        "options": {
            "temperature": config.temperature,
            "num_predict": config.max_tokens,
        },
    });

    if config.thinking {
        body["options"]["extended_thinking"] = json!(true);
    }

    if config.json_mode {
        body["format"] = json!("json");
    }

    merge_custom_options(&mut body, config);

    let url = format!("{}/api/chat", endpoint.trim_end_matches('/'));
    let resp =
        client.post(&url).json(&body).send().await.map_err(|e| {
            PipelineError::Other(format!("Failed to connect to LLM at {}: {}", url, e))
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(PipelineError::Other(format!(
            "LLM returned error {}: {}",
            status, text
        )));
    }

    let json_response: Value = resp.json().await?;
    let raw_response = json_response
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let (thinking, cleaned_response) = extract_thinking(&raw_response);
    let output: T = parse_output(&cleaned_response)?;

    Ok(StageOutput {
        output,
        thinking,
        raw_response,
    })
}

/// Call LLM with `/api/generate` in streaming mode, invoking `on_chunk` for each token.
pub async fn call_llm_streaming<T, F>(
    client: &Client,
    endpoint: &str,
    model: &str,
    prompt: &str,
    config: &LlmConfig,
    mut on_chunk: F,
) -> Result<StageOutput<T>>
where
    T: serde::de::DeserializeOwned,
    F: FnMut(&str),
{
    let mut body = json!({
        "model": model,
        "prompt": prompt,
        "stream": true,
        "options": {
            "temperature": config.temperature,
            "num_predict": config.max_tokens,
        },
    });

    if config.thinking {
        body["options"]["extended_thinking"] = json!(true);
    }

    if config.json_mode {
        body["format"] = json!("json");
    }

    merge_custom_options(&mut body, config);

    let url = format!("{}/api/generate", endpoint.trim_end_matches('/'));
    let resp =
        client.post(&url).json(&body).send().await.map_err(|e| {
            PipelineError::Other(format!("Failed to connect to LLM at {}: {}", url, e))
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(PipelineError::Other(format!(
            "LLM returned error {}: {}",
            status, text
        )));
    }

    let mut stream = resp.bytes_stream();
    let mut accumulated = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(PipelineError::Request)?;
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if let Ok(json) = serde_json::from_str::<Value>(line) {
                if let Some(response) = json.get("response").and_then(|v| v.as_str()) {
                    accumulated.push_str(response);
                    on_chunk(response);
                }
            }
        }
    }

    let (thinking, cleaned) = extract_thinking(&accumulated);
    let output: T = parse_output(&cleaned)?;

    Ok(StageOutput {
        output,
        thinking,
        raw_response: accumulated,
    })
}

/// Extract `<think>...</think>` blocks from a response (DeepSeek R1 style).
fn extract_thinking(text: &str) -> (Option<String>, String) {
    let think_start = "<think>";
    let think_end = "</think>";

    if let Some(start_idx) = text.find(think_start) {
        if let Some(end_idx) = text.find(think_end) {
            let thinking = text[start_idx + think_start.len()..end_idx]
                .trim()
                .to_string();
            let mut cleaned = String::new();
            cleaned.push_str(&text[..start_idx]);
            cleaned.push_str(&text[end_idx + think_end.len()..]);
            let cleaned = cleaned.trim().to_string();
            let thinking = if thinking.is_empty() {
                None
            } else {
                Some(thinking)
            };
            return (thinking, cleaned);
        }
    }

    (None, text.to_string())
}

/// Parse LLM output text as `T`, with defensive JSON extraction.
fn parse_output<T: serde::de::DeserializeOwned>(text: &str) -> Result<T> {
    let trimmed = text.trim();

    // Try direct parse first
    if let Ok(val) = serde_json::from_str::<T>(trimmed) {
        return Ok(val);
    }

    // Try extracting JSON from markdown code blocks
    if let Some(json_str) = extract_json_block(trimmed) {
        if let Ok(val) = serde_json::from_str::<T>(&json_str) {
            return Ok(val);
        }
    }

    // Try finding first { or [ and parsing from there
    if let Some(idx) = trimmed.find('{').or_else(|| trimmed.find('[')) {
        let candidate = &trimmed[idx..];
        if let Ok(val) = serde_json::from_str::<T>(candidate) {
            return Ok(val);
        }
        // Try finding matching closing brace/bracket
        let open = candidate.as_bytes()[0];
        let close = if open == b'{' { b'}' } else { b']' };
        if let Some(end) = candidate.rfind(close as char) {
            let substr = &candidate[..=end];
            if let Ok(val) = serde_json::from_str::<T>(substr) {
                return Ok(val);
            }
        }
    }

    Err(PipelineError::Other(format!(
        "Failed to parse LLM output as expected type. Raw text: {}",
        &trimmed[..trimmed.len().min(200)]
    )))
}

/// Extract JSON from ```json ... ``` code blocks.
fn extract_json_block(text: &str) -> Option<String> {
    let markers = ["```json", "```JSON", "```"];
    for marker in markers {
        if let Some(start) = text.find(marker) {
            let content_start = start + marker.len();
            if let Some(end) = text[content_start..].find("```") {
                return Some(text[content_start..content_start + end].trim().to_string());
            }
        }
    }
    None
}

/// Merge custom options into the body's options object.
fn merge_custom_options(body: &mut Value, config: &LlmConfig) {
    if let Some(ref opts) = config.options {
        if let Some(options) = body["options"].as_object_mut() {
            if let Some(custom) = opts.as_object() {
                for (k, v) in custom {
                    options.insert(k.clone(), v.clone());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_thinking_present() {
        let text = "Before <think>my reasoning here</think> after";
        let (thinking, cleaned) = extract_thinking(text);
        assert_eq!(thinking, Some("my reasoning here".to_string()));
        assert_eq!(cleaned, "Before  after");
    }

    #[test]
    fn test_extract_thinking_absent() {
        let text = "no thinking tags here";
        let (thinking, cleaned) = extract_thinking(text);
        assert!(thinking.is_none());
        assert_eq!(cleaned, "no thinking tags here");
    }

    #[test]
    fn test_extract_thinking_empty() {
        let text = "<think>  </think>actual content";
        let (thinking, cleaned) = extract_thinking(text);
        assert!(thinking.is_none());
        assert_eq!(cleaned, "actual content");
    }

    #[test]
    fn test_parse_output_direct_json() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct T {
            value: String,
        }
        let result: T = parse_output(r#"{"value": "hello"}"#).unwrap();
        assert_eq!(result.value, "hello");
    }

    #[test]
    fn test_parse_output_markdown_block() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct T {
            x: i32,
        }
        let text = "Here is the result:\n```json\n{\"x\": 42}\n```\nDone.";
        let result: T = parse_output(text).unwrap();
        assert_eq!(result.x, 42);
    }

    #[test]
    fn test_parse_output_embedded_json() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct T {
            name: String,
        }
        let text = "Sure! Here is the output: {\"name\": \"test\"} hope that helps.";
        let result: T = parse_output(text).unwrap();
        assert_eq!(result.name, "test");
    }

    #[test]
    fn test_parse_output_failure() {
        #[derive(Debug, serde::Deserialize)]
        struct T {
            _x: i32,
        }
        let result = parse_output::<T>("not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_json_block() {
        let text = "text\n```json\n{\"a\":1}\n```\nmore";
        assert_eq!(extract_json_block(text), Some("{\"a\":1}".to_string()));
    }

    #[test]
    fn test_extract_json_block_none() {
        assert_eq!(extract_json_block("no code block"), None);
    }

    #[test]
    fn test_llm_config_defaults() {
        let config = LlmConfig::default();
        assert_eq!(config.temperature, 0.7);
        assert_eq!(config.max_tokens, 2048);
        assert!(!config.thinking);
        assert!(!config.json_mode);
        assert!(config.options.is_none());
    }

    #[test]
    fn test_llm_config_builder() {
        let config = LlmConfig::default()
            .with_temperature(0.3)
            .with_max_tokens(4096)
            .with_thinking(true)
            .with_json_mode(true);
        assert_eq!(config.temperature, 0.3);
        assert_eq!(config.max_tokens, 4096);
        assert!(config.thinking);
        assert!(config.json_mode);
    }
}
