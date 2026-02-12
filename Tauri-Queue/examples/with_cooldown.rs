use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri_queue::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiCallJob {
    endpoint: String,
}

impl JobHandler for ApiCallJob {
    async fn execute(&self, _ctx: &JobContext) -> Result<JobResult, QueueError> {
        println!("Calling API: {}", self.endpoint);
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(JobResult::success())
    }
}

fn main() {
    // Configure queue with 2-second cooldown and max 5 consecutive jobs
    let config = QueueConfig::builder()
        .with_cooldown(Duration::from_secs(2))
        .with_max_consecutive(5)
        .build();

    let queue = QueueManager::new(config).unwrap();

    // Add 10 jobs
    for i in 0..10 {
        let job = QueueJob::new(ApiCallJob {
            endpoint: format!("https://api.example.com/resource/{}", i),
        });
        queue.add(job).unwrap();
    }

    let jobs = queue.list_jobs().unwrap();
    println!("{} jobs queued.", jobs.len());
    println!("Will process 5, then cool down for 2 seconds, then process remaining 5.");

    // In a real Tauri app, you would call:
    // queue.spawn::<ApiCallJob>(app.handle().clone());
}
