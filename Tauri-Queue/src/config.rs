use std::path::PathBuf;
use std::time::Duration;

/// Configuration for the queue system.
///
/// Use [`QueueConfig::builder()`] for ergonomic construction, or
/// [`QueueConfig::default()`] for sensible defaults (in-memory DB, no cooldown).
#[derive(Debug, Clone)]
pub struct QueueConfig {
    /// Path to SQLite database file. `None` = in-memory database.
    pub db_path: Option<PathBuf>,

    /// Cooldown duration between job executions (0 = no cooldown).
    pub cooldown: Duration,

    /// Maximum consecutive jobs before a forced cooldown (0 = unlimited).
    pub max_consecutive: u32,

    /// Polling interval for checking pending jobs.
    pub poll_interval: Duration,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            db_path: None,
            cooldown: Duration::from_secs(0),
            max_consecutive: 0,
            poll_interval: Duration::from_secs(3),
        }
    }
}

impl QueueConfig {
    /// Start building a config with the builder pattern.
    pub fn builder() -> QueueConfigBuilder {
        QueueConfigBuilder::default()
    }
}

/// Builder for [`QueueConfig`].
#[derive(Default)]
pub struct QueueConfigBuilder {
    config: QueueConfig,
}

impl QueueConfigBuilder {
    /// Set the SQLite database path for persistence. Omit for in-memory.
    pub fn with_db_path(mut self, path: PathBuf) -> Self {
        self.config.db_path = Some(path);
        self
    }

    /// Set the cooldown duration between consecutive job executions.
    pub fn with_cooldown(mut self, duration: Duration) -> Self {
        self.config.cooldown = duration;
        self
    }

    /// Set the maximum consecutive jobs before a forced cooldown.
    pub fn with_max_consecutive(mut self, max: u32) -> Self {
        self.config.max_consecutive = max;
        self
    }

    /// Set the polling interval for checking pending jobs.
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.config.poll_interval = interval;
        self
    }

    /// Build the final [`QueueConfig`].
    pub fn build(self) -> QueueConfig {
        self.config
    }
}
