use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri_queue::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LongRunningJob {
    duration_secs: u64,
}

impl JobHandler for LongRunningJob {
    async fn execute(&self, ctx: &JobContext) -> Result<JobResult, QueueError> {
        for i in 0..self.duration_secs {
            // Check for cancellation each second
            if ctx.is_cancelled() {
                println!("Job {} cancelled at step {}", ctx.job_id, i);
                return Err(QueueError::Cancelled);
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
            ctx.emit_progress(i as u32, self.duration_secs as u32)?;
            println!("Step {}/{}", i + 1, self.duration_secs);
        }
        Ok(JobResult::success())
    }
}

fn main() {
    let config = QueueConfig::default();
    let queue = QueueManager::new(config).unwrap();

    let job = QueueJob::new(LongRunningJob { duration_secs: 30 }).with_id("long-job-1".to_string());

    let job_id = queue.add(job).unwrap();
    println!("Queued job: {}", job_id);

    // In a real app, cancel from another thread/command:
    // queue.cancel("long-job-1").unwrap();
    //
    // The handler checks ctx.is_cancelled() each iteration
    // and returns QueueError::Cancelled when detected.
}
