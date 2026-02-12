use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::queue::BatchQueue;
use crate::types::*;
use crate::BatchItemHandler;

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(2);

// -- Tauri event payloads --

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchJobStartedEvent {
    job_id: String,
    operation: String,
    resource_key: String,
    total_items: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchItemProgressEvent {
    job_id: String,
    item_id: String,
    status: BatchItemStatus,
    completed: usize,
    total: usize,
    error: Option<String>,
    duration_ms: Option<u64>,
    eta_remaining_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchJobCompletedEvent {
    summary: BatchCompletionSummary,
}

/// Spawn the background batch executor as a tokio task.
///
/// The executor polls the queue at `poll_interval` (default 2s) and
/// processes one batch at a time. Progress events are emitted for each item.
///
/// The `BatchQueue<D>` must be registered in Tauri's managed state.
pub fn spawn<D, H>(app_handle: AppHandle, handler: H)
where
    D: Clone + Send + Sync + Serialize + serde::de::DeserializeOwned + 'static,
    H: BatchItemHandler<D> + 'static,
{
    spawn_with_interval(app_handle, handler, DEFAULT_POLL_INTERVAL);
}

/// Spawn with a custom poll interval.
pub fn spawn_with_interval<D, H>(app_handle: AppHandle, handler: H, poll_interval: Duration)
where
    D: Clone + Send + Sync + Serialize + serde::de::DeserializeOwned + 'static,
    H: BatchItemHandler<D> + 'static,
{
    tauri::async_runtime::spawn(async move {
        run_loop(app_handle, handler, poll_interval).await;
    });
}

async fn run_loop<D, H>(app_handle: AppHandle, handler: H, poll_interval: Duration)
where
    D: Clone + Send + Sync + Serialize + serde::de::DeserializeOwned + 'static,
    H: BatchItemHandler<D>,
{
    loop {
        tokio::time::sleep(poll_interval).await;

        let queue = match app_handle.try_state::<BatchQueue<D>>() {
            Some(q) => q,
            None => continue,
        };

        if queue.has_running_job() {
            continue;
        }

        let job = match queue.next_queued() {
            Some(j) => j,
            None => continue,
        };

        process_batch_job(&app_handle, &queue, &handler, &job).await;
    }
}

async fn process_batch_job<D, H>(
    app_handle: &AppHandle,
    queue: &BatchQueue<D>,
    handler: &H,
    job: &BatchJob<D>,
) where
    D: Clone + Send + Sync + Serialize + serde::de::DeserializeOwned + 'static,
    H: BatchItemHandler<D>,
{
    let job_id = job.id.clone();

    if let Err(e) = queue.mark_running(&job_id) {
        eprintln!(
            "[ai-batch-queue] Failed to mark job {} as running: {}",
            job_id, e
        );
        return;
    }

    let _ = app_handle.emit(
        "ai_batch:job_started",
        BatchJobStartedEvent {
            job_id: job_id.clone(),
            operation: job.operation.clone(),
            resource_key: job.resource_key.clone(),
            total_items: job.items.len(),
        },
    );

    let total = job.items.len();
    let mut completed_count: usize = 0;

    for item in &job.items {
        // Check if the item or job was cancelled
        if let Some(current_job) = queue.get_job(&job_id) {
            if let Some(ci) = current_job.items.iter().find(|i| i.id == item.id) {
                if ci.status == BatchItemStatus::Cancelled {
                    completed_count += 1;
                    continue;
                }
            }
            if current_job.status == BatchJobStatus::Cancelled {
                break;
            }
        }

        // Check overwrite/skip policy
        if job.overwrite_policy == OverwritePolicy::Skip
            && handler.should_skip(&item.data, &job.operation)
        {
            let _ = queue.update_item(
                &job_id,
                &item.id,
                BatchItemStatus::Skipped,
                Some("Skipped: already has data".to_string()),
                None,
            );
            completed_count += 1;

            let eta = queue.estimate_remaining_ms(&job_id);
            let _ = app_handle.emit(
                "ai_batch:item_progress",
                BatchItemProgressEvent {
                    job_id: job_id.clone(),
                    item_id: item.id.clone(),
                    status: BatchItemStatus::Skipped,
                    completed: completed_count,
                    total,
                    error: Some("Skipped".to_string()),
                    duration_ms: None,
                    eta_remaining_ms: eta,
                },
            );
            continue;
        }

        // Mark item as running
        let _ = queue.update_item(&job_id, &item.id, BatchItemStatus::Running, None, None);

        // Process the item
        let start = Instant::now();
        let result = handler
            .process(&item.data, &job.resource_key, &job.operation)
            .await;
        let duration_ms = start.elapsed().as_millis() as u64;

        let (status, error) = match result {
            Ok(item_result) => {
                if item_result.success {
                    (BatchItemStatus::Completed, None)
                } else {
                    (
                        BatchItemStatus::Failed,
                        item_result.error.or(Some("Unknown error".to_string())),
                    )
                }
            }
            Err(e) => (BatchItemStatus::Failed, Some(format!("{:#}", e))),
        };

        let _ = queue.update_item(
            &job_id,
            &item.id,
            status.clone(),
            error.clone(),
            Some(duration_ms),
        );

        completed_count += 1;
        let eta = queue.estimate_remaining_ms(&job_id);
        let _ = app_handle.emit(
            "ai_batch:item_progress",
            BatchItemProgressEvent {
                job_id: job_id.clone(),
                item_id: item.id.clone(),
                status,
                completed: completed_count,
                total,
                error,
                duration_ms: Some(duration_ms),
                eta_remaining_ms: eta,
            },
        );
    }

    match queue.mark_completed(&job_id) {
        Ok(Some(summary)) => {
            let _ = app_handle.emit("ai_batch:job_completed", BatchJobCompletedEvent { summary });
        }
        Ok(None) => {}
        Err(e) => eprintln!(
            "[ai-batch-queue] Failed to mark job {} completed: {}",
            job_id, e
        ),
    }
}
