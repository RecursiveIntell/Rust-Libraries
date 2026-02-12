use thiserror::Error;

/// Errors that can occur in the queue system.
#[derive(Error, Debug)]
pub enum QueueError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Job execution failed: {0}")]
    Execution(String),

    #[error("Event emission failed: {0}")]
    Event(String),

    #[error("Job not found: {0}")]
    NotFound(String),

    #[error("Queue is paused")]
    Paused,

    #[error("Job was cancelled")]
    Cancelled,

    #[error("{0}")]
    Other(String),
}

impl From<anyhow::Error> for QueueError {
    fn from(err: anyhow::Error) -> Self {
        QueueError::Other(err.to_string())
    }
}
