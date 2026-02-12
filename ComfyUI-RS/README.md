# comfyui-rs

Async Rust client for [ComfyUI](https://github.com/comfyanonymous/ComfyUI) — the node-based Stable Diffusion GUI/backend.

## Features

- **Typed REST client** — queue prompts, fetch history, download images, manage VRAM
- **WebSocket progress** — real-time step-by-step updates with automatic polling fallback
- **Model discovery** — list available checkpoints, samplers, and schedulers
- **Workflow builder** — `Txt2ImgRequest` builder with sensible defaults
- **Custom error types** — `ComfyError` enum with `thiserror`, no `unwrap()`

## Installation

```toml
[dependencies]
comfyui-rs = { path = "../ComfyUI-RS" }
tokio = { version = "1", features = ["full"] }
```

## Quick Start

```rust
use comfyui_rs::{ComfyClient, Txt2ImgRequest, GenerationOutcome};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Connect
    let client = ComfyClient::new("http://127.0.0.1:8188");

    // 2. Build a workflow
    let checkpoints = client.checkpoints().await?;
    let (workflow, seed) = Txt2ImgRequest::new("a sunset over mountains", &checkpoints[0])
        .negative("lowres, blurry")
        .steps(25)
        .build();

    // 3. Queue and wait with real-time progress
    let prompt_id = client.queue_prompt(&workflow).await?;
    let result = client.wait_for_completion_ws(
        &prompt_id,
        Duration::from_secs(120),
        |p| println!("Step {}/{}", p.current_step, p.total_steps),
    ).await?;

    if let GenerationOutcome::Completed { images } = result {
        for img in &images {
            let bytes = client.image(img).await?;
            std::fs::write(&img.filename, &bytes)?;
        }
    }
    Ok(())
}
```

## Configuration

```rust
// Custom HTTP client (connection pooling, TLS, etc.)
let client = ComfyClient::new("http://192.168.1.100:8188")
    .with_http_client(reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?)
    .with_client_id("my-app");
```

## API Reference

### ComfyClient

| Method | Description |
|--------|-------------|
| `health()` | Check if ComfyUI is reachable |
| `queue_prompt(workflow)` | Queue a workflow, returns `prompt_id` |
| `history(prompt_id)` | Fetch prompt history (outputs, status) |
| `image(img_ref)` | Download output image bytes |
| `queue_status()` | Get running/pending job counts |
| `free_memory(unload_models)` | Free VRAM, optionally unload all models |
| `interrupt()` | Interrupt current generation |
| `checkpoints()` | List available checkpoint models |
| `samplers()` | List available sampler algorithms |
| `schedulers()` | List available scheduler algorithms |
| `wait_for_completion(id, timeout)` | Poll until done |
| `wait_for_completion_ws(id, timeout, callback)` | WebSocket progress with polling fallback |

### Txt2ImgRequest Builder

| Method | Default | Description |
|--------|---------|-------------|
| `new(prompt, checkpoint)` | — | Required: positive prompt + checkpoint name |
| `.negative(prompt)` | `""` | Negative prompt |
| `.size(w, h)` | 512x768 | Output dimensions |
| `.steps(n)` | 25 | Sampling steps |
| `.cfg_scale(f)` | 7.5 | Classifier-free guidance scale |
| `.sampler(name)` | `"dpmpp_2m"` | Sampler algorithm |
| `.scheduler(name)` | `"karras"` | Noise scheduler |
| `.seed(n)` | -1 (random) | Seed (-1 for random) |
| `.batch_size(n)` | 1 | Images per generation |
| `.filename_prefix(s)` | `"ComfyUI"` | Output filename prefix |
| `.build()` | — | Returns `(workflow_json, actual_seed)` |

### GenerationOutcome

```rust
match result {
    GenerationOutcome::Completed { images } => { /* Vec<ImageRef> */ }
    GenerationOutcome::Failed { error } => { /* String */ }
    GenerationOutcome::TimedOut => { /* ... */ }
}
```

## Examples

```sh
# Basic generation (requires running ComfyUI)
cargo run --example simple_generation

# Real-time WebSocket progress tracking
cargo run --example progress_tracking

# Explore models and build custom workflows
cargo run --example workflow_builder
```

## License

MIT
