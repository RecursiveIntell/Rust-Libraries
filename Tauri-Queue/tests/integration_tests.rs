mod test_helpers;

use tauri_queue::*;
use tempfile::tempdir;
use test_helpers::TestJob;

#[test]
fn test_queue_creation_in_memory() {
    let config = QueueConfig::default();
    let queue = QueueManager::new(config);
    assert!(queue.is_ok());
}

#[test]
fn test_queue_creation_with_db() {
    let temp = tempdir().unwrap();
    let config = QueueConfig::builder()
        .with_db_path(temp.path().join("test.db"))
        .build();
    let queue = QueueManager::new(config);
    assert!(queue.is_ok());
}

#[test]
fn test_add_job() {
    let config = QueueConfig::default();
    let queue = QueueManager::new(config).unwrap();

    let job = QueueJob::new(TestJob {
        data: "test".to_string(),
    });

    let job_id = queue.add(job).unwrap();
    assert!(!job_id.is_empty());
}

#[test]
fn test_add_job_with_custom_id() {
    let config = QueueConfig::default();
    let queue = QueueManager::new(config).unwrap();

    let job = QueueJob::new(TestJob {
        data: "test".to_string(),
    })
    .with_id("my-custom-id".to_string());

    let job_id = queue.add(job).unwrap();
    assert_eq!(job_id, "my-custom-id");
}

#[test]
fn test_priority_ordering() {
    let config = QueueConfig::default();
    let queue = QueueManager::new(config).unwrap();

    let low = QueueJob::new(TestJob { data: "low".into() })
        .with_priority(QueuePriority::Low)
        .with_id("low".into());

    let high = QueueJob::new(TestJob {
        data: "high".into(),
    })
    .with_priority(QueuePriority::High)
    .with_id("high".into());

    let normal = QueueJob::new(TestJob {
        data: "normal".into(),
    })
    .with_priority(QueuePriority::Normal)
    .with_id("normal".into());

    queue.add(low).unwrap();
    queue.add(high).unwrap();
    queue.add(normal).unwrap();

    let jobs = queue.list_jobs().unwrap();
    assert_eq!(jobs.len(), 3);
    // All pending, sorted by priority: high(1), normal(2), low(3)
    assert_eq!(jobs[0].0, "high");
    assert_eq!(jobs[1].0, "normal");
    assert_eq!(jobs[2].0, "low");
}

#[test]
fn test_cancel_pending_job() {
    let config = QueueConfig::default();
    let queue = QueueManager::new(config).unwrap();

    let job = QueueJob::new(TestJob {
        data: "test".into(),
    })
    .with_id("cancel-me".into());

    queue.add(job).unwrap();
    queue.cancel("cancel-me").unwrap();

    let jobs = queue.list_jobs().unwrap();
    assert_eq!(jobs[0].1, "cancelled");
}

#[test]
fn test_cancel_nonexistent_fails() {
    let config = QueueConfig::default();
    let queue = QueueManager::new(config).unwrap();

    let result = queue.cancel("nonexistent");
    assert!(result.is_err());
}

#[test]
fn test_reorder_pending_job() {
    let config = QueueConfig::default();
    let queue = QueueManager::new(config).unwrap();

    let job = QueueJob::new(TestJob {
        data: "test".into(),
    })
    .with_priority(QueuePriority::Low)
    .with_id("reorder-me".into());

    queue.add(job).unwrap();
    queue.reorder("reorder-me", QueuePriority::High).unwrap();

    // Verify by listing — job should be present
    let jobs = queue.list_jobs().unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].0, "reorder-me");
}

#[test]
fn test_reorder_nonexistent_fails() {
    let config = QueueConfig::default();
    let queue = QueueManager::new(config).unwrap();

    let result = queue.reorder("nonexistent", QueuePriority::High);
    assert!(result.is_err());
}

