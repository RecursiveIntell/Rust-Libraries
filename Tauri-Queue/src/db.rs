use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde_json::Value;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS queue_jobs (
    id              TEXT PRIMARY KEY,
    priority        INTEGER DEFAULT 2,
    status          TEXT CHECK(status IN ('pending', 'processing', 'completed', 'failed', 'cancelled')),
    data_json       TEXT NOT NULL,
    created_at      DATETIME DEFAULT CURRENT_TIMESTAMP,
    started_at      DATETIME,
    completed_at    DATETIME,
    error_message   TEXT
);

CREATE INDEX IF NOT EXISTS idx_queue_status_priority ON queue_jobs(status, priority);
"#;

/// Open (or create) the queue database. Pass `None` for an in-memory database.
pub fn open_database(path: Option<&std::path::Path>) -> Result<Connection> {
    let conn = match path {
        Some(p) => Connection::open(p).context("Failed to open queue database")?,
        None => Connection::open_in_memory().context("Failed to open in-memory database")?,
    };

    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )
    .context("Failed to set PRAGMA options")?;

    conn.execute_batch(SCHEMA)
        .context("Failed to create queue schema")?;

    Ok(conn)
}

/// Insert a new job into the queue.
pub fn insert_job(conn: &Connection, job_id: &str, priority: i32, data: &Value) -> Result<()> {
    conn.execute(
        "INSERT INTO queue_jobs (id, priority, status, data_json)
         VALUES (?1, ?2, 'pending', ?3)",
        params![job_id, priority, serde_json::to_string(data)?],
    )
    .context("Failed to insert queue job")?;
    Ok(())
}

/// Get the next pending job (highest priority, oldest first).
/// Returns the job ID and its data as a JSON value.
pub fn get_next_pending(conn: &Connection) -> Result<Option<(String, Value)>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, data_json FROM queue_jobs
             WHERE status = 'pending'
             ORDER BY priority ASC, created_at ASC
             LIMIT 1",
        )
        .context("Failed to prepare get_next_pending query")?;

    let mut rows = stmt.query([]).context("Failed to query next pending job")?;

    if let Some(row) = rows.next().context("Failed to read next pending row")? {
        let id: String = row.get(0)?;
        let data_json: String = row.get(1)?;
        let data: Value =
            serde_json::from_str(&data_json).context("Failed to parse job data JSON")?;
        Ok(Some((id, data)))
    } else {
        Ok(None)
    }
}

/// Mark a job as processing and set started_at.
pub fn mark_processing(conn: &Connection, job_id: &str) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE queue_jobs SET status = 'processing', started_at = ?1 WHERE id = ?2",
        params![now, job_id],
    )
    .context("Failed to mark job as processing")?;
    Ok(())
}

/// Mark a job as completed and set completed_at.
pub fn mark_completed(conn: &Connection, job_id: &str) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE queue_jobs SET status = 'completed', completed_at = ?1 WHERE id = ?2",
        params![now, job_id],
    )
    .context("Failed to mark job as completed")?;
    Ok(())
}

/// Mark a job as failed with an error message and set completed_at.
pub fn mark_failed(conn: &Connection, job_id: &str, error: &str) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE queue_jobs SET status = 'failed', completed_at = ?1, error_message = ?2 WHERE id = ?3",
        params![now, error, job_id],
    )
    .context("Failed to mark job as failed")?;
    Ok(())
}

/// Check if a job has been cancelled (used by executor during execution).
pub fn is_cancelled(conn: &Connection, job_id: &str) -> Result<bool> {
    let status: String = conn
        .query_row(
            "SELECT status FROM queue_jobs WHERE id = ?1",
            params![job_id],
            |row| row.get(0),
        )
        .map_err(|_| anyhow::anyhow!("Job '{}' not found", job_id))?;
    Ok(status == "cancelled")
}

/// Cancel a pending or processing job. Returns the previous status.
pub fn cancel_job(conn: &Connection, job_id: &str) -> Result<String> {
    let prev_status: String = conn
        .query_row(
            "SELECT status FROM queue_jobs WHERE id = ?1",
            params![job_id],
            |row| row.get(0),
        )
        .map_err(|_| anyhow::anyhow!("Job '{}' not found", job_id))?;

    if prev_status != "pending" && prev_status != "processing" {
        anyhow::bail!(
            "Job '{}' is not cancellable (status: {})",
            job_id,
            prev_status
        );
    }

    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE queue_jobs SET status = 'cancelled', completed_at = ?1
         WHERE id = ?2 AND status IN ('pending', 'processing')",
        params![now, job_id],
    )
    .context("Failed to cancel job")?;

    Ok(prev_status)
}

