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
        println!("Sending email to: {}", self.to);
        println!("Subject: {}", self.subject);

        // Simulate sending email with progress
        for i in 0..10 {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            ctx.emit_progress(i, 10)?;
        }

        println!("Email sent successfully!");
        Ok(JobResult::success())
    }
}

fn main() {
    let config = QueueConfig::default();
    let queue = QueueManager::new(config).unwrap();

    // Add a normal priority job
    let job1 = QueueJob::new(EmailJob {
        to: "alice@example.com".to_string(),
        subject: "Hello".to_string(),
        body: "Test message".to_string(),
    });

    // Add a high priority job
    let job2 = QueueJob::new(EmailJob {
        to: "bob@example.com".to_string(),
        subject: "Important".to_string(),
        body: "Urgent message".to_string(),
    })
    .with_priority(QueuePriority::High);

    let id1 = queue.add(job1).unwrap();
    let id2 = queue.add(job2).unwrap();

    println!("Job 1 (Normal): {}", id1);
    println!("Job 2 (High):   {}", id2);
    println!();
    println!("High priority job will execute first.");

    let jobs = queue.list_jobs().unwrap();
    for (id, status) in &jobs {
        println!("  {} -> {}", id, status);
    }

    // In a real Tauri app, you would call:
    // queue.spawn::<EmailJob>(app.handle().clone());
}
