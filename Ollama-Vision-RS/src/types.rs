use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for the Ollama vision client.
#[derive(Debug, Clone)]
pub struct OllamaVisionConfig {
    /// Ollama API endpoint (e.g., "http://localhost:11434")
    pub endpoint: String,
    /// Vision model name (e.g., "llava", "llava-llama3", "minicpm-v")
    pub model: String,
    /// Request timeout (default: 120s)
    pub timeout: Duration,
    /// Generation options sent to Ollama
    pub options: GenerateOptions,
}

/// Ollama generation options controlling output quality.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateOptions {
    /// Maximum tokens to generate
    pub num_predict: u32,
    /// Penalize repeated tokens (default: 1.2)
    pub repeat_penalty: f32,
    /// Window for repeat penalty (default: 128)
    pub repeat_last_n: u32,
    /// Temperature (default: None, uses Ollama default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-p sampling (default: None, uses Ollama default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
}

impl Default for GenerateOptions {
    fn default() -> Self {
        Self {
            num_predict: 512,
            repeat_penalty: 1.2,
            repeat_last_n: 128,
            temperature: None,
            top_p: None,
        }
    }
}

impl Default for OllamaVisionConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:11434".to_string(),
            model: "llava".to_string(),
            timeout: Duration::from_secs(120),
            options: GenerateOptions::default(),
        }
    }
}

impl OllamaVisionConfig {
    /// Create a new config with the given model name.
    pub fn with_model(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Default::default()
        }
    }

    /// Set the Ollama endpoint.
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Set the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the generation options.
    pub fn options(mut self, options: GenerateOptions) -> Self {
        self.options = options;
        self
    }
}

/// Tag configuration for controlling tag extraction behavior.
#[derive(Debug, Clone)]
pub struct TagOptions {
    /// Custom system prompt (overrides default)
    pub prompt: Option<String>,
    /// Request JSON format from Ollama (default: true)
    pub request_json_format: bool,
}

impl Default for TagOptions {
    fn default() -> Self {
        Self {
            prompt: None,
            request_json_format: true,
        }
    }
}

/// Caption configuration for controlling caption generation.
#[derive(Debug, Clone)]
pub struct CaptionOptions {
    /// Custom prompt (overrides default)
    pub prompt: Option<String>,
}

impl Default for CaptionOptions {
    fn default() -> Self {
        Self { prompt: None }
    }
}