/// Re-queue any jobs that were mid-processing when the app crashed.
/// Returns the number of jobs requeued.
pub fn requeue_interrupted(conn: &Connection) -> Result<u32> {
    let count = conn
        .execute(
            "UPDATE queue_jobs SET status = 'pending' WHERE status = 'processing'",
            [],
        )
        .context("Failed to requeue interrupted jobs")?;
    Ok(count as u32)
}

/// Update the priority of a job.
pub fn update_priority(conn: &Connection, job_id: &str, priority: i32) -> Result<()> {
    conn.execute(
        "UPDATE queue_jobs SET priority = ?1 WHERE id = ?2",
        params![priority, job_id],
    )
    .context("Failed to update job priority")?;
    Ok(())
}

/// List all jobs ordered by status then priority then creation time.
/// Returns tuples of (id, status, data_json).
pub fn list_all_jobs(conn: &Connection) -> Result<Vec<(String, String, String)>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, status, data_json FROM queue_jobs
             ORDER BY
                CASE status
                    WHEN 'processing' THEN 0
                    WHEN 'pending' THEN 1
                    WHEN 'completed' THEN 2
                    WHEN 'failed' THEN 3
                    WHEN 'cancelled' THEN 4
                END,
                priority ASC,
                created_at ASC",
        )
        .context("Failed to prepare list_all_jobs query")?;

    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .context("Failed to execute list_all_jobs query")?;

    let mut jobs = Vec::new();
    for row in rows {
        jobs.push(row.context("Failed to read job row")?);
    }
    Ok(jobs)
}

/// Delete completed/failed/cancelled jobs older than the specified number of days.
/// Returns the number of jobs deleted.
pub fn prune_old_jobs(conn: &Connection, days: u32) -> Result<u32> {
    let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
    let cutoff_str = cutoff.to_rfc3339();

    let count = conn
        .execute(
            "DELETE FROM queue_jobs
             WHERE status IN ('completed', 'failed', 'cancelled')
             AND completed_at < ?1",
            params![cutoff_str],
        )
        .context("Failed to prune old queue jobs")?;

    Ok(count as u32)
}

/// Row data for a single job.
pub type JobRow = (String, i32, String, String, Option<String>);

