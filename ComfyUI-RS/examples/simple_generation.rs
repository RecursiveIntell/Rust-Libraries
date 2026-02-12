//! Generate a single image from a text prompt.
//!
//! Requires a running ComfyUI instance at http://127.0.0.1:8188
//! with at least one checkpoint installed.
//!
//! ```sh
//! cargo run --example simple_generation
//! ```

use comfyui_rs::{ComfyClient, GenerationOutcome, Txt2ImgRequest};
use std::time::Duration;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let client = ComfyClient::new("http://127.0.0.1:8188");

    // Check connection
    if !client.health().await? {
        eprintln!("ComfyUI is not responding");
        return Ok(());
    }
    println!("ComfyUI is online");

    // List available checkpoints
    let checkpoints = client.checkpoints().await?;
    if checkpoints.is_empty() {
        eprintln!("No checkpoints found — install a model first");
        return Ok(());
    }
    println!("Using checkpoint: {}", checkpoints[0]);

    // Build workflow
    let (workflow, seed) = Txt2ImgRequest::new("a beautiful sunset over mountains", &checkpoints[0])
        .negative("lowres, blurry, bad anatomy")
        .steps(25)
        .cfg_scale(7.5)
        .build();
    println!("Seed: {}", seed);

    // Queue and wait
    let prompt_id = client.queue_prompt(&workflow).await?;
    println!("Queued prompt: {}", prompt_id);

    let result = client
        .wait_for_completion(&prompt_id, Duration::from_secs(120))
        .await?;

    match result {
        GenerationOutcome::Completed { images } => {
            println!("Generated {} image(s)", images.len());
            for img in &images {
                let bytes = client.image(img).await?;
                std::fs::write(&img.filename, &bytes)?;
                println!("Saved: {}", img.filename);
            }
        }
        GenerationOutcome::Failed { error } => eprintln!("Generation failed: {}", error),
        GenerationOutcome::TimedOut => eprintln!("Generation timed out"),
    }

    Ok(())
}