#[test]
fn test_pause_resume() {
    let config = QueueConfig::default();
    let queue = QueueManager::new(config).unwrap();

    assert!(!queue.is_paused());

    queue.pause();
    assert!(queue.is_paused());

    queue.resume();
    assert!(!queue.is_paused());
}

#[test]
fn test_list_jobs_empty() {
    let config = QueueConfig::default();
    let queue = QueueManager::new(config).unwrap();

    let jobs = queue.list_jobs().unwrap();
    assert!(jobs.is_empty());
}

#[test]
fn test_list_jobs_with_data() {
    let config = QueueConfig::default();
    let queue = QueueManager::new(config).unwrap();

    let job = QueueJob::new(TestJob {
        data: "hello world".into(),
    })
    .with_id("data-job".into());

    queue.add(job).unwrap();

    let jobs = queue.list_jobs_with_data().unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].0, "data-job");
    assert_eq!(jobs[0].1, "pending");
    assert!(jobs[0].2.contains("hello world"));
}

#[test]
fn test_persistence_across_instances() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("persist.db");

    // First instance: add jobs
    {
        let config = QueueConfig::builder().with_db_path(db_path.clone()).build();
        let queue = QueueManager::new(config).unwrap();

        queue
            .add(
                QueueJob::new(TestJob {
                    data: "persistent".into(),
                })
                .with_id("p1".into()),
            )
            .unwrap();
        queue
            .add(
                QueueJob::new(TestJob {
                    data: "persistent2".into(),
                })
                .with_id("p2".into()),
            )
            .unwrap();
    }

    // Second instance: jobs should still be there
    {
        let config = QueueConfig::builder().with_db_path(db_path).build();
        let queue = QueueManager::new(config).unwrap();

        let jobs = queue.list_jobs().unwrap();
        assert_eq!(jobs.len(), 2);
    }
}

#[test]
fn test_crash_recovery_requeues_processing() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("crash.db");

    // First instance: add a job and mark it processing via raw DB
    {
        let conn = tauri_queue::db::open_database(Some(&db_path)).unwrap();
        tauri_queue::db::insert_job(
            &conn,
            "crashed-job",
            2,
            &serde_json::json!({"data": "was processing"}),
        )
        .unwrap();
        tauri_queue::db::mark_processing(&conn, "crashed-job").unwrap();
    }

    // Second instance: QueueManager should requeue the processing job
    {
        let config = QueueConfig::builder().with_db_path(db_path).build();
        let queue = QueueManager::new(config).unwrap();

        let jobs = queue.list_jobs().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].0, "crashed-job");
        assert_eq!(jobs[0].1, "pending"); // Requeued from processing
    }
}

#[test]
fn test_prune_no_completed_jobs() {
    let config = QueueConfig::default();
    let queue = QueueManager::new(config).unwrap();

    queue
        .add(QueueJob::new(TestJob {
            data: "pending".into(),
        }))
        .unwrap();

    let pruned = queue.prune(0).unwrap();
    assert_eq!(pruned, 0);
}

#[test]
fn test_multiple_priorities_interleaved() {
    let config = QueueConfig::default();
    let queue = QueueManager::new(config).unwrap();

    for i in 0..3 {
        queue
            .add(
                QueueJob::new(TestJob {
                    data: format!("high-{}", i),
                })
                .with_priority(QueuePriority::High)
                .with_id(format!("h-{}", i)),
            )
            .unwrap();
        queue
            .add(
                QueueJob::new(TestJob {
                    data: format!("low-{}", i),
                })
                .with_priority(QueuePriority::Low)
                .with_id(format!("l-{}", i)),
            )
            .unwrap();
    }

    let jobs = queue.list_jobs().unwrap();
    assert_eq!(jobs.len(), 6);

    // First 3 should be high priority
    for i in 0..3 {
        assert!(
            jobs[i].0.starts_with("h-"),
            "Expected high priority at index {}",
            i
        );
    }
    // Last 3 should be low priority
    for i in 3..6 {
        assert!(
            jobs[i].0.starts_with("l-"),
            "Expected low priority at index {}",
            i
        );
    }
}

