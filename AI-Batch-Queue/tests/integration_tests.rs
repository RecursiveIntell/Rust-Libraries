use ai_batch_queue::*;

fn make_items(count: usize) -> Vec<(String, String, SizeBucket)> {
    (0..count)
        .map(|i| {
            (
                format!("item-{}", i),
                format!("data-{}", i),
                SizeBucket::Medium,
            )
        })
        .collect()
}

fn make_job(resource: &str, op: &str, count: usize) -> BatchJob<String> {
    build_job(resource, op, OverwritePolicy::Skip, make_items(count))
}

// -- Queue creation --

#[test]
fn test_queue_creation() {
    let queue: BatchQueue<String> = BatchQueue::new();
    assert_eq!(queue.queued_count(), 0);
    assert!(!queue.has_running_job());
}

// -- Enqueue and list --

#[test]
fn test_enqueue_assigns_id() {
    let queue: BatchQueue<String> = BatchQueue::new();
    let id = queue.enqueue(make_job("model-a", "tag", 3)).unwrap();
    assert!(!id.is_empty());
}

#[test]
fn test_list_jobs() {
    let queue: BatchQueue<String> = BatchQueue::new();
    queue.enqueue(make_job("model-a", "tag", 2)).unwrap();
    queue.enqueue(make_job("model-b", "caption", 3)).unwrap();

    let jobs = queue.list_jobs();
    assert_eq!(jobs.len(), 2);
}

#[test]
fn test_get_job() {
    let queue: BatchQueue<String> = BatchQueue::new();
    let id = queue.enqueue(make_job("model-a", "tag", 1)).unwrap();

    let job = queue.get_job(&id).unwrap();
    assert_eq!(job.resource_key, "model-a");
    assert_eq!(job.operation, "tag");
    assert_eq!(job.items.len(), 1);
}

#[test]
fn test_get_nonexistent_job() {
    let queue: BatchQueue<String> = BatchQueue::new();
    assert!(queue.get_job("nonexistent").is_none());
}

// -- Next queued --

#[test]
fn test_next_queued_empty() {
    let queue: BatchQueue<String> = BatchQueue::new();
    assert!(queue.next_queued().is_none());
}

#[test]
fn test_next_queued_returns_first() {
    let queue: BatchQueue<String> = BatchQueue::new();
    let id1 = queue.enqueue(make_job("model-a", "tag", 1)).unwrap();
    queue.enqueue(make_job("model-a", "caption", 1)).unwrap();

    let next = queue.next_queued().unwrap();
    assert_eq!(next.id, id1);
}

// -- Status lifecycle --

#[test]
fn test_mark_running() {
    let queue: BatchQueue<String> = BatchQueue::new();
    let id = queue.enqueue(make_job("model-a", "tag", 1)).unwrap();

    queue.mark_running(&id).unwrap();
    let job = queue.get_job(&id).unwrap();
    assert_eq!(job.status, BatchJobStatus::Running);
    assert!(job.started_at.is_some());
    assert!(queue.has_running_job());
}

#[test]
fn test_complete_all_success() {
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
    assert_eq!(summary.skipped, 0);
    assert_eq!(summary.total_duration_ms, 3000);
    assert_eq!(summary.avg_duration_ms, 1500);
    assert_eq!(summary.operation, "tag");
    assert_eq!(summary.resource_key, "model-a");

    let job = queue.get_job(&id).unwrap();
    assert_eq!(job.status, BatchJobStatus::Completed);
    assert!(job.completed_at.is_some());
}

#[test]
fn test_complete_with_errors() {
    let queue: BatchQueue<String> = BatchQueue::new();
    let id = queue.enqueue(make_job("model-a", "tag", 3)).unwrap();
    queue.mark_running(&id).unwrap();

    queue
        .update_item(&id, "item-0", BatchItemStatus::Completed, None, Some(1000))
        .unwrap();
    queue
        .update_item(
            &id,
            "item-1",
            BatchItemStatus::Failed,
            Some("timeout".into()),
            Some(5000),
        )
        .unwrap();
    queue
        .update_item(&id, "item-2", BatchItemStatus::Completed, None, Some(1000))
        .unwrap();

    let summary = queue.mark_completed(&id).unwrap().unwrap();
    assert_eq!(summary.succeeded, 2);
    assert_eq!(summary.failed, 1);

    let job = queue.get_job(&id).unwrap();
    assert_eq!(job.status, BatchJobStatus::CompletedWithErrors);
}

// -- Cancellation --

