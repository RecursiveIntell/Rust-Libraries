use crate::parser;
use crate::types::{CaptionOptions, OllamaVisionConfig};
use reqwest::Client;
use serde_json::json;
use std::path::Path;

const DEFAULT_CAPTION_PROMPT: &str = r#"Describe this image in 1-2 sentences. Focus on the main subject, art style, composition, lighting, and mood. Be specific and concise. Do not start with "This image shows" or "The image depicts". Just describe what you see directly."#;

/// Generate a caption for an image using an Ollama vision model.
///
/// Returns a cleaned caption string with `<think>` blocks stripped.
///
/// # Errors
///
/// Returns an error if:
/// - The image file cannot be read
/// - The Ollama endpoint is unreachable
/// - The model returns an error or empty caption
pub async fn caption_image(
    client: &Client,
    config: &OllamaVisionConfig,
    image_path: &Path,
    options: &CaptionOptions,
) -> Result<String, CaptionError> {
    let image_b64 = read_image_base64(image_path)?;

    let prompt = options
        .prompt
        .as_deref()
        .unwrap_or(DEFAULT_CAPTION_PROMPT);

    let body = json!({
        "model": config.model,
        "prompt": prompt,
        "images": [image_b64],
        "stream": false,
        "options": config.options,
    });

    let url = format!("{}/api/generate", config.endpoint);
    let resp = client
        .post(&url)
        .timeout(config.timeout)
        .json(&body)
        .send()
        .await
        .map_err(|e| CaptionError::Connection(config.endpoint.clone(), e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        return Err(CaptionError::OllamaError(status, text));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| CaptionError::InvalidResponse(e.to_string()))?;

    let raw = json
        .get("response")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let caption = parser::strip_think_tags(raw).trim().to_string();

    if caption.is_empty() {
        return Err(CaptionError::EmptyCaption);
    }

    Ok(caption)
}

/// Caption an image from raw base64-encoded bytes (no file I/O).
pub async fn caption_image_base64(
    client: &Client,
    config: &OllamaVisionConfig,
    image_b64: &str,
    options: &CaptionOptions,
) -> Result<String, CaptionError> {
    let prompt = options
        .prompt
        .as_deref()
        .unwrap_or(DEFAULT_CAPTION_PROMPT);

    let body = json!({
        "model": config.model,
        "prompt": prompt,
        "images": [image_b64],
        "stream": false,
        "options": config.options,
    });

    let url = format!("{}/api/generate", config.endpoint);
    let resp = client
        .post(&url)
        .timeout(config.timeout)
        .json(&body)
        .send()
        .await
        .map_err(|e| CaptionError::Connection(config.endpoint.clone(), e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        return Err(CaptionError::OllamaError(status, text));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| CaptionError::InvalidResponse(e.to_string()))?;

    let raw = json
        .get("response")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let caption = parser::strip_think_tags(raw).trim().to_string();

    if caption.is_empty() {
        return Err(CaptionError::EmptyCaption);
    }

    Ok(caption)
}

/// Errors that can occur during image captioning.
#[derive(Debug, thiserror::Error)]
pub enum CaptionError {
    #[error("Cannot connect to Ollama at {0}: {1}")]
    Connection(String, String),

    #[error("Ollama returned HTTP {0}: {1}")]
    OllamaError(u16, String),

    #[error("Invalid response from Ollama: {0}")]
    InvalidResponse(String),

    #[error("Failed to read image: {0}")]
    ImageRead(String),

    #[error("Ollama returned empty caption")]
    EmptyCaption,
}

fn read_image_base64(path: &Path) -> Result<String, CaptionError> {
    let bytes = std::fs::read(path)
        .map_err(|e| CaptionError::ImageRead(format!("{}: {}", path.display(), e)))?;
    Ok(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &bytes,
    ))
}
