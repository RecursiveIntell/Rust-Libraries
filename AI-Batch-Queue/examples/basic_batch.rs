use ai_batch_queue::*;

#[allow(dead_code)]
struct FileProcessor;

impl BatchItemHandler<String> for FileProcessor {
    async fn process(
        &self,
        data: &String,
        resource_key: &str,
        operation: &str,
    ) -> anyhow::Result<ItemResult> {
        println!("[{}] {} file: {}", resource_key, operation, data);
        // Simulate processing
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        Ok(ItemResult::success_with_output(format!(
            "Processed {} with {}",
            data, resource_key
        )))
    }

    fn should_skip(&self, _data: &String, _operation: &str) -> bool {
        false
    }
}

fn main() {
    // Create the queue
    let queue: BatchQueue<String> = BatchQueue::new();

    // Build a batch job
    let job = build_job(
        "llava:13b",           // resource key (model name)
        "tag",                 // operation
        OverwritePolicy::Skip, // skip already-processed items
        vec![
            ("img-1".into(), "/photos/cat.jpg".into(), SizeBucket::Medium),
            ("img-2".into(), "/photos/dog.jpg".into(), SizeBucket::Medium),
            (
                "img-3".into(),
                "/photos/sunset.jpg".into(),
                SizeBucket::Large,
            ),
        ],
    );

    let job_id = queue.enqueue(job).unwrap();
    println!("Queued batch job: {}", job_id);

    let jobs = queue.list_jobs();
    println!("Total jobs in queue: {}", jobs.len());
    println!("Items in first job: {}", jobs[0].items.len());

    // In a real Tauri app:
    // app.manage(queue);
    // executor::spawn::<String, FileProcessor>(app.handle().clone(), FileProcessor);
}
