# ai-batch-queue

A Rust library for managing AI batch processing jobs in Tauri 2 desktop applications. Features model-aware job reordering, size-bucketed ETA estimation, item-level status tracking, overwrite policies, and retry support.

## Features

- **Model-aware reordering** — Automatically groups jobs by resource key (e.g., model name) to minimize expensive GPU model swaps
- **Size-bucketed ETA** — Tracks processing times by (resource, operation, size) for increasingly accurate time estimates
- **Item-level tracking** — Individual status, error, and duration tracking for each item in a batch
- **Overwrite policies** — Skip already-processed items or overwrite them
- **Retry failed items** — Re-queue only the failed items in a completed job
- **Cancellation** — Cancel entire jobs or individual items
- **Tauri event integration** — Emits progress events (`ai_batch:job_started`, `ai_batch:item_progress`, `ai_batch:job_completed`) for frontend reactivity
- **Generic data type** — Works with any `Clone + Send + Sync + Serialize` data type

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
ai-batch-queue = { path = "../AI-Batch-Queue" }
```

### Define your handler

```rust
use ai_batch_queue::*;

struct ImageTagger;

impl BatchItemHandler<String> for ImageTagger {
    async fn process(
        &self,
        data: &String,
        resource_key: &str,
        operation: &str,
    ) -> anyhow::Result<ItemResult> {
        // Your AI processing logic here
        println!("[{}] {} on {}", resource_key, operation, data);
        Ok(ItemResult::success_with_output("tags: cat, sunset".into()))
    }

    fn should_skip(&self, _data: &String, _operation: &str) -> bool {
        false // Return true if this item already has results
    }
}
```

### Register with Tauri

```rust
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let queue: BatchQueue<String> = BatchQueue::new();
            app.manage(queue);

            // Spawn the background executor
            ai_batch_queue::executor::spawn::<String, _>(
                app.handle().clone(),
                ImageTagger,
            );
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running app");
}
```

### Enqueue jobs

```rust
use ai_batch_queue::*;

let queue: BatchQueue<String> = BatchQueue::new();

let job = build_job(
    "llava:13b",           // resource_key (model name)
    "tag",                 // operation
    OverwritePolicy::Skip, // skip already-processed
    vec![
        ("img-1".into(), "/photos/cat.jpg".into(), SizeBucket::Medium),
        ("img-2".into(), "/photos/dog.jpg".into(), SizeBucket::Small),
        ("img-3".into(), "/photos/sunset.jpg".into(), SizeBucket::Large),
    ],
);

let job_id = queue.enqueue(job).unwrap();
```

## Model-Aware Reordering

When you queue jobs that use different models, the queue automatically reorders them to group by resource key. This minimizes expensive GPU model swaps:

```
User queues:                    After reordering:
1. llava:13b  - tag             1. llava:13b  - tag
2. moondream  - tag      -->    2. llava:13b  - caption
3. llava:13b  - caption         3. moondream  - tag

