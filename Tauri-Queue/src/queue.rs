use crate::{
    config::QueueConfig,
    db,
    error::QueueError,
    executor::QueueExecutor,
    types::{QueueJob, QueuePriority},
    JobHandler,
};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

/// High-level queue manager providing the public API.
///
/// Create a `QueueManager`, add jobs to it, then call [`spawn()`](Self::spawn)
/// to start the background executor that processes them.
///
/// # Example
///
/// ```ignore
/// let config = QueueConfig::builder()
///     .with_db_path(PathBuf::from("queue.db"))
///     .build();
///
/// let manager = QueueManager::new(config).unwrap();
/// let job = QueueJob::new(MyJob { ... });
/// manager.add(job).unwrap();
///
/// // In Tauri setup:
/// manager.spawn::<MyJob>(app.handle().clone());
/// ```
pub struct QueueManager {
    db: Arc<Mutex<Connection>>,
    executor: Arc<QueueExecutor>,
}

impl QueueManager {
    /// Create a new queue manager with the given configuration.
    ///
    /// Opens (or creates) the SQLite database and requeues any jobs that
    /// were interrupted by a previous crash.
    pub fn new(config: QueueConfig) -> Result<Self, QueueError> {
        let db_path = config.db_path.as_deref();
        let conn = db::open_database(db_path).map_err(|e| QueueError::Other(e.to_string()))?;

        // Requeue interrupted jobs from a previous crash
        let requeued =
            db::requeue_interrupted(&conn).map_err(|e| QueueError::Other(e.to_string()))?;
        if requeued > 0 {
            eprintln!("[tauri-queue] Requeued {} interrupted jobs", requeued);
        }

        let db = Arc::new(Mutex::new(conn));
        let executor = Arc::new(QueueExecutor::new(config, Arc::clone(&db)));

        Ok(Self { db, executor })
    }

    /// Add a job to the queue. Returns the job ID.
    pub fn add<H>(&self, job: QueueJob<H>) -> Result<String, QueueError>
    where
        H: JobHandler,
    {
        let conn = self
            .db
            .lock()
            .map_err(|e| QueueError::Other(e.to_string()))?;
        let data = serde_json::to_value(&job.data)?;
        db::insert_job(&conn, &job.id, job.priority.as_i32(), &data)
            .map_err(|e| QueueError::Other(e.to_string()))?;
        Ok(job.id)
    }

    /// Cancel a pending or processing job by ID.
    pub fn cancel(&self, job_id: &str) -> Result<(), QueueError> {
        let conn = self
            .db
            .lock()
            .map_err(|e| QueueError::Other(e.to_string()))?;
        db::cancel_job(&conn, job_id).map_err(|e| QueueError::Other(e.to_string()))?;
        Ok(())
    }

    /// Reorder a pending job to a new priority.
    pub fn reorder(&self, job_id: &str, new_priority: QueuePriority) -> Result<(), QueueError> {
        let conn = self
            .db
            .lock()
            .map_err(|e| QueueError::Other(e.to_string()))?;

        // Check job is pending
        if let Some(job) =
            db::get_job(&conn, job_id).map_err(|e| QueueError::Other(e.to_string()))?
        {
            if job.2 != "pending" {
                return Err(QueueError::Other(format!(
                    "Can only reorder pending jobs (job {} is {})",
                    job_id, job.2
                )));
            }
        } else {
            return Err(QueueError::NotFound(job_id.to_string()));
        }

        db::update_priority(&conn, job_id, new_priority.as_i32())
            .map_err(|e| QueueError::Other(e.to_string()))?;
        Ok(())
    }

    /// Pause the queue. The current job will finish, but no new jobs start.
    pub fn pause(&self) {
        self.executor.pause();
    }

    /// Resume the queue after a pause.
    pub fn resume(&self) {
        self.executor.resume();
    }

    /// Check if the queue is currently paused.
    pub fn is_paused(&self) -> bool {
        self.executor.is_paused()
    }

    /// Get all jobs as `(id, status)` pairs, ordered by status then priority.
    pub fn list_jobs(&self) -> Result<Vec<(String, String)>, QueueError> {
        let conn = self
            .db
            .lock()
            .map_err(|e| QueueError::Other(e.to_string()))?;
        let jobs = db::list_all_jobs(&conn).map_err(|e| QueueError::Other(e.to_string()))?;
        Ok(jobs
            .into_iter()
            .map(|(id, status, _)| (id, status))
            .collect())
    }

    /// Get all jobs as `(id, status, data_json)` tuples.
    pub fn list_jobs_with_data(&self) -> Result<Vec<(String, String, String)>, QueueError> {
        let conn = self
            .db
            .lock()
            .map_err(|e| QueueError::Other(e.to_string()))?;
        db::list_all_jobs(&conn).map_err(|e| QueueError::Other(e.to_string()))
    }

    /// Prune completed/failed/cancelled jobs older than `days`.
    /// Returns the number of jobs deleted.
    pub fn prune(&self, days: u32) -> Result<u32, QueueError> {
        let conn = self
            .db
            .lock()
            .map_err(|e| QueueError::Other(e.to_string()))?;
        db::prune_old_jobs(&conn, days).map_err(|e| QueueError::Other(e.to_string()))
    }

    /// Spawn the background executor and return the manager wrapped in an `Arc`.
    ///
    /// The returned `Arc<QueueManager>` can be stored in Tauri's managed state
    /// and shared across commands.
    pub fn spawn<H>(self, app_handle: tauri::AppHandle) -> Arc<Self>
    where
        H: JobHandler + 'static,
    {
        let manager = Arc::new(self);
        let executor = Arc::clone(&manager.executor);
        executor.spawn::<H>(app_handle);
        manager
    }
}
