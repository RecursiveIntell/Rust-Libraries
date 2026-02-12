//! Demonstrate the workflow builder and model discovery.
//!
//! Lists available checkpoints, samplers, and schedulers, then builds
//! a customized txt2img workflow and prints the JSON.
//!
//! ```sh
//! cargo run --example workflow_builder
//! ```

use comfyui_rs::{ComfyClient, Txt2ImgRequest};

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let client = ComfyClient::new("http://127.0.0.1:8188");

    // Discover available options
    let checkpoints = client.checkpoints().await?;
    let samplers = client.samplers().await?;
    let schedulers = client.schedulers().await?;

    println!("Checkpoints ({}):", checkpoints.len());
    for c in &checkpoints {
        println!("  - {}", c);
    }
    println!("\nSamplers ({}):", samplers.len());
    for s in &samplers {
        println!("  - {}", s);
    }
    println!("\nSchedulers ({}):", schedulers.len());
    for s in &schedulers {
        println!("  - {}", s);
    }

    if checkpoints.is_empty() {
        eprintln!("\nNo checkpoints found — install a model first");
        return Ok(());
    }

    // Build a fully customized request
    let request = Txt2ImgRequest::new("masterpiece, best quality, landscape painting", &checkpoints[0])
        .negative("lowres, blurry, bad anatomy, watermark")
        .size(1024, 1024)
        .steps(30)
        .cfg_scale(7.0)
        .sampler("dpmpp_2m")
        .scheduler("karras")
        .seed(42)
        .batch_size(2)
        .filename_prefix("my-project");

    let (workflow, seed) = request.build();
    println!("\nWorkflow JSON:");
    println!("{}", serde_json::to_string_pretty(&workflow)?);
    println!("\nSeed: {}", seed);

    Ok(())
}