Result: 2 model loads instead of 3 (33% fewer GPU swaps)
```

Running jobs are never reordered. Only queued jobs participate in reordering.

## Size-Bucketed ETA

ETA estimates improve as items complete. Processing times are tracked per (resource, operation, size_bucket):

| Bucket | Pixel Count |
|--------|-------------|
| Small | < 500,000 |
| Medium | 500,000 - 2,000,000 |
| Large | > 2,000,000 |
| Unknown | Dimensions unavailable |

```rust
// After some items complete with timing data:
let eta_ms = queue.estimate_remaining_ms(&job_id);
// Returns Some(estimated_ms) or None if no data yet
```

If no data exists for a specific size bucket, the estimator falls back to the `Unknown` bucket for that resource/operation.

## API Reference

### `BatchQueue<D>`

| Method | Description |
|--------|-------------|
| `new()` | Create an empty queue |
| `enqueue(job)` | Add a job (auto-reorders queued jobs) |
| `next_queued()` | Get the next queued job |
| `mark_running(job_id)` | Set job status to Running |
| `update_item(job_id, item_id, status, error, duration_ms)` | Update item status (auto-records ETA on completion) |
| `mark_completed(job_id)` | Complete job, returns `BatchCompletionSummary` |
| `cancel_job(job_id)` | Cancel entire job (pending items only) |
| `cancel_item(job_id, item_id)` | Cancel a single pending item |
| `retry_failed(job_id)` | Re-queue failed items |
| `list_jobs()` | Get all jobs (cloned snapshot) |
| `get_job(job_id)` | Get a specific job |
| `estimate_remaining_ms(job_id)` | Estimate remaining time |
| `eta_sample_count(resource, op, size)` | Get number of ETA data points |
| `has_running_job()` | Check if any job is running |
| `queued_count()` | Count of queued jobs |

### `BatchItemHandler<D>` Trait

```rust
pub trait BatchItemHandler<D>: Send + Sync + 'static
where
    D: Clone + Send + Sync + Serialize,
{
    // Required: process a single item
    fn process(
        &self,
        data: &D,
        resource_key: &str,
        operation: &str,
    ) -> impl Future<Output = anyhow::Result<ItemResult>> + Send;

    // Optional: return true to skip this item (for OverwritePolicy::Skip)
    fn should_skip(&self, _data: &D, _operation: &str) -> bool {
        false
    }
}
```

### `build_job()` Helper

```rust
let job = build_job(
    "model-name",          // resource_key
    "operation",           // operation name
    OverwritePolicy::Skip, // or OverwritePolicy::Overwrite
    vec![
        ("id".into(), data, SizeBucket::Medium),
        // ...
    ],
);
```

### Types

| Type | Description |
|------|-------------|
| `BatchJob<D>` | A batch job containing items to process |
| `BatchItem<D>` | A single item with status, error, duration, size |
| `BatchItemStatus` | `Pending`, `Running`, `Completed`, `Failed`, `Skipped`, `Cancelled` |
| `BatchJobStatus` | `Queued`, `Running`, `Completed`, `CompletedWithErrors`, `Cancelled` |
| `OverwritePolicy` | `Skip` (skip existing), `Overwrite` (reprocess all) |
| `SizeBucket` | `Small`, `Medium`, `Large`, `Unknown` |
| `ItemResult` | Processing result with `success`, `output`, `error` fields |
| `BatchCompletionSummary` | Job completion stats (succeeded, failed, skipped, duration) |

### Tauri Events

| Event | Payload | When |
|-------|---------|------|
| `ai_batch:job_started` | `{ jobId, operation, resourceKey, totalItems }` | Job begins processing |
| `ai_batch:item_progress` | `{ jobId, itemId, status, completed, total, error, durationMs, etaRemainingMs }` | Each item completes |
| `ai_batch:job_completed` | `{ summary: BatchCompletionSummary }` | All items processed |

### Frontend (TypeScript)

```typescript
import { listen } from '@tauri-apps/api/event';

// Listen for progress updates
const unlisten = await listen('ai_batch:item_progress', (event) => {
    const { jobId, itemId, completed, total, etaRemainingMs } = event.payload;
    console.log(`${completed}/${total} - ETA: ${etaRemainingMs}ms`);
});

// Listen for job completion
await listen('ai_batch:job_completed', (event) => {
    const { summary } = event.payload;
    console.log(`Done: ${summary.succeeded} ok, ${summary.failed} failed`);
});
```

## Examples

```bash
cargo run --example basic_batch
cargo run --example model_optimization
cargo run --example eta_tracking
```

## Testing

```bash
cargo test                    # 47 tests (18 unit + 28 integration + 1 doc-test)
cargo clippy -- -D warnings   # Zero warnings
cargo fmt --check             # Formatted
```

## License

MIT