#[test]
fn test_cancel_queued_job() {
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
fn test_cancel_running_item_no_op() {
    let queue: BatchQueue<String> = BatchQueue::new();
    let id = queue.enqueue(make_job("model-a", "tag", 1)).unwrap();
    queue.mark_running(&id).unwrap();

    // Mark item as running
    queue
        .update_item(&id, "item-0", BatchItemStatus::Running, None, None)
        .unwrap();

    // Trying to cancel a running item should be a no-op
    queue.cancel_item(&id, "item-0").unwrap();
    let job = queue.get_job(&id).unwrap();
    assert_eq!(job.items[0].status, BatchItemStatus::Running);
}

// -- Retry --

#[test]
fn test_retry_failed() {
    let queue: BatchQueue<String> = BatchQueue::new();
    let id = queue.enqueue(make_job("model-a", "tag", 3)).unwrap();
    queue.mark_running(&id).unwrap();

    queue
        .update_item(&id, "item-0", BatchItemStatus::Completed, None, Some(1000))
        .unwrap();
    queue
        .update_item(
            &id,
            "item-1",
            BatchItemStatus::Failed,
            Some("err".into()),
            None,
        )
        .unwrap();
    queue
        .update_item(&id, "item-2", BatchItemStatus::Completed, None, Some(1000))
        .unwrap();
    queue.mark_completed(&id).unwrap();

    queue.retry_failed(&id).unwrap();
    let job = queue.get_job(&id).unwrap();
    assert_eq!(job.status, BatchJobStatus::Queued);
    assert_eq!(job.items[0].status, BatchItemStatus::Completed);
    assert_eq!(job.items[1].status, BatchItemStatus::Pending);
    assert!(job.items[1].error.is_none());
    assert_eq!(job.items[2].status, BatchItemStatus::Completed);
}

#[test]
fn test_retry_no_failed_items_errors() {
    let queue: BatchQueue<String> = BatchQueue::new();
    let id = queue.enqueue(make_job("model-a", "tag", 1)).unwrap();
    queue.mark_running(&id).unwrap();
    queue
        .update_item(&id, "item-0", BatchItemStatus::Completed, None, Some(1000))
        .unwrap();
    queue.mark_completed(&id).unwrap();

    let result = queue.retry_failed(&id);
    assert!(result.is_err());
}

// -- Model-aware reordering --

#[test]
fn test_reorder_groups_by_resource() {
    let queue: BatchQueue<String> = BatchQueue::new();
    queue.enqueue(make_job("model-b", "tag", 1)).unwrap();
    queue.enqueue(make_job("model-a", "caption", 1)).unwrap();
    queue.enqueue(make_job("model-b", "caption", 1)).unwrap();

    let jobs = queue.list_jobs();
    assert_eq!(jobs[0].resource_key, "model-a");
    assert_eq!(jobs[1].resource_key, "model-b");
    assert_eq!(jobs[2].resource_key, "model-b");
}

#[test]
fn test_reorder_marks_jobs() {
    let queue: BatchQueue<String> = BatchQueue::new();
    queue.enqueue(make_job("model-b", "tag", 1)).unwrap();
    queue.enqueue(make_job("model-a", "tag", 1)).unwrap();

    let jobs = queue.list_jobs();
    assert!(jobs[0].reordered);
    assert!(jobs[0].reorder_note.is_some());
}

#[test]
fn test_reorder_preserves_running() {
    let queue: BatchQueue<String> = BatchQueue::new();
    let id1 = queue.enqueue(make_job("model-c", "tag", 1)).unwrap();
    queue.mark_running(&id1).unwrap();

    queue.enqueue(make_job("model-a", "tag", 1)).unwrap();
    queue.enqueue(make_job("model-c", "tag", 1)).unwrap();

    let jobs = queue.list_jobs();
    assert_eq!(jobs[0].id, id1); // Running job stays at its position
    assert_eq!(jobs[0].status, BatchJobStatus::Running);
    assert_eq!(jobs[1].resource_key, "model-a"); // Queued reordered
    assert_eq!(jobs[2].resource_key, "model-c");
}

#[test]
fn test_no_reorder_same_resource() {
    let queue: BatchQueue<String> = BatchQueue::new();
    queue.enqueue(make_job("model-a", "tag", 1)).unwrap();
    queue.enqueue(make_job("model-a", "caption", 1)).unwrap();

    let jobs = queue.list_jobs();
    // Same resource, no reorder needed
    assert!(!jobs[0].reordered);
    assert!(!jobs[1].reordered);
}

// -- ETA estimation --

#[test]
fn test_eta_no_data() {
    let queue: BatchQueue<String> = BatchQueue::new();
    let id = queue.enqueue(make_job("model-a", "tag", 3)).unwrap();
    queue.mark_running(&id).unwrap();

    assert!(queue.estimate_remaining_ms(&id).is_none());
}

#[test]
fn test_eta_after_completions() {
    let queue: BatchQueue<String> = BatchQueue::new();
    let id = queue.enqueue(make_job("model-a", "tag", 3)).unwrap();
    queue.mark_running(&id).unwrap();

    queue
        .update_item(&id, "item-0", BatchItemStatus::Completed, None, Some(1000))
        .unwrap();

    // 2 remaining * 1000ms avg = 2000ms
    let eta = queue.estimate_remaining_ms(&id);
    assert_eq!(eta, Some(2000));
}

#[test]
fn test_eta_zero_when_all_done() {
    let queue: BatchQueue<String> = BatchQueue::new();
    let id = queue.enqueue(make_job("model-a", "tag", 1)).unwrap();
    queue.mark_running(&id).unwrap();

    queue
        .update_item(&id, "item-0", BatchItemStatus::Completed, None, Some(500))
        .unwrap();

    assert_eq!(queue.estimate_remaining_ms(&id), Some(0));
}

// -- Counts --

#[test]
fn test_queued_count() {
    let queue: BatchQueue<String> = BatchQueue::new();
    assert_eq!(queue.queued_count(), 0);

    queue.enqueue(make_job("a", "tag", 1)).unwrap();
    assert_eq!(queue.queued_count(), 1);

    queue.enqueue(make_job("b", "tag", 1)).unwrap();
    assert_eq!(queue.queued_count(), 2);
}

#[test]
fn test_has_running_job() {
    let queue: BatchQueue<String> = BatchQueue::new();
    assert!(!queue.has_running_job());

    let id = queue.enqueue(make_job("a", "tag", 1)).unwrap();
    assert!(!queue.has_running_job());

    queue.mark_running(&id).unwrap();
    assert!(queue.has_running_job());
}

// -- build_job helper --

#[test]
fn test_build_job_helper() {
    let job = build_job(
        "model-x",
        "embed",
        OverwritePolicy::Overwrite,
        vec![
            ("a".into(), "data-a".to_string(), SizeBucket::Small),
            ("b".into(), "data-b".to_string(), SizeBucket::Large),
        ],
    );

    assert_eq!(job.resource_key, "model-x");
    assert_eq!(job.operation, "embed");
    assert_eq!(job.overwrite_policy, OverwritePolicy::Overwrite);
    assert_eq!(job.items.len(), 2);
    assert_eq!(job.items[0].id, "a");
    assert_eq!(job.items[0].data, "data-a");
    assert_eq!(job.items[0].size_bucket, SizeBucket::Small);
    assert_eq!(job.items[1].size_bucket, SizeBucket::Large);
}

// -- Type tests --

#[test]
fn test_size_bucket_classification() {
    assert_eq!(SizeBucket::from_pixel_count(100_000), SizeBucket::Small);
    assert_eq!(SizeBucket::from_pixel_count(1_000_000), SizeBucket::Medium);
    assert_eq!(SizeBucket::from_pixel_count(5_000_000), SizeBucket::Large);

    assert_eq!(
        SizeBucket::from_dimensions(Some(500), Some(500)),
        SizeBucket::Small
    );
    assert_eq!(
        SizeBucket::from_dimensions(Some(1000), Some(1000)),
        SizeBucket::Medium
    );
    assert_eq!(
        SizeBucket::from_dimensions(Some(2000), Some(2000)),
        SizeBucket::Large
    );
    assert_eq!(
        SizeBucket::from_dimensions(None, Some(500)),
        SizeBucket::Unknown
    );
    assert_eq!(
        SizeBucket::from_dimensions(Some(500), None),
        SizeBucket::Unknown
    );
    assert_eq!(SizeBucket::from_dimensions(None, None), SizeBucket::Unknown);
}

#[test]
fn test_item_result_variants() {
    let success = ItemResult::success();
    assert!(success.success);
    assert!(success.output.is_none());

    let with_output = ItemResult::success_with_output("tags: cat, dog".into());
    assert!(with_output.success);
    assert_eq!(with_output.output.as_deref(), Some("tags: cat, dog"));

    let failure = ItemResult::failure("connection timeout".into());
    assert!(!failure.success);
    assert_eq!(failure.error.as_deref(), Some("connection timeout"));
}

#[test]
fn test_event_serialization() {
    // BatchCompletionSummary should serialize with camelCase
    let summary = BatchCompletionSummary {
        job_id: "j1".into(),
        operation: "tag".into(),
        resource_key: "llava".into(),
        total: 10,
        succeeded: 8,
        failed: 1,
        skipped: 1,
        total_duration_ms: 10000,
        avg_duration_ms: 1111,
    };
    let json = serde_json::to_string(&summary).unwrap();
    assert!(json.contains("jobId"));
    assert!(json.contains("resourceKey"));
    assert!(json.contains("totalDurationMs"));
    assert!(json.contains("avgDurationMs"));
}
