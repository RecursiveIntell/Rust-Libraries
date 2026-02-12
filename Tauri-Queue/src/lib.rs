//! # Tauri Queue
//!
//! Production-grade background job queue system for Tauri applications.
//!
//! ## Features
//!
//! - Priority-based scheduling (High, Normal, Low)
//! - SQLite persistence with crash recovery
//! - Hardware throttling (cooldown, max consecutive runs)
//! - Real-time cancellation during job execution
//! - Progress tracking via Tauri events
//! - Pause/resume capability
//!
//! ## Quick Start
//!
//! 1. Define a job type implementing [`JobHandler`]
//! 2. Create a [`QueueManager`] with a [`QueueConfig`]
//! 3. Add jobs with [`QueueManager::add()`]
//! 4. Spawn the executor with [`QueueManager::spawn()`]
//!
//! See the `examples/` directory for complete usage examples.

pub mod config;
pub mod db;
pub mod error;
pub mod events;
pub mod executor;
pub mod queue;
pub mod types;

pub use config::{QueueConfig, QueueConfigBuilder};
pub use error::QueueError;
pub use queue::QueueManager;
pub use types::{JobResult, QueueJob, QueueJobStatus, QueuePriority};

use rusqlite::Connection;
use std::sync::{Arc, Mutex};

/// Context provided to job handlers during execution.
///
/// Gives access to the Tauri app handle for emitting events, and
/// methods for checking cancellation and emitting progress.
pub struct JobContext {
    /// The ID of the currently executing job.
    pub job_id: String,
    /// The Tauri app handle for emitting events.
    pub app_handle: tauri::AppHandle,
    /// Shared database connection for cancellation checks.
    pub(crate) db: Arc<Mutex<Connection>>,
}

impl JobContext {
    /// Emit a progress event to the frontend.
    ///
    /// # Arguments
    /// * `current` - Current step number
    /// * `total` - Total number of steps
    pub fn emit_progress(&self, current: u32, total: u32) -> Result<(), QueueError> {
        use tauri::Emitter;
        self.app_handle
            .emit(
                "queue:job_progress",
                events::JobProgressEvent {
                    job_id: self.job_id.clone(),
                    current_step: current,
                    total_steps: total,
                    progress: if total > 0 {
                        current as f64 / total as f64
                    } else {
                        0.0
                    },
                },
            )
            .map_err(|e| QueueError::Event(e.to_string()))?;
        Ok(())
    }

    /// Check if this job has been cancelled.
    ///
    /// Call this periodically during long-running jobs to support
    /// cooperative cancellation. If it returns `true`, your handler
    /// should return `Err(QueueError::Cancelled)`.
    pub fn is_cancelled(&self) -> bool {
        match self.db.lock() {
            Ok(conn) => db::is_cancelled(&conn, &self.job_id).unwrap_or(false),
            Err(_) => false,
        }
    }
}

/// Trait that job types must implement to be processed by the queue.
///
/// Your job type must be serializable (stored as JSON in SQLite),
/// cloneable, and thread-safe.
///
/// # Example
///
/// ```ignore
/// use serde::{Serialize, Deserialize};
/// use tauri_queue::*;
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct EmailJob {
///     to: String,
///     subject: String,
/// }
///
/// impl JobHandler for EmailJob {
///     async fn execute(&self, ctx: &JobContext) -> Result<JobResult, QueueError> {
///         // Send email...
///         ctx.emit_progress(1, 1)?;
///         Ok(JobResult::success())
///     }
/// }
/// ```
pub trait JobHandler: Send + Sync + serde::Serialize + serde::de::DeserializeOwned + Clone {
    /// Execute the job. This is called by the executor when the job is picked up.
    ///
    /// Use `ctx.emit_progress()` to report progress and `ctx.is_cancelled()`
    /// to check for cancellation during long-running operations.
    fn execute(
        &self,
        ctx: &JobContext,
    ) -> impl std::future::Future<Output = Result<JobResult, QueueError>> + Send;

    /// Optional: a human-readable name for this job type, used in logging.
    fn job_type(&self) -> &str {
        std::any::type_name::<Self>()
    }
}
