use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri_queue::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DataProcessingJob {
    file_path: String,
}

impl JobHandler for DataProcessingJob {
    async fn execute(&self, _ctx: &JobContext) -> Result<JobResult, QueueError> {
        println!("Processing file: {}", self.file_path);
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        Ok(JobResult::success_with_output(format!(
            "Processed {}",
            self.file_path
        )))
    }
}

fn main() {
    // Use a persistent database file
    let config = QueueConfig::builder()
        .with_db_path(PathBuf::from("/tmp/tauri-queue-example.db"))
        .build();

    let queue = QueueManager::new(config).unwrap();

    // Check existing jobs from previous runs
    let existing = queue.list_jobs().unwrap();
    if !existing.is_empty() {
        println!("Found {} jobs from previous run:", existing.len());
        for (id, status) in &existing {
            println!("  {} -> {}", id, status);
        }
        println!();
    }

    // Add new jobs
    for i in 0..5 {
        let job = QueueJob::new(DataProcessingJob {
            file_path: format!("/data/file_{}.csv", i),
        });
        queue.add(job).unwrap();
    }

    let all_jobs = queue.list_jobs().unwrap();
    println!("{} total jobs in persistent queue.", all_jobs.len());
    println!("Jobs survive app restarts! Run this example again to see.");

    // Prune old completed jobs (older than 7 days)
    let pruned = queue.prune(7).unwrap();
    if pruned > 0 {
        println!("Pruned {} old jobs.", pruned);
    }

    // In a real Tauri app, you would call:
    // queue.spawn::<DataProcessingJob>(app.handle().clone());
}
