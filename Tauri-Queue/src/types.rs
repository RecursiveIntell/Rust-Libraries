use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// Priority levels for queue jobs.
///
/// Jobs are processed in priority order: High (1), Normal (2), Low (3).
/// Within the same priority, jobs are processed in FIFO order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueuePriority {
    Low,
    Normal,
    High,
}

impl QueuePriority {
    pub fn as_i32(&self) -> i32 {
        match self {
            QueuePriority::Low => 3,
            QueuePriority::Normal => 2,
            QueuePriority::High => 1,
        }
    }

    pub fn from_i32(val: i32) -> Self {
        match val {
            1 => QueuePriority::High,
            2 => QueuePriority::Normal,
            _ => QueuePriority::Low,
        }
    }
}

/// Job status lifecycle: Pending -> Processing -> Completed/Failed/Cancelled
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueueJobStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

impl QueueJobStatus {
    pub fn as_str(&self) -> &str {
        match self {
            QueueJobStatus::Pending => "pending",
            QueueJobStatus::Processing => "processing",
            QueueJobStatus::Completed => "completed",
            QueueJobStatus::Failed => "failed",
            QueueJobStatus::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(QueueJobStatus::Pending),
            "processing" => Some(QueueJobStatus::Processing),
            "completed" => Some(QueueJobStatus::Completed),
            "failed" => Some(QueueJobStatus::Failed),
            "cancelled" => Some(QueueJobStatus::Cancelled),
            _ => None,
        }
    }
}

/// A generic queue job carrying a custom data payload.
///
/// The data field is stored as JSON in SQLite and deserialized back when the
/// executor picks up the job. Your data type must implement `Serialize`,
/// `DeserializeOwned`, `Clone`, `Send`, and `Sync`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound(deserialize = "T: DeserializeOwned"))]
pub struct QueueJob<T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync,
{
    pub id: String,
    pub priority: QueuePriority,
    pub status: QueueJobStatus,
    pub data: T,
    pub created_at: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
}

impl<T> QueueJob<T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync,
{
    /// Create a new job with a generated UUID and Normal priority.
    pub fn new(data: T) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            priority: QueuePriority::Normal,
            status: QueueJobStatus::Pending,
            data,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    /// Set the priority for this job (builder pattern).
    pub fn with_priority(mut self, priority: QueuePriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set a custom ID for this job (builder pattern).
    pub fn with_id(mut self, id: String) -> Self {
        self.id = id;
        self
    }
}

/// Result returned by a job handler after execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
}

impl JobResult {
    /// Create a successful result with no output.
    pub fn success() -> Self {
        Self {
            success: true,
            output: None,
            error: None,
        }
    }

    /// Create a successful result with output data.
    pub fn success_with_output(output: String) -> Self {
        Self {
            success: true,
            output: Some(output),
            error: None,
        }
    }

    /// Create a failure result with an error message.
    pub fn failure(error: String) -> Self {
        Self {
            success: false,
            output: None,
            error: Some(error),
        }
    }
}
