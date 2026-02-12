use ollama_vision::{OllamaVisionConfig, TagOptions};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let image_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| {
            eprintln!("Usage: tag_images <image_path> [model]");
            std::process::exit(1);
        });

    let model = std::env::args().nth(2).unwrap_or("llava".to_string());

    let config = OllamaVisionConfig::with_model(&model);
    let client = reqwest::Client::new();

    println!("Tagging {} with model '{}'...", image_path, model);

    let tags = ollama_vision::tag_image(
        &client,
        &config,
        Path::new(&image_path),
        &TagOptions::default(),
    )
    .await?;

    println!("Tags ({}):", tags.len());
    for tag in &tags {
        println!("  - {}", tag);
    }

    Ok(())
}
