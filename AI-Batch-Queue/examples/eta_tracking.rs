use ai_batch_queue::*;

#[allow(dead_code)]
struct SimulatedProcessor;

impl BatchItemHandler<String> for SimulatedProcessor {
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

    // Create a job with items of different sizes
    let items = vec![
        ("img-1".into(), "small.jpg".into(), SizeBucket::Small),
        ("img-2".into(), "medium.jpg".into(), SizeBucket::Medium),
        ("img-3".into(), "large.jpg".into(), SizeBucket::Large),
        ("img-4".into(), "small2.jpg".into(), SizeBucket::Small),
        ("img-5".into(), "medium2.jpg".into(), SizeBucket::Medium),
    ];

    let job_id = queue
        .enqueue(build_job("llava:13b", "tag", OverwritePolicy::Skip, items))
        .unwrap();

    queue.mark_running(&job_id).unwrap();

    // Before any completions: no ETA data available
    let eta = queue.estimate_remaining_ms(&job_id);
    println!("ETA before any data: {:?}", eta);

    // Simulate completing items with different durations per size
    println!("\nSimulating item completions...");

    // Small image: 500ms
    queue
        .update_item(
            &job_id,
            "img-1",
            BatchItemStatus::Completed,
            None,
            Some(500),
        )
        .unwrap();
    println!("  Completed small image in 500ms");

    let eta = queue.estimate_remaining_ms(&job_id);
    println!("  ETA for 4 remaining: {:?}ms", eta);

    // Medium image: 1200ms
    queue
        .update_item(
            &job_id,
            "img-2",
            BatchItemStatus::Completed,
            None,
            Some(1200),
        )
        .unwrap();
    println!("  Completed medium image in 1200ms");

    let eta = queue.estimate_remaining_ms(&job_id);
    println!("  ETA for 3 remaining: {:?}ms", eta);

    // Large image: 3000ms
    queue
        .update_item(
            &job_id,
            "img-3",
            BatchItemStatus::Completed,
            None,
            Some(3000),
        )
        .unwrap();
    println!("  Completed large image in 3000ms");

    let eta = queue.estimate_remaining_ms(&job_id);
    println!("  ETA for 2 remaining (small + medium): {:?}ms", eta);
    // Should be ~500 + 1200 = 1700ms

    println!("\nETA accuracy improves with more data points.");
    println!("Estimates are bucketed by (model, operation, image_size).");

    // Verify ETA tracking
    assert_eq!(
        queue.eta_sample_count("llava:13b", "tag", SizeBucket::Small),
        1
    );
    assert_eq!(
        queue.eta_sample_count("llava:13b", "tag", SizeBucket::Medium),
        1
    );
    assert_eq!(
        queue.eta_sample_count("llava:13b", "tag", SizeBucket::Large),
        1
    );
}
