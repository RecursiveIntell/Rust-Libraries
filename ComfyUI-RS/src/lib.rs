//! # comfyui-rs
//!
//! Async Rust client for [ComfyUI](https://github.com/comfyanonymous/ComfyUI) —
//! the node-based Stable Diffusion GUI/backend.
//!
//! Provides a typed client for REST operations, WebSocket-based real-time
//! progress tracking with automatic polling fallback, model discovery, and
//! a workflow builder for common generation patterns.
//!
//! ## Quick Start
//!
//! ```no_run
//! use comfyui_rs::{ComfyClient, Txt2ImgRequest, GenerationOutcome};
//! use std::time::Duration;
//!
//! # async fn example() -> comfyui_rs::Result<()> {
//! let client = ComfyClient::new("http://127.0.0.1:8188");
//!
//! // Discover models
//! let checkpoints = client.checkpoints().await?;
//! let checkpoint = &checkpoints[0];
//!
//! // Build a workflow
//! let (workflow, seed) = Txt2ImgRequest::new("a sunset over mountains", checkpoint)
//!     .negative("lowres, blurry")
//!     .steps(25)
//!     .build();
//!
//! // Queue and wait with real-time progress
//! let prompt_id = client.queue_prompt(&workflow).await?;
//! let result = client.wait_for_completion_ws(
//!     &prompt_id,
//!     Duration::from_secs(120),
//!     |p| println!("Step {}/{}", p.current_step, p.total_steps),
//! ).await?;
//!
//! if let GenerationOutcome::Completed { images } = result {
//!     for img in &images {
//!         let bytes = client.image(img).await?;
//!         std::fs::write(&img.filename, &bytes).unwrap();
//!     }
//! }
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod error;
pub mod types;
pub mod workflow;

pub use client::ComfyClient;
pub use error::{ComfyError, Result};
pub use types::{GenerationOutcome, ImageRef, ProgressUpdate, PromptHistory, QueueStatus};
pub use workflow::Txt2ImgRequest;
