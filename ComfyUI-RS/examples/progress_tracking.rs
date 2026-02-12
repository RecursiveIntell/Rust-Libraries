//! Generate an image with real-time WebSocket progress tracking.
//!
//! Uses ComfyUI's WebSocket API for step-by-step progress updates.
//! Automatically falls back to polling if WebSocket connection fails.
//!
//! ```sh
//! cargo run --example progress_tracking
//! ```

use comfyui_rs::{ComfyClient, GenerationOutcome, Txt2ImgRequest};
use std::time::Duration;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let client = ComfyClient::new("http://127.0.0.1:8188")
        .with_client_id("progress-example");

    let checkpoints = client.checkpoints().await?;
    if checkpoints.is_empty() {
        eprintln!("No checkpoints found");
        return Ok(());
    }

    let (workflow, seed) = Txt2ImgRequest::new("a cat wearing a tiny hat, digital art", &checkpoints[0])
        .steps(30)
        .cfg_scale(7.5)
        .build();
    println!("Seed: {}", seed);

    let prompt_id = client.queue_prompt(&workflow).await?;
    println!("Queued: {}", prompt_id);

    // WebSocket with real-time progress, auto-falls back to polling
    let result = client
        .wait_for_completion_ws(&prompt_id, Duration::from_secs(300), |progress| {
            let pct = (progress.current_step as f64 / progress.total_steps as f64) * 100.0;
            println!(
                "  Step {}/{} ({:.0}%)",
                progress.current_step, progress.total_steps, pct
            );
        })
        .await?;

    match result {
        GenerationOutcome::Completed { images } => {
            println!("Done! Generated {} image(s)", images.len());
        }
        GenerationOutcome::Failed { error } => eprintln!("Failed: {}", error),
        GenerationOutcome::TimedOut => eprintln!("Timed out"),
    }

    Ok(())
}
