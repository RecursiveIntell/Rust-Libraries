# Tauri Queue

Production-grade background job queue system for Tauri 2 applications.

## Features

- **Priority-based scheduling** — High, Normal, Low priority queues with FIFO ordering within each level
- **SQLite persistence** — Jobs survive app crashes and restarts
- **Hardware throttling** — Configurable cooldown between jobs and max consecutive runs
- **Real-time cancellation** — Cancel jobs during execution via cooperative checking
- **Progress tracking** — Emit progress events to the frontend via Tauri's event system
- **Pause/Resume** — Pause the queue without losing jobs
- **Crash recovery** — Automatically requeue interrupted jobs on startup

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
tauri-queue = "0.1"
```

## Quick Start

### 1. Define Your Job Type

```rust
use serde::{Deserialize, Serialize};
use tauri_queue::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EmailJob {
    to: String,
    subject: String,
    body: String,
}

impl JobHandler for EmailJob {
    async fn execute(&self, ctx: &JobContext) -> Result<JobResult, QueueError> {
        // Your job logic here
        println!("Sending email to: {}", self.to);

        // Emit progress (optional)
        ctx.emit_progress(50, 100)?;

        // Check for cancellation (optional)
        if ctx.is_cancelled() {
            return Err(QueueError::Cancelled);
        }

        Ok(JobResult::success())
    }
}
```

### 2. Set Up Queue in Tauri App

```rust
use tauri_queue::*;
use std::sync::Arc;

#[tauri::command]
async fn add_email_job(
    queue: tauri::State<'_, Arc<QueueManager>>,
    to: String,
    subject: String,
    body: String,
) -> Result<String, String> {
    let job = QueueJob::new(EmailJob { to, subject, body })
        .with_priority(QueuePriority::High);

    queue.add(job).map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let config = QueueConfig::builder()
                .with_db_path(app.path().app_data_dir()?.join("queue.db"))
                .with_cooldown(std::time::Duration::from_secs(2))
                .with_max_consecutive(10)
                .build();

            let queue = QueueManager::new(config)
                .expect("Failed to create queue");

            // Spawn executor and store in managed state
            let queue = queue.spawn::<EmailJob>(app.handle().clone());
            app.manage(queue);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![add_email_job])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 3. Listen to Events in Frontend

```typescript
import { listen } from '@tauri-apps/api/event';

await listen('queue:job_started', (event) => {
  console.log('Job started:', event.payload.jobId);
});

await listen('queue:job_progress', (event) => {
  const { jobId, currentStep, totalSteps, progress } = event.payload;
  console.log(`${jobId}: ${Math.round(progress * 100)}%`);
});

await listen('queue:job_completed', (event) => {
  console.log('Job completed:', event.payload.jobId);
});

await listen('queue:job_failed', (event) => {
  console.error('Job failed:', event.payload.error);
});

await listen('queue:job_cancelled', (event) => {
  console.log('Job cancelled:', event.payload.jobId);
});
```

## Configuration

```rust
use std::time::Duration;
use std::path::PathBuf;

let config = QueueConfig::builder()
    .with_db_path(PathBuf::from("queue.db"))   // Persistent storage (omit for in-memory)
    .with_cooldown(Duration::from_secs(5))      // Wait 5s between jobs
    .with_max_consecutive(20)                   // Max 20 jobs before forced cooldown
    .with_poll_interval(Duration::from_secs(3)) // Check for new jobs every 3s
    .build();
```

| Option | Default | Description |
|--------|---------|-------------|
| `db_path` | `None` (in-memory) | Path to SQLite database file |
| `cooldown` | `0s` | Pause between consecutive job executions |
| `max_consecutive` | `0` (unlimited) | Max jobs before forced cooldown |
| `poll_interval` | `3s` | How often to check for pending jobs |

## API Reference

### QueueManager

| Method | Description |
|--------|-------------|
| `new(config)` | Create a new queue manager |
| `add(job)` | Add a job to the queue, returns job ID |
| `cancel(job_id)` | Cancel a pending or processing job |
| `reorder(job_id, priority)` | Change priority of a pending job |
| `pause()` | Pause the queue (current job finishes) |
| `resume()` | Resume a paused queue |
| `is_paused()` | Check if queue is paused |
| `list_jobs()` | Get all jobs as `(id, status)` pairs |
| `list_jobs_with_data()` | Get all jobs with their JSON data |
| `prune(days)` | Delete old completed/failed/cancelled jobs |
| `spawn::<H>(app_handle)` | Start executor, returns `Arc<Self>` |

### JobHandler Trait

```rust
pub trait JobHandler: Send + Sync + Serialize + DeserializeOwned + Clone {
    async fn execute(&self, ctx: &JobContext) -> Result<JobResult, QueueError>;
}
```

### JobContext

| Method | Description |
|--------|-------------|
| `emit_progress(current, total)` | Emit progress event to frontend |
| `is_cancelled()` | Check if this job has been cancelled |
| `job_id` | The ID of the current job |
| `app_handle` | Tauri AppHandle for custom event emission |

### Events

| Event | Payload | Description |
|-------|---------|-------------|
| `queue:job_started` | `{ jobId }` | Job execution started |
| `queue:job_progress` | `{ jobId, currentStep, totalSteps, progress }` | Progress update |
| `queue:job_completed` | `{ jobId, output? }` | Job finished successfully |
| `queue:job_failed` | `{ jobId, error }` | Job failed with error |
| `queue:job_cancelled` | `{ jobId }` | Job was cancelled |

## Examples

See the `examples/` directory:

- **`basic_usage.rs`** — Simple job queue with priority
- **`with_cooldown.rs`** — Rate limiting with cooldown between jobs
- **`with_cancellation.rs`** — Cancellable long-running jobs
- **`with_persistence.rs`** — Persistent queue across app restarts

Run an example:

```bash
cargo run --example basic_usage
```

## License

MIT