/// Get a single job by ID. Returns (id, priority, status, data_json, error_message).
pub fn get_job(conn: &Connection, job_id: &str) -> Result<Option<JobRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, priority, status, data_json, error_message
             FROM queue_jobs WHERE id = ?1",
        )
        .context("Failed to prepare get_job query")?;

    let mut rows = stmt.query(params![job_id])?;

    if let Some(row) = rows.next()? {
        Ok(Some((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
        )))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        open_database(None).unwrap()
    }

    #[test]
    fn test_open_in_memory() {
        let conn = open_database(None);
        assert!(conn.is_ok());
    }

    #[test]
    fn test_insert_and_get_next_pending() {
        let conn = setup();
        let data = serde_json::json!({"task": "send email"});
        insert_job(&conn, "job-1", 2, &data).unwrap();

        let next = get_next_pending(&conn).unwrap();
        assert!(next.is_some());
        let (id, val) = next.unwrap();
        assert_eq!(id, "job-1");
        assert_eq!(val["task"], "send email");
    }

    #[test]
    fn test_priority_ordering() {
        let conn = setup();
        insert_job(&conn, "low-1", 3, &serde_json::json!({"p": "low"})).unwrap();
        insert_job(&conn, "high-1", 1, &serde_json::json!({"p": "high"})).unwrap();
        insert_job(&conn, "normal-1", 2, &serde_json::json!({"p": "normal"})).unwrap();

        let next = get_next_pending(&conn).unwrap().unwrap();
        assert_eq!(next.0, "high-1");
    }

    #[test]
    fn test_mark_processing() {
        let conn = setup();
        insert_job(&conn, "job-1", 2, &serde_json::json!({})).unwrap();
        mark_processing(&conn, "job-1").unwrap();

        let job = get_job(&conn, "job-1").unwrap().unwrap();
        assert_eq!(job.2, "processing");

        // No more pending jobs
        assert!(get_next_pending(&conn).unwrap().is_none());
    }

    #[test]
    fn test_mark_completed() {
        let conn = setup();
        insert_job(&conn, "job-1", 2, &serde_json::json!({})).unwrap();
        mark_processing(&conn, "job-1").unwrap();
        mark_completed(&conn, "job-1").unwrap();

        let job = get_job(&conn, "job-1").unwrap().unwrap();
        assert_eq!(job.2, "completed");
    }

    #[test]
    fn test_mark_failed() {
        let conn = setup();
        insert_job(&conn, "job-1", 2, &serde_json::json!({})).unwrap();
        mark_processing(&conn, "job-1").unwrap();
        mark_failed(&conn, "job-1", "something broke").unwrap();

        let job = get_job(&conn, "job-1").unwrap().unwrap();
        assert_eq!(job.2, "failed");
        assert_eq!(job.4.as_deref(), Some("something broke"));
    }

    #[test]
    fn test_cancel_pending() {
        let conn = setup();
        insert_job(&conn, "job-1", 2, &serde_json::json!({})).unwrap();
        let prev = cancel_job(&conn, "job-1").unwrap();
        assert_eq!(prev, "pending");

        assert!(is_cancelled(&conn, "job-1").unwrap());
    }

    #[test]
    fn test_cancel_processing() {
        let conn = setup();
        insert_job(&conn, "job-1", 2, &serde_json::json!({})).unwrap();
        mark_processing(&conn, "job-1").unwrap();
        let prev = cancel_job(&conn, "job-1").unwrap();
        assert_eq!(prev, "processing");

        assert!(is_cancelled(&conn, "job-1").unwrap());
    }

    #[test]
    fn test_cancel_completed_fails() {
        let conn = setup();
        insert_job(&conn, "job-1", 2, &serde_json::json!({})).unwrap();
        mark_processing(&conn, "job-1").unwrap();
        mark_completed(&conn, "job-1").unwrap();

        let result = cancel_job(&conn, "job-1");
        assert!(result.is_err());
    }

    #[test]
    fn test_requeue_interrupted() {
        let conn = setup();
        insert_job(&conn, "job-1", 2, &serde_json::json!({})).unwrap();
        mark_processing(&conn, "job-1").unwrap();

        let count = requeue_interrupted(&conn).unwrap();
        assert_eq!(count, 1);

        let next = get_next_pending(&conn).unwrap();
        assert!(next.is_some());
        assert_eq!(next.unwrap().0, "job-1");
    }

    #[test]
    fn test_update_priority() {
        let conn = setup();
        insert_job(&conn, "job-1", 3, &serde_json::json!({})).unwrap();
        update_priority(&conn, "job-1", 1).unwrap();

        let job = get_job(&conn, "job-1").unwrap().unwrap();
        assert_eq!(job.1, 1);
    }

    #[test]
    fn test_list_all_jobs() {
        let conn = setup();
        insert_job(&conn, "a", 2, &serde_json::json!({"n": 1})).unwrap();
        insert_job(&conn, "b", 1, &serde_json::json!({"n": 2})).unwrap();
        insert_job(&conn, "c", 3, &serde_json::json!({"n": 3})).unwrap();

        let jobs = list_all_jobs(&conn).unwrap();
        assert_eq!(jobs.len(), 3);
        // All pending, so ordered by priority: b(1), a(2), c(3)
        assert_eq!(jobs[0].0, "b");
        assert_eq!(jobs[1].0, "a");
        assert_eq!(jobs[2].0, "c");
    }

    #[test]
    fn test_prune_old_jobs() {
        let conn = setup();
        insert_job(&conn, "job-1", 2, &serde_json::json!({})).unwrap();
        mark_processing(&conn, "job-1").unwrap();
        mark_completed(&conn, "job-1").unwrap();

        // Job completed just now — pruning with 30 days should NOT remove it
        let count = prune_old_jobs(&conn, 30).unwrap();
        assert_eq!(count, 0);

        // Set completed_at to 10 days ago manually
        let old_date = (chrono::Utc::now() - chrono::Duration::days(10)).to_rfc3339();
        conn.execute(
            "UPDATE queue_jobs SET completed_at = ?1 WHERE id = 'job-1'",
            params![old_date],
        )
        .unwrap();

        // Pruning with 5 days should remove it (10 days old > 5 day cutoff)
        let count = prune_old_jobs(&conn, 5).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_get_job_not_found() {
        let conn = setup();
        let result = get_job(&conn, "nonexistent").unwrap();
        assert!(result.is_none());
    }
}
