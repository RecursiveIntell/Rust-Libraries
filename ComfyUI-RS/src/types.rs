use serde::{Deserialize, Serialize};

/// Real-time progress update from ComfyUI's WebSocket.
#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    pub current_step: u32,
    pub total_steps: u32,
}

/// Reference to an image stored in ComfyUI's output directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRef {
    pub filename: String,
    pub subfolder: String,
    pub img_type: String,
}

/// Parsed history entry for a completed prompt.
#[derive(Debug, Clone)]
pub struct PromptHistory {
    pub status: String,
    pub completed: bool,
    pub images: Vec<ImageRef>,
}

/// Snapshot of ComfyUI's queue state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStatus {
    pub running: u32,
    pub pending: u32,
}

/// Outcome of waiting for a generation to finish.
#[derive(Debug, Clone)]
pub enum GenerationOutcome {
    /// Generation completed successfully with output images.
    Completed { images: Vec<ImageRef> },
    /// ComfyUI reported an execution-level failure.
    Failed { error: String },
    /// Timed out before completion.
    TimedOut,
}