// -- Type tests --

#[test]
fn test_queue_priority_roundtrip() {
    assert_eq!(
        QueuePriority::from_i32(QueuePriority::High.as_i32()),
        QueuePriority::High
    );
    assert_eq!(
        QueuePriority::from_i32(QueuePriority::Normal.as_i32()),
        QueuePriority::Normal
    );
    assert_eq!(
        QueuePriority::from_i32(QueuePriority::Low.as_i32()),
        QueuePriority::Low
    );
    // Unknown value defaults to Low
    assert_eq!(QueuePriority::from_i32(99), QueuePriority::Low);
}

#[test]
fn test_job_status_roundtrip() {
    let statuses = [
        QueueJobStatus::Pending,
        QueueJobStatus::Processing,
        QueueJobStatus::Completed,
        QueueJobStatus::Failed,
        QueueJobStatus::Cancelled,
    ];

    for status in &statuses {
        let s = status.as_str();
        let parsed = QueueJobStatus::parse(s);
        assert_eq!(parsed.as_ref(), Some(status));
    }

    assert_eq!(QueueJobStatus::parse("unknown"), None);
}

#[test]
fn test_job_result_variants() {
    let success = JobResult::success();
    assert!(success.success);
    assert!(success.output.is_none());
    assert!(success.error.is_none());

    let with_output = JobResult::success_with_output("done".into());
    assert!(with_output.success);
    assert_eq!(with_output.output.as_deref(), Some("done"));

    let failure = JobResult::failure("oops".into());
    assert!(!failure.success);
    assert_eq!(failure.error.as_deref(), Some("oops"));
}

#[test]
fn test_event_serialization() {
    use tauri_queue::events::*;

    let started = JobStartedEvent {
        job_id: "j1".to_string(),
    };
    let json = serde_json::to_string(&started).unwrap();
    assert!(json.contains("jobId"));

    let completed = JobCompletedEvent {
        job_id: "j1".to_string(),
        output: Some("result".to_string()),
    };
    let json = serde_json::to_string(&completed).unwrap();
    assert!(json.contains("jobId"));
    assert!(json.contains("result"));

    let failed = JobFailedEvent {
        job_id: "j1".to_string(),
        error: "something broke".to_string(),
    };
    let json = serde_json::to_string(&failed).unwrap();
    assert!(json.contains("something broke"));

    let progress = JobProgressEvent {
        job_id: "j1".to_string(),
        current_step: 5,
        total_steps: 10,
        progress: 0.5,
    };
    let json = serde_json::to_string(&progress).unwrap();
    assert!(json.contains("currentStep"));
    assert!(json.contains("totalSteps"));

    let cancelled = JobCancelledEvent {
        job_id: "j1".to_string(),
    };
    let json = serde_json::to_string(&cancelled).unwrap();
    assert!(json.contains("jobId"));
}

#[test]
fn test_config_builder() {
    use std::time::Duration;

    let config = QueueConfig::builder()
        .with_cooldown(Duration::from_secs(5))
        .with_max_consecutive(10)
        .with_poll_interval(Duration::from_secs(1))
        .build();

    assert_eq!(config.cooldown, Duration::from_secs(5));
    assert_eq!(config.max_consecutive, 10);
    assert_eq!(config.poll_interval, Duration::from_secs(1));
    assert!(config.db_path.is_none());
}

#[test]
fn test_config_defaults() {
    let config = QueueConfig::default();
    assert!(config.db_path.is_none());
    assert_eq!(config.cooldown, std::time::Duration::from_secs(0));
    assert_eq!(config.max_consecutive, 0);
    assert_eq!(config.poll_interval, std::time::Duration::from_secs(3));
}
