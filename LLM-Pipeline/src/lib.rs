//! # LLM Pipeline
//!
//! Multi-stage LLM workflow orchestrator with streaming, extended thinking,
//! and cancellation support.
//!
//! This library provides a flexible framework for building multi-stage LLM
//! workflows. Each stage can use a different model, enable extended thinking
//! (DeepSeek R1 style), and be independently enabled or disabled.
//!
//! ## Features
//!
//! - **Composable stages** — chain multiple LLM calls with automatic
//!   output-to-input piping
//! - **Per-stage models** — use different models for different stages
//! - **Extended thinking** — support for `<think>...</think>` reasoning blocks
//! - **Streaming** — real-time token callbacks during execution
//! - **Cancellation** — interrupt pipelines mid-execution via `AtomicBool`
//! - **Context injection** — inject domain knowledge into prompt templates
//! - **Chat mode** — stages with system prompts use `/api/chat`
//! - **Defensive parsing** — extracts JSON from markdown blocks, embedded
//!   JSON, and raw responses
//!
//! ## Quick Start
//!
//! ```no_run
//! use llm_pipeline::{Pipeline, Stage, PipelineInput};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! struct Analysis {
//!     summary: String,
//!     insights: Vec<String>,
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = reqwest::Client::new();
//!
//!     let pipeline = Pipeline::<Analysis>::builder()
//!         .add_stage(Stage::new("analyze", "Analyze: {input}"))
//!         .add_stage(Stage::new("refine", "Refine: {input}").with_thinking(true))
//!         .build()?;
//!
//!     let result = pipeline.execute(
//!         &client,
//!         "http://localhost:11434",
//!         PipelineInput::new("Your text here"),
//!     ).await?;
//!
//!     println!("Summary: {}", result.final_output.summary);
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod error;
pub mod pipeline;
pub mod prompt;
pub mod stage;
pub mod types;

pub use client::LlmConfig;
pub use error::{PipelineError, Result};
pub use pipeline::{Pipeline, PipelineBuilder};
pub use stage::{Stage, StageBuilder};
pub use types::{PipelineContext, PipelineInput, PipelineProgress, PipelineResult, StageOutput};
