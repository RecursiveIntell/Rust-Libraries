use serde::{Deserialize, Serialize};
use tauri_queue::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestJob {
    pub data: String,
}

impl JobHandler for TestJob {
    async fn execute(&self, _ctx: &JobContext) -> Result<JobResult, QueueError> {
        Ok(JobResult::success_with_output(self.data.clone()))
    }
}
