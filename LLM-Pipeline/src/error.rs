use thiserror::Error;

#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("JSON parsing failed: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Stage '{stage}' failed: {message}")]
    StageFailed { stage: String, message: String },

    #[error("Pipeline was cancelled")]
    Cancelled,

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("{0}")]
    Other(String),
}

impl From<anyhow::Error> for PipelineError {
    fn from(err: anyhow::Error) -> Self {
        PipelineError::Other(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, PipelineError>;
