use crate::{config::QueueConfig, db, error::QueueError, events::*, JobContext, JobHandler};
use rusqlite::Connection;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::{AppHandle, Emitter};

/// The background job executor.
///
/// Polls the database for pending jobs and processes them using the
/// registered [`JobHandler`] implementation. Supports pause/resume,
/// consecutive job limits with cooldown, and cancellation.
pub struct QueueExecutor {
    config: QueueConfig,
    pub(crate) db: Arc<Mutex<Connection>>,
    paused: Arc<AtomicBool>,
}

impl QueueExecutor {
    pub fn new(config: QueueConfig, db: Arc<Mutex<Connection>>) -> Self {
        Self {
            config,
            db,
            paused: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Spawn the executor loop as a background tokio task.
    ///
    /// The executor will poll for pending jobs at the configured interval
    /// and process them using the provided `JobHandler` implementation.
    pub fn spawn<H>(self: Arc<Self>, app_handle: AppHandle)
    where
        H: JobHandler + 'static,
    {
        tauri::async_runtime::spawn(async move {
            self.run_loop::<H>(app_handle).await;
        });
    }

    async fn run_loop<H>(&self, app_handle: AppHandle)
    where
        H: JobHandler,
    {
        let mut consecutive_count: u32 = 0;

        loop {
            tokio::time::sleep(self.config.poll_interval).await;

            // Check if paused
            if self.paused.load(Ordering::Relaxed) {
                continue;
            }

            // Check consecutive limit
            if self.config.max_consecutive > 0 && consecutive_count >= self.config.max_consecutive {
                eprintln!(
                    "[tauri-queue] Consecutive limit ({}) reached, cooling down for {:?}",
                    self.config.max_consecutive, self.config.cooldown
                );
                tokio::time::sleep(self.config.cooldown).await;
                consecutive_count = 0;
                continue;
            }

            // Get next pending job
            let (job_id, job_data) = {
                let conn = match self.db.lock() {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("[tauri-queue] DB mutex poisoned: {}", e);
                        continue;
                    }
                };
                match db::get_next_pending(&conn) {
                    Ok(Some(job)) => job,
                    Ok(None) => {
                        consecutive_count = 0;
                        continue;
                    }
                    Err(e) => {
                        eprintln!("[tauri-queue] Failed to query next pending job: {:#}", e);
                        continue;
                    }
                }
            };

            // Deserialize job data into the handler type
            let job_handler: H = match serde_json::from_value(job_data) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("[tauri-queue] Failed to deserialize job {}: {}", job_id, e);
                    if let Ok(conn) = self.db.lock() {
                        let _ = db::mark_failed(
                            &conn,
                            &job_id,
                            &format!("Deserialization failed: {}", e),
                        );
                    }
                    let _ = app_handle.emit(
                        "queue:job_failed",
                        JobFailedEvent {
                            job_id: job_id.clone(),
                            error: format!("Deserialization failed: {}", e),
                        },
                    );
                    continue;
                }
            };

            // Process the job
            let result = self
                .process_job::<H>(&app_handle, &job_id, job_handler)
                .await;

            match result {
                Ok(_) => {
                    consecutive_count += 1;
                    if self.config.cooldown.as_secs() > 0 {
                        tokio::time::sleep(self.config.cooldown).await;
                    }
                }
                Err(e) => {
                    // Check if this was a cancellation
                    let was_cancelled = {
                        match self.db.lock() {
                            Ok(conn) => db::is_cancelled(&conn, &job_id).unwrap_or(false),
                            Err(_) => false,
                        }
                    };

                    if was_cancelled {
                        eprintln!("[tauri-queue] Job {} was cancelled", job_id);
                        let _ = app_handle.emit(
                            "queue:job_cancelled",
                            JobCancelledEvent {
                                job_id: job_id.clone(),
                            },
                        );
                    } else {
                        eprintln!("[tauri-queue] Job {} failed: {:#}", job_id, e);
                        if let Ok(conn) = self.db.lock() {
                            let _ = db::mark_failed(&conn, &job_id, &e.to_string());
                        }
                        let _ = app_handle.emit(
                            "queue:job_failed",
                            JobFailedEvent {
                                job_id: job_id.clone(),
                                error: e.to_string(),
                            },
                        );
                    }
                }
            }
        }
    }

    async fn process_job<H>(
        &self,
        app_handle: &AppHandle,
        job_id: &str,
        job_handler: H,
    ) -> Result<(), QueueError>
    where
        H: JobHandler,
    {
        // Mark as processing
        {
            let conn = self
                .db
                .lock()
                .map_err(|e| QueueError::Other(e.to_string()))?;
            db::mark_processing(&conn, job_id).map_err(|e| QueueError::Other(e.to_string()))?;
        }

        let _ = app_handle.emit(
            "queue:job_started",
            JobStartedEvent {
                job_id: job_id.to_string(),
            },
        );

        // Create job context with DB reference for cancellation checks
        let ctx = JobContext {
            job_id: job_id.to_string(),
            app_handle: app_handle.clone(),
            db: Arc::clone(&self.db),
        };

        // Execute job
        let result = job_handler.execute(&ctx).await;

        match result {
            Ok(job_result) => {
                if job_result.success {
                    let conn = self
                        .db
                        .lock()
                        .map_err(|e| QueueError::Other(e.to_string()))?;
                    db::mark_completed(&conn, job_id)
                        .map_err(|e| QueueError::Other(e.to_string()))?;

                    let _ = app_handle.emit(
                        "queue:job_completed",
                        JobCompletedEvent {
                            job_id: job_id.to_string(),
                            output: job_result.output,
                        },
                    );
                } else {
                    let error = job_result
                        .error
                        .unwrap_or_else(|| "Unknown error".to_string());
                    let conn = self
                        .db
                        .lock()
                        .map_err(|e| QueueError::Other(e.to_string()))?;
                    db::mark_failed(&conn, job_id, &error)
                        .map_err(|e| QueueError::Other(e.to_string()))?;

                    let _ = app_handle.emit(
                        "queue:job_failed",
                        JobFailedEvent {
                            job_id: job_id.to_string(),
                            error,
                        },
                    );
                }
                Ok(())
            }
            Err(e) => {
                let conn = self
                    .db
                    .lock()
                    .map_err(|e| QueueError::Other(e.to_string()))?;
                db::mark_failed(&conn, job_id, &e.to_string())
                    .map_err(|e2| QueueError::Other(e2.to_string()))?;
                Err(e)
            }
        }
    }

    /// Pause the executor. The current job (if any) will finish,
    /// but no new jobs will be started until [`resume()`](Self::resume) is called.
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Relaxed);
    }

    /// Resume the executor after a pause.
    pub fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
    }

    /// Check if the executor is currently paused.
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }
}
