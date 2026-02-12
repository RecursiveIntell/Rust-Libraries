use ollama_vision::{OllamaVisionConfig, TagOptions};
use std::path::Path;

/// Demonstrates tagging with a reasoning/thinking model (e.g., deepseek-r1).
///
/// Thinking models wrap their reasoning in `<think>...</think>` blocks.
/// The parser automatically strips these before extracting tags.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let image_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| {
            eprintln!("Usage: thinking_mode <image_path> [model]");
            eprintln!("  model defaults to 'minicpm-v' (supports thinking)");
            std::process::exit(1);
        });

    let model = std::env::args()
        .nth(2)
        .unwrap_or("minicpm-v".to_string());

    let config = OllamaVisionConfig::with_model(&model);
    let client = reqwest::Client::new();

    // Disable JSON format constraint — thinking models often struggle with it
    let options = TagOptions {
        request_json_format: false,
        ..Default::default()
    };

    println!("Tagging {} with thinking model '{}'...", image_path, model);
    println!("(JSON format constraint disabled for thinking model compatibility)");

    let tags = ollama_vision::tag_image(
        &client,
        &config,
        Path::new(&image_path),
        &options,
    )
    .await?;

    println!("\nTags ({}):", tags.len());
    for tag in &tags {
        println!("  - {}", tag);
    }

    Ok(())
}
