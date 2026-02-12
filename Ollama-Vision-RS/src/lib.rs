//! # ollama-vision
//!
//! Robust Ollama vision model toolkit for image tagging and captioning.
//!
//! ## Features
//!
//! - **Image tagging** with a 7-strategy response parser that handles every
//!   common LLM output format (JSON arrays, code blocks, `<think>` tags,
//!   JSON objects, numbered lists, comma-separated text)
//! - **Image captioning** with automatic `<think>` block stripping
//! - **Works with any Ollama vision model** (llava, minicpm-v, llama3.2-vision, etc.)
//! - **Base64 API** for in-memory images (no file I/O required)
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use ollama_vision::{OllamaVisionConfig, TagOptions, CaptionOptions};
//! use std::path::Path;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = OllamaVisionConfig::with_model("llava");
//!     let client = reqwest::Client::new();
//!
//!     // Tag an image
//!     let tags = ollama_vision::tag_image(
//!         &client, &config,
//!         Path::new("photo.jpg"),
//!         &TagOptions::default(),
//!     ).await?;
//!     println!("Tags: {:?}", tags);
//!
//!     // Caption an image
//!     let caption = ollama_vision::caption_image(
//!         &client, &config,
//!         Path::new("photo.jpg"),
//!         &CaptionOptions::default(),
//!     ).await?;
//!     println!("Caption: {}", caption);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Parsing Robustness
//!
//! The tag parser handles all common LLM response formats:
//!
//! ```rust
//! use ollama_vision::parse_tags;
//!
//! // Pure JSON array
//! assert!(parse_tags(r#"["portrait", "fantasy"]"#).is_ok());
//!
//! // With <think> blocks from reasoning models
//! assert!(parse_tags(r#"<think>analyzing...</think>["portrait"]"#).is_ok());
//!
//! // Markdown code blocks
//! assert!(parse_tags("```json\n[\"portrait\"]\n```").is_ok());
//!
//! // JSON objects with "tags" key
//! assert!(parse_tags(r#"{"tags": ["portrait"]}"#).is_ok());
//!
//! // Comma-separated fallback
//! assert!(parse_tags("portrait, fantasy, dark").is_ok());
//! ```

pub mod captioner;
pub mod parser;
pub mod tagger;
pub mod types;

// Re-export main types at crate root
pub use captioner::{caption_image, caption_image_base64, CaptionError};
pub use parser::{parse_tags, strip_think_tags, ParseError};
pub use tagger::{tag_image, tag_image_base64, TagError};
pub use types::{CaptionOptions, GenerateOptions, OllamaVisionConfig, TagOptions};
