//! # AI Batch Queue
//!
//! Model-aware batch processing queue with ETA estimation for Tauri applications.
//!
//! ## Key Features
//!
//! - **Resource-aware reordering** — automatically groups jobs by resource key
//!   (e.g. model name) to minimize expensive swaps
//! - **Size-bucketed ETA estimation** — tracks processing durations by
//!   (resource, operation, size) for accurate time predictions
//! - **Item-level status tracking** — each item has its own lifecycle
//! - **Overwrite policies** — skip items that already have results
//! - **Progressive completion with retry** — failed items can be retried
//!   without re-processing successful ones
//!
//! ## Quick Start
//!
//! 1. Define your item data type
//! 2. Implement [`BatchItemHandler`] for your processing logic
//! 3. Create a [`BatchQueue`] and register it in Tauri state
//! 4. Call [`executor::spawn()`] to start the background processor

pub mod eta;
pub mod executor;
pub mod queue;
pub mod types;

pub use queue::BatchQueue;
pub use types::{
    BatchCompletionSummary, BatchItem, BatchItemStatus, BatchJob, BatchJobStatus, ItemResult,
    OverwritePolicy, SizeBucket,
};

/// Trait for processing individual items in a batch.
///
/// Implement this for your application to define:
/// - How to process each item (`process`)
/// - Whether an item should be skipped (`should_skip`)
///
/// # Type Parameter
///
/// `D` is the per-item data type (e.g. a file path, image reference, document ID).
///
/// # Example
///
/// ```ignore
/// use ai_batch_queue::*;
///
/// struct MyProcessor;
///
/// impl BatchItemHandler<String> for MyProcessor {
///     async fn process(
///         &self,
///         data: &String,
///         resource_key: &str,
///         operation: &str,
///     ) -> anyhow::Result<ItemResult> {
///         println!("Processing {} with {}", data, resource_key);
///         Ok(ItemResult::success())
///     }
///
///     fn should_skip(&self, data: &String, operation: &str) -> bool {
///         false // never skip
///     }
/// }
/// ```
pub trait BatchItemHandler<D>: Send + Sync + 'static
where
    D: Clone + Send + Sync + serde::Serialize,
{
    /// Process a single item.
    ///
    /// # Arguments
    /// * `data` — the item's user-defined data payload
    /// * `resource_key` — the resource this batch uses (e.g. model name)
    /// * `operation` — the operation label (e.g. "tag", "caption")
    fn process(
        &self,
        data: &D,
        resource_key: &str,
        operation: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<ItemResult>> + Send;

    /// Check if this item should be skipped when the overwrite policy is `Skip`.
    ///
    /// Return `true` to skip (item already has results).
    /// Default implementation never skips.
    fn should_skip(&self, _data: &D, _operation: &str) -> bool {
        false
    }
}

/// Helper to build a [`BatchJob`] from a list of items.
///
/// # Example
///
/// ```
/// use ai_batch_queue::*;
///
/// let job = build_job(
///     "llava:13b",
///     "tag",
///     OverwritePolicy::Skip,
///     vec![
///         ("img-1".to_string(), "path/to/1.png".to_string(), SizeBucket::Medium),
///         ("img-2".to_string(), "path/to/2.png".to_string(), SizeBucket::Large),
///     ],
/// );
///
/// assert_eq!(job.items.len(), 2);
/// assert_eq!(job.resource_key, "llava:13b");
/// ```
pub fn build_job<D>(
    resource_key: &str,
    operation: &str,
    overwrite_policy: OverwritePolicy,
    items: Vec<(String, D, SizeBucket)>,
) -> BatchJob<D>
where
    D: Clone + Send + Sync + serde::Serialize,
{
    let batch_items = items
        .into_iter()
        .map(|(id, data, bucket)| BatchItem {
            id,
            data,
            status: BatchItemStatus::Pending,
            error: None,
            duration_ms: None,
            size_bucket: bucket,
        })
        .collect();

    BatchJob {
        id: String::new(),
        resource_key: resource_key.to_string(),
        operation: operation.to_string(),
        overwrite_policy,
        items: batch_items,
        status: BatchJobStatus::Queued,
        created_at: String::new(),
        started_at: None,
        completed_at: None,
        reordered: false,
        reorder_note: None,
    }
}
