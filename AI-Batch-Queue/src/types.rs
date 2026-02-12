use serde::{Deserialize, Serialize};

/// Per-item status within a batch job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BatchItemStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
    Cancelled,
}

/// Overall batch job status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BatchJobStatus {
    Queued,
    Running,
    Completed,
    CompletedWithErrors,
    Cancelled,
}

/// Overwrite policy for batch operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OverwritePolicy {
    /// Skip items that already have results.
    Skip,
    /// Overwrite existing results.
    Overwrite,
}

/// Size bucket for ETA estimation — groups items by processing complexity.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SizeBucket {
    Small,
    Medium,
    Large,
    Unknown,
}

impl SizeBucket {
    /// Classify by pixel count. Thresholds: <500K = Small, <2M = Medium, else Large.
    pub fn from_pixel_count(pixels: u64) -> Self {
        if pixels < 500_000 {
            Self::Small
        } else if pixels < 2_000_000 {
            Self::Medium
        } else {
            Self::Large
        }
    }

    /// Classify from optional width/height dimensions.
    pub fn from_dimensions(width: Option<u32>, height: Option<u32>) -> Self {
        match (width, height) {
            (Some(w), Some(h)) => Self::from_pixel_count(w as u64 * h as u64),
            _ => Self::Unknown,
        }
    }
}

/// A single item within a batch job.
///
/// The `data` field carries user-defined per-item payload (e.g. file path,
/// image ID, document reference). It must be serializable for event emission.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchItem<D>
where
    D: Clone + Send + Sync + Serialize,
{
    /// Unique identifier for this item within the batch.
    pub id: String,
    /// User-defined data payload.
    pub data: D,
    /// Current processing status.
    pub status: BatchItemStatus,
    /// Error message if status is Failed.
    pub error: Option<String>,
    /// Processing duration in milliseconds (set after completion).
    pub duration_ms: Option<u64>,
    /// Size bucket for ETA estimation.
    pub size_bucket: SizeBucket,
}

/// A batch job containing multiple items processed with the same resource.
///
/// The "resource key" (e.g. model name) is used for intelligent reordering:
/// jobs sharing a resource are grouped together to minimize expensive
/// resource swaps (GPU model loads, connection setup, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchJob<D>
where
    D: Clone + Send + Sync + Serialize,
{
    /// Unique job identifier.
    pub id: String,
    /// The resource key (e.g. model name) used for grouping.
    pub resource_key: String,
    /// Human-readable operation label (e.g. "tag", "caption", "embed").
    pub operation: String,
    /// Overwrite policy for items that already have results.
    pub overwrite_policy: OverwritePolicy,
    /// The items in this batch.
    pub items: Vec<BatchItem<D>>,
    /// Current job status.
    pub status: BatchJobStatus,
    /// ISO 8601 timestamp when the job was created.
    pub created_at: String,
    /// ISO 8601 timestamp when processing started.
    pub started_at: Option<String>,
    /// ISO 8601 timestamp when processing finished.
    pub completed_at: Option<String>,
    /// Whether the job was reordered for resource optimization.
    pub reordered: bool,
    /// Human-readable note explaining the reorder.
    pub reorder_note: Option<String>,
}

/// Summary of a completed batch job.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCompletionSummary {
    pub job_id: String,
    pub operation: String,
    pub resource_key: String,
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub total_duration_ms: u64,
    pub avg_duration_ms: u64,
}

/// Result of processing a single batch item.
#[derive(Debug, Clone)]
pub struct ItemResult {
    /// Whether the item was processed successfully.
    pub success: bool,
    /// Optional output data (e.g. generated tags, captions).
    pub output: Option<String>,
    /// Error message if processing failed.
    pub error: Option<String>,
}

impl ItemResult {
    pub fn success() -> Self {
        Self {
            success: true,
            output: None,
            error: None,
        }
    }

    pub fn success_with_output(output: String) -> Self {
        Self {
            success: true,
            output: Some(output),
            error: None,
        }
    }

    pub fn failure(error: String) -> Self {
        Self {
            success: false,
            output: None,
            error: Some(error),
        }
    }
}
