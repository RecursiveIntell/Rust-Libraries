use std::sync::Mutex;

use crate::eta::EtaTracker;
use crate::types::*;

/// In-memory batch queue with model-aware reordering and ETA estimation.
///
/// The queue automatically groups jobs by `resource_key` to minimize expensive
/// resource swaps (e.g. GPU model loads). It also tracks per-item processing
/// durations bucketed by size for accurate ETA predictions.
pub struct BatchQueue<D>
where
    D: Clone + Send + Sync + serde::Serialize + 'static,
{
    jobs: Mutex<Vec<BatchJob<D>>>,
    pub(crate) eta: EtaTracker,
}

impl<D> Default for BatchQueue<D>
where
    D: Clone + Send + Sync + serde::Serialize + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<D> BatchQueue<D>
where
    D: Clone + Send + Sync + serde::Serialize + 'static,
{
    /// Create a new empty batch queue.
    pub fn new() -> Self {
        Self {
            jobs: Mutex::new(Vec::new()),
            eta: EtaTracker::new(),
        }
    }

    /// Add a new batch job and perform resource-aware reordering.
    /// Returns the assigned job ID.
    pub fn enqueue(&self, mut job: BatchJob<D>) -> anyhow::Result<String> {
        let mut jobs = self.jobs.lock().map_err(|e| anyhow::anyhow!("{}", e))?;

        if job.id.is_empty() {
            job.id = uuid::Uuid::new_v4().to_string();
        }
        job.status = BatchJobStatus::Queued;
        job.created_at = chrono::Utc::now().to_rfc3339();

        let job_id = job.id.clone();
        jobs.push(job);

        Self::reorder_queued_jobs(&mut jobs);
        Ok(job_id)
    }

    /// Reorder only queued jobs to group by resource_key (minimizes resource swaps).
    ///
    /// For example, if you queue jobs for models A, B, A, this reorders to A, A, B
    /// so the GPU only loads each model once instead of switching back and forth.
    fn reorder_queued_jobs(jobs: &mut [BatchJob<D>]) {
        let queued_indices: Vec<usize> = jobs
            .iter()
            .enumerate()
            .filter(|(_, j)| j.status == BatchJobStatus::Queued)
            .map(|(i, _)| i)
            .collect();

        if queued_indices.len() < 2 {
            return;
        }

        let mut queued_jobs: Vec<BatchJob<D>> =
            queued_indices.iter().map(|&i| jobs[i].clone()).collect();

        let original_order: Vec<String> = queued_jobs.iter().map(|j| j.id.clone()).collect();
        queued_jobs.sort_by(|a, b| a.resource_key.cmp(&b.resource_key));
        let new_order: Vec<String> = queued_jobs.iter().map(|j| j.id.clone()).collect();

        if original_order != new_order {
            for job in &mut queued_jobs {
                job.reordered = true;
                job.reorder_note =
                    Some("Reordered: grouping by resource to minimize swaps".to_string());
            }
            for (slot_idx, job) in queued_indices.iter().zip(queued_jobs) {
                jobs[*slot_idx] = job;
            }
        }
    }

    /// Get the next queued job (without removing it).
    pub fn next_queued(&self) -> Option<BatchJob<D>> {
        let jobs = self.jobs.lock().ok()?;
        jobs.iter()
            .find(|j| j.status == BatchJobStatus::Queued)
            .cloned()
    }

    /// Mark a job as running and set its started_at timestamp.
    pub fn mark_running(&self, job_id: &str) -> anyhow::Result<()> {
        let mut jobs = self.jobs.lock().map_err(|e| anyhow::anyhow!("{}", e))?;
        if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
            job.status = BatchJobStatus::Running;
            job.started_at = Some(chrono::Utc::now().to_rfc3339());
        }
        Ok(())
    }

    /// Update a single item's status within a job.
    ///
    /// If the item completed successfully and `duration_ms` is provided,
    /// the ETA tracker is automatically updated with the new data point.
    pub fn update_item(
        &self,
        job_id: &str,
        item_id: &str,
        status: BatchItemStatus,
        error: Option<String>,
        duration_ms: Option<u64>,
    ) -> anyhow::Result<()> {
        let mut jobs = self.jobs.lock().map_err(|e| anyhow::anyhow!("{}", e))?;
        if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
            if let Some(item) = job.items.iter_mut().find(|i| i.id == item_id) {
                let should_record = status == BatchItemStatus::Completed && duration_ms.is_some();
                let resource_key = job.resource_key.clone();
                let operation = job.operation.clone();
                let bucket = item.size_bucket;

                item.status = status;
                item.error = error;
                item.duration_ms = duration_ms;

                if should_record {
                    let ms = duration_ms.unwrap();
                    drop(jobs); // Release jobs lock before eta lock
                    self.eta.record(&resource_key, &operation, bucket, ms);
                }
            }
        }
        Ok(())
    }

    /// Mark a job as completed and produce a completion summary.
    ///
    /// Automatically determines whether it's `Completed` or `CompletedWithErrors`
    /// based on item statuses.
    pub fn mark_completed(&self, job_id: &str) -> anyhow::Result<Option<BatchCompletionSummary>> {
        let mut jobs = self.jobs.lock().map_err(|e| anyhow::anyhow!("{}", e))?;
        if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
            let failed = job
                .items
                .iter()
                .filter(|i| i.status == BatchItemStatus::Failed)
                .count();
            let succeeded = job
                .items
                .iter()
                .filter(|i| i.status == BatchItemStatus::Completed)
                .count();
            let skipped = job
                .items
                .iter()
                .filter(|i| {
                    i.status == BatchItemStatus::Cancelled || i.status == BatchItemStatus::Skipped
                })
                .count();

            job.status = if failed > 0 {
                BatchJobStatus::CompletedWithErrors
            } else {
                BatchJobStatus::Completed
            };
            job.completed_at = Some(chrono::Utc::now().to_rfc3339());

            let total_ms: u64 = job.items.iter().filter_map(|i| i.duration_ms).sum();
            let processed = succeeded + failed;
            let avg_ms = if processed > 0 {
                total_ms / processed as u64
            } else {
                0
            };

            return Ok(Some(BatchCompletionSummary {
                job_id: job.id.clone(),
                operation: job.operation.clone(),
                resource_key: job.resource_key.clone(),
                total: job.items.len(),
                succeeded,
                failed,
                skipped,
                total_duration_ms: total_ms,
                avg_duration_ms: avg_ms,
            }));
        }
        Ok(None)
    }

    /// Cancel a single pending item within a job.
    pub fn cancel_item(&self, job_id: &str, item_id: &str) -> anyhow::Result<()> {
        let mut jobs = self.jobs.lock().map_err(|e| anyhow::anyhow!("{}", e))?;
        if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
            if let Some(item) = job.items.iter_mut().find(|i| i.id == item_id) {
                if item.status == BatchItemStatus::Pending {
                    item.status = BatchItemStatus::Cancelled;
                }
            }
        }
        Ok(())
    }

    /// Cancel an entire batch job. Running items finish; pending items are cancelled.
    pub fn cancel_job(&self, job_id: &str) -> anyhow::Result<()> {
        let mut jobs = self.jobs.lock().map_err(|e| anyhow::anyhow!("{}", e))?;
        if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
            for item in &mut job.items {
                if item.status == BatchItemStatus::Pending {
                    item.status = BatchItemStatus::Cancelled;
                }
            }
            let any_running = job
                .items
                .iter()
                .any(|i| i.status == BatchItemStatus::Running);
            if !any_running {
                job.status = BatchJobStatus::Cancelled;
                job.completed_at = Some(chrono::Utc::now().to_rfc3339());
            }
        }
        Ok(())
    }

    /// Retry all failed items in a completed job by resetting them to Pending.
    /// The job is re-queued and reordering is applied.
    pub fn retry_failed(&self, job_id: &str) -> anyhow::Result<()> {
        let mut jobs = self.jobs.lock().map_err(|e| anyhow::anyhow!("{}", e))?;
        if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
            let has_failed = job
                .items
                .iter()
                .any(|i| i.status == BatchItemStatus::Failed);
            if !has_failed {
                anyhow::bail!("No failed items to retry in job {}", job_id);
            }
            for item in &mut job.items {
                if item.status == BatchItemStatus::Failed {
                    item.status = BatchItemStatus::Pending;
                    item.error = None;
                    item.duration_ms = None;
                }
            }
            job.status = BatchJobStatus::Queued;
            job.completed_at = None;
            Self::reorder_queued_jobs(&mut jobs);
        }
        Ok(())
    }

    /// Get all jobs (cloned snapshot).
    pub fn list_jobs(&self) -> Vec<BatchJob<D>> {
        self.jobs.lock().map(|j| j.clone()).unwrap_or_default()
    }

    /// Get a specific job by ID.
    pub fn get_job(&self, job_id: &str) -> Option<BatchJob<D>> {
        self.jobs
            .lock()
            .ok()?
            .iter()
            .find(|j| j.id == job_id)
            .cloned()
    }

    /// Estimate remaining processing time for a job in milliseconds.
    /// Returns `None` if no historical data is available.
    pub fn estimate_remaining_ms(&self, job_id: &str) -> Option<u64> {
        let jobs = self.jobs.lock().ok()?;
        let job = jobs.iter().find(|j| j.id == job_id)?;

        let remaining_buckets: Vec<SizeBucket> = job
            .items
            .iter()
            .filter(|i| {
                i.status == BatchItemStatus::Pending || i.status == BatchItemStatus::Running
            })
            .map(|i| i.size_bucket)
            .collect();

        if remaining_buckets.is_empty() {
            return Some(0);
        }

        self.eta
            .estimate_remaining(&job.resource_key, &job.operation, &remaining_buckets)
    }

    /// Check if any batch job is currently running.
    pub fn has_running_job(&self) -> bool {
        self.jobs
            .lock()
            .map(|j| j.iter().any(|job| job.status == BatchJobStatus::Running))
            .unwrap_or(false)
    }

    /// Get the number of ETA samples for a specific resource/operation/size combination.
    pub fn eta_sample_count(
        &self,
        resource_key: &str,
        operation: &str,
        size_bucket: SizeBucket,
    ) -> u64 {
        self.eta.sample_count(resource_key, operation, size_bucket)
    }

    /// Get the number of queued (waiting) jobs.
    pub fn queued_count(&self) -> usize {
        self.jobs
            .lock()
            .map(|j| {
                j.iter()
                    .filter(|job| job.status == BatchJobStatus::Queued)
                    .count()
            })
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_items(count: usize) -> Vec<BatchItem<String>> {
        (0..count)
            .map(|i| BatchItem {
                id: format!("item-{}", i),
                data: format!("data-{}", i),
                status: BatchItemStatus::Pending,
                error: None,
                duration_ms: None,
                size_bucket: SizeBucket::Medium,
            })
            .collect()
    }

    fn make_job(resource: &str, op: &str, count: usize) -> BatchJob<String> {
        BatchJob {
            id: String::new(),
            resource_key: resource.to_string(),
            operation: op.to_string(),
            overwrite_policy: OverwritePolicy::Skip,
            items: make_items(count),
            status: BatchJobStatus::Queued,
            created_at: String::new(),
            started_at: None,
            completed_at: None,
            reordered: false,
            reorder_note: None,
        }
    }

    #[test]
    fn test_enqueue_assigns_id() {
        let queue: BatchQueue<String> = BatchQueue::new();
        let job = make_job("model-a", "tag", 3);
        let id = queue.enqueue(job).unwrap();
        assert!(!id.is_empty());
    }

    #[test]
    fn test_next_queued() {
        let queue: BatchQueue<String> = BatchQueue::new();
        assert!(queue.next_queued().is_none());

        let job = make_job("model-a", "tag", 2);
        let id = queue.enqueue(job).unwrap();

        let next = queue.next_queued().unwrap();
        assert_eq!(next.id, id);
    }

    #[test]
    fn test_mark_running() {
        let queue: BatchQueue<String> = BatchQueue::new();
        let id = queue.enqueue(make_job("model-a", "tag", 1)).unwrap();

        queue.mark_running(&id).unwrap();
        let job = queue.get_job(&id).unwrap();
        assert_eq!(job.status, BatchJobStatus::Running);
        assert!(job.started_at.is_some());
    }

    #[test]
    fn test_update_item_and_complete() {
        let queue: BatchQueue<String> = BatchQueue::new();
        let id = queue.enqueue(make_job("model-a", "tag", 2)).unwrap();
        queue.mark_running(&id).unwrap();

        queue
            .update_item(&id, "item-0", BatchItemStatus::Completed, None, Some(1000))
            .unwrap();
        queue
            .update_item(&id, "item-1", BatchItemStatus::Completed, None, Some(2000))
            .unwrap();

        let summary = queue.mark_completed(&id).unwrap().unwrap();
        assert_eq!(summary.succeeded, 2);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.total_duration_ms, 3000);
        assert_eq!(summary.avg_duration_ms, 1500);
    }

    #[test]
    fn test_completed_with_errors() {
        let queue: BatchQueue<String> = BatchQueue::new();
        let id = queue.enqueue(make_job("model-a", "tag", 2)).unwrap();
        queue.mark_running(&id).unwrap();

        queue
            .update_item(&id, "item-0", BatchItemStatus::Completed, None, Some(1000))
            .unwrap();
        queue
            .update_item(
                &id,
                "item-1",
                BatchItemStatus::Failed,
                Some("timeout".to_string()),
                Some(5000),
            )
            .unwrap();

        let summary = queue.mark_completed(&id).unwrap().unwrap();
        assert_eq!(summary.succeeded, 1);
        assert_eq!(summary.failed, 1);

        let job = queue.get_job(&id).unwrap();
        assert_eq!(job.status, BatchJobStatus::CompletedWithErrors);
    }

    #[test]
    fn test_cancel_job() {
        let queue: BatchQueue<String> = BatchQueue::new();
        let id = queue.enqueue(make_job("model-a", "tag", 3)).unwrap();

        queue.cancel_job(&id).unwrap();
        let job = queue.get_job(&id).unwrap();
        assert_eq!(job.status, BatchJobStatus::Cancelled);
        assert!(job
            .items
            .iter()
            .all(|i| i.status == BatchItemStatus::Cancelled));
    }

    #[test]
    fn test_cancel_single_item() {
        let queue: BatchQueue<String> = BatchQueue::new();
        let id = queue.enqueue(make_job("model-a", "tag", 3)).unwrap();

        queue.cancel_item(&id, "item-1").unwrap();
        let job = queue.get_job(&id).unwrap();
        assert_eq!(job.items[0].status, BatchItemStatus::Pending);
        assert_eq!(job.items[1].status, BatchItemStatus::Cancelled);
        assert_eq!(job.items[2].status, BatchItemStatus::Pending);
    }

    #[test]
    fn test_retry_failed() {
        let queue: BatchQueue<String> = BatchQueue::new();
        let id = queue.enqueue(make_job("model-a", "tag", 2)).unwrap();
        queue.mark_running(&id).unwrap();

        queue
            .update_item(&id, "item-0", BatchItemStatus::Completed, None, Some(1000))
            .unwrap();
        queue
            .update_item(
                &id,
                "item-1",
                BatchItemStatus::Failed,
                Some("err".to_string()),
                None,
            )
            .unwrap();
        queue.mark_completed(&id).unwrap();

        queue.retry_failed(&id).unwrap();
        let job = queue.get_job(&id).unwrap();
        assert_eq!(job.status, BatchJobStatus::Queued);
        assert_eq!(job.items[1].status, BatchItemStatus::Pending);
        assert!(job.items[1].error.is_none());
    }

    #[test]
    fn test_model_aware_reordering() {
        let queue: BatchQueue<String> = BatchQueue::new();
        queue.enqueue(make_job("model-b", "tag", 1)).unwrap();
        queue.enqueue(make_job("model-a", "caption", 1)).unwrap();
        queue.enqueue(make_job("model-b", "caption", 1)).unwrap();

        let jobs = queue.list_jobs();
        // After reordering: model-a first, then model-b jobs
        assert_eq!(jobs[0].resource_key, "model-a");
        assert_eq!(jobs[1].resource_key, "model-b");
        assert_eq!(jobs[2].resource_key, "model-b");
    }

    #[test]
    fn test_reorder_preserves_running_jobs() {
        let queue: BatchQueue<String> = BatchQueue::new();
        let id1 = queue.enqueue(make_job("model-b", "tag", 1)).unwrap();
        queue.mark_running(&id1).unwrap();

        // Running job should not be reordered
        queue.enqueue(make_job("model-a", "tag", 1)).unwrap();
        queue.enqueue(make_job("model-b", "tag", 1)).unwrap();

        let jobs = queue.list_jobs();
        assert_eq!(jobs[0].resource_key, "model-b"); // running, stays first
        assert_eq!(jobs[0].status, BatchJobStatus::Running);
        // Queued jobs reordered: model-a before model-b
        assert_eq!(jobs[1].resource_key, "model-a");
        assert_eq!(jobs[2].resource_key, "model-b");
    }

    #[test]
    fn test_list_and_count() {
        let queue: BatchQueue<String> = BatchQueue::new();
        assert_eq!(queue.queued_count(), 0);
        assert!(!queue.has_running_job());

        let id = queue.enqueue(make_job("model-a", "tag", 1)).unwrap();
        assert_eq!(queue.queued_count(), 1);

        queue.mark_running(&id).unwrap();
        assert!(queue.has_running_job());
        assert_eq!(queue.queued_count(), 0);
    }

    #[test]
    fn test_eta_integration() {
        let queue: BatchQueue<String> = BatchQueue::new();
        let id = queue.enqueue(make_job("model-a", "tag", 3)).unwrap();
        queue.mark_running(&id).unwrap();

        // No ETA data yet
        assert!(queue.estimate_remaining_ms(&id).is_none());

        // Complete first item with timing
        queue
            .update_item(&id, "item-0", BatchItemStatus::Completed, None, Some(1000))
            .unwrap();

        // Now we have data: 2 remaining items * 1000ms avg = 2000ms
        let eta = queue.estimate_remaining_ms(&id);
        assert_eq!(eta, Some(2000));
    }
}
