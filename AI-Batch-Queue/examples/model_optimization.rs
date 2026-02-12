use ai_batch_queue::*;

#[allow(dead_code)]
struct DummyProcessor;

impl BatchItemHandler<String> for DummyProcessor {
    async fn process(
        &self,
        _data: &String,
        _resource_key: &str,
        _operation: &str,
    ) -> anyhow::Result<ItemResult> {
        Ok(ItemResult::success())
    }
}

fn main() {
    let queue: BatchQueue<String> = BatchQueue::new();

    // Simulate user queuing jobs in arbitrary order:
    println!("User queues jobs in this order:");
    println!("  1. llava:13b  - 100 images (tag)");
    println!("  2. moondream  - 50 images  (tag)");
    println!("  3. llava:13b  - 80 images  (caption)");
    println!();

    // Job 1: llava:13b tag
    let items1: Vec<_> = (0..100)
        .map(|i| {
            (
                format!("a-{}", i),
                format!("file-a-{}.jpg", i),
                SizeBucket::Medium,
            )
        })
        .collect();
    queue
        .enqueue(build_job("llava:13b", "tag", OverwritePolicy::Skip, items1))
        .unwrap();

    // Job 2: moondream tag
    let items2: Vec<_> = (0..50)
        .map(|i| {
            (
                format!("b-{}", i),
                format!("file-b-{}.jpg", i),
                SizeBucket::Medium,
            )
        })
        .collect();
    queue
        .enqueue(build_job("moondream", "tag", OverwritePolicy::Skip, items2))
        .unwrap();

    // Job 3: llava:13b caption
    let items3: Vec<_> = (0..80)
        .map(|i| {
            (
                format!("c-{}", i),
                format!("file-c-{}.jpg", i),
                SizeBucket::Medium,
            )
        })
        .collect();
    queue
        .enqueue(build_job(
            "llava:13b",
            "caption",
            OverwritePolicy::Skip,
            items3,
        ))
        .unwrap();

    // Check the reordered queue
    let jobs = queue.list_jobs();
    println!("After model-aware reordering:");
    for (i, job) in jobs.iter().enumerate() {
        let tag = if job.reordered { " (reordered)" } else { "" };
        println!(
            "  {}. {} - {} items ({}){tag}",
            i + 1,
            job.resource_key,
            job.items.len(),
            job.operation,
        );
    }

    println!();
    println!("Result: llava:13b jobs grouped together = 2 model loads instead of 3!");
    println!("        That's 33% fewer expensive GPU model swaps.");

    // Verify the optimization
    assert_eq!(jobs[0].resource_key, "llava:13b");
    assert_eq!(jobs[1].resource_key, "llava:13b");
    assert_eq!(jobs[2].resource_key, "moondream");
}
