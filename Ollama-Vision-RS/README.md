# ollama-vision

Robust Ollama vision model toolkit for image tagging and captioning in Rust.

## Features

- **Image tagging** — extract structured tags from any Ollama vision model
- **Image captioning** — generate natural language descriptions
- **7-strategy response parser** — handles every LLM output format reliably
- **Thinking model support** — automatically strips `<think>` blocks
- **Base64 API** — tag/caption in-memory images without file I/O

## The Parser

The killer feature. LLM vision models return wildly inconsistent formats. This parser handles all of them:

| Strategy | Format | Example |
|----------|--------|---------|
| 1 | Pure JSON array | `["portrait", "fantasy"]` |
| 2 | After `<think>` blocks | `<think>reasoning...</think>["portrait"]` |
| 3 | JSON object with key | `{"tags": ["portrait", "fantasy"]}` |
| 4 | Markdown code blocks | `` ```json\n["portrait"]\n``` `` |
| 5 | Bracket-matched extraction | `Here are tags: ["portrait", "fantasy"]` |
| 6 | Numbered/bulleted lists | `1. portrait\n2. fantasy` |
| 7 | Comma-separated fallback | `portrait, fantasy, dark lighting` |

All strategies normalize to lowercase, deduplicate, trim whitespace, and filter overlong entries.

## Quick Start

```rust
use ollama_vision::{OllamaVisionConfig, TagOptions, CaptionOptions};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = OllamaVisionConfig::with_model("llava");
    let client = reqwest::Client::new();

    // Tag an image
    let tags = ollama_vision::tag_image(
        &client, &config,
        Path::new("photo.jpg"),
        &TagOptions::default(),
    ).await?;
    println!("Tags: {:?}", tags);

    // Caption an image
    let caption = ollama_vision::caption_image(
        &client, &config,
        Path::new("photo.jpg"),
        &CaptionOptions::default(),
    ).await?;
    println!("Caption: {}", caption);

    Ok(())
}
```

## Use the Parser Directly

```rust
use ollama_vision::parse_tags;

// Works with any LLM text output
let tags = parse_tags(r#"<think>let me analyze...</think>["cat", "cute", "fluffy"]"#).unwrap();
assert_eq!(tags, vec!["cat", "cute", "fluffy"]);
```

## Configuration

```rust
use ollama_vision::{OllamaVisionConfig, GenerateOptions, TagOptions};
use std::time::Duration;

let config = OllamaVisionConfig::with_model("minicpm-v")
    .endpoint("http://192.168.1.100:11434")
    .timeout(Duration::from_secs(180))
    .options(GenerateOptions {
        num_predict: 1024,
        temperature: Some(0.3),
        ..Default::default()
    });

// Disable JSON format for thinking models
let tag_opts = TagOptions {
    request_json_format: false,
    ..Default::default()
};
```

## Examples

```bash
cargo run --example tag_images -- photo.jpg llava
cargo run --example caption_images -- photo.jpg llava
cargo run --example thinking_mode -- photo.jpg minicpm-v
```

## License

MIT
