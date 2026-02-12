use std::collections::HashMap;
use std::sync::Mutex;

use crate::types::SizeBucket;

/// Cache key for ETA estimation, combining resource + operation + size.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct EtaKey {
    pub resource_key: String,
    pub operation: String,
    pub size_bucket: SizeBucket,
}

#[derive(Debug, Clone)]
struct EtaStats {
    total_ms: u64,
    count: u64,
}

impl EtaStats {
    fn avg_ms(&self) -> u64 {
        if self.count == 0 {
            0
        } else {
            self.total_ms / self.count
        }
    }
}

/// Tracks processing durations bucketed by (resource, operation, size)
/// to provide increasingly accurate ETA estimates.
pub struct EtaTracker {
    data: Mutex<HashMap<EtaKey, EtaStats>>,
}

impl Default for EtaTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl EtaTracker {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
        }
    }

    /// Record a completed item's duration for future ETA estimates.
    pub fn record(
        &self,
        resource_key: &str,
        operation: &str,
        size_bucket: SizeBucket,
        duration_ms: u64,
    ) {
        let key = EtaKey {
            resource_key: resource_key.to_string(),
            operation: operation.to_string(),
            size_bucket,
        };

        match self.data.lock() {
            Ok(mut data) => {
                let entry = data.entry(key).or_insert(EtaStats {
                    total_ms: 0,
                    count: 0,
                });
                entry.total_ms += duration_ms;
                entry.count += 1;
            }
            Err(e) => {
                eprintln!("[ai-batch-queue] WARNING: ETA stats mutex poisoned: {}", e);
            }
        }
    }

    /// Estimate processing time for a single item based on historical data.
    /// Returns `None` if no data is available for this combination.
    pub fn estimate_one(
        &self,
        resource_key: &str,
        operation: &str,
        size_bucket: SizeBucket,
    ) -> Option<u64> {
        let data = self.data.lock().ok()?;

        // Try exact match first
        let key = EtaKey {
            resource_key: resource_key.to_string(),
            operation: operation.to_string(),
            size_bucket,
        };
        if let Some(stats) = data.get(&key) {
            return Some(stats.avg_ms());
        }

        // Fall back to Unknown bucket for this resource+operation
        let fallback = EtaKey {
            resource_key: resource_key.to_string(),
            operation: operation.to_string(),
            size_bucket: SizeBucket::Unknown,
        };
        data.get(&fallback).map(|stats| stats.avg_ms())
    }

    /// Estimate total remaining time for a set of items.
    pub fn estimate_remaining(
        &self,
        resource_key: &str,
        operation: &str,
        remaining_buckets: &[SizeBucket],
    ) -> Option<u64> {
        let data = self.data.lock().ok()?;

        let mut total_estimate: u64 = 0;
        let mut has_data = false;

        for &bucket in remaining_buckets {
            let key = EtaKey {
                resource_key: resource_key.to_string(),
                operation: operation.to_string(),
                size_bucket: bucket,
            };

            if let Some(stats) = data.get(&key) {
                total_estimate += stats.avg_ms();
                has_data = true;
            } else {
                let fallback = EtaKey {
                    resource_key: resource_key.to_string(),
                    operation: operation.to_string(),
                    size_bucket: SizeBucket::Unknown,
                };
                if let Some(stats) = data.get(&fallback) {
                    total_estimate += stats.avg_ms();
                    has_data = true;
                }
            }
        }

        if has_data {
            Some(total_estimate)
        } else {
            None
        }
    }

    /// Get the number of data points recorded for a specific key.
    pub fn sample_count(
        &self,
        resource_key: &str,
        operation: &str,
        size_bucket: SizeBucket,
    ) -> u64 {
        let key = EtaKey {
            resource_key: resource_key.to_string(),
            operation: operation.to_string(),
            size_bucket,
        };
        self.data
            .lock()
            .ok()
            .and_then(|d| d.get(&key).map(|s| s.count))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_estimate() {
        let tracker = EtaTracker::new();

        tracker.record("model-a", "tag", SizeBucket::Medium, 1000);
        tracker.record("model-a", "tag", SizeBucket::Medium, 2000);

        let estimate = tracker.estimate_one("model-a", "tag", SizeBucket::Medium);
        assert_eq!(estimate, Some(1500)); // (1000 + 2000) / 2
    }

    #[test]
    fn test_no_data_returns_none() {
        let tracker = EtaTracker::new();
        let estimate = tracker.estimate_one("model-a", "tag", SizeBucket::Medium);
        assert_eq!(estimate, None);
    }

    #[test]
    fn test_fallback_to_unknown_bucket() {
        let tracker = EtaTracker::new();

        // Only record Unknown bucket
        tracker.record("model-a", "tag", SizeBucket::Unknown, 500);

        // Should fall back from Medium -> Unknown
        let estimate = tracker.estimate_one("model-a", "tag", SizeBucket::Medium);
        assert_eq!(estimate, Some(500));
    }

    #[test]
    fn test_estimate_remaining_multiple() {
        let tracker = EtaTracker::new();

        tracker.record("model-a", "tag", SizeBucket::Small, 500);
        tracker.record("model-a", "tag", SizeBucket::Large, 2000);

        let remaining = vec![SizeBucket::Small, SizeBucket::Small, SizeBucket::Large];
        let estimate = tracker.estimate_remaining("model-a", "tag", &remaining);
        // 500 + 500 + 2000 = 3000
        assert_eq!(estimate, Some(3000));
    }

    #[test]
    fn test_sample_count() {
        let tracker = EtaTracker::new();
        assert_eq!(tracker.sample_count("m", "op", SizeBucket::Small), 0);

        tracker.record("m", "op", SizeBucket::Small, 100);
        tracker.record("m", "op", SizeBucket::Small, 200);
        assert_eq!(tracker.sample_count("m", "op", SizeBucket::Small), 2);
    }

    #[test]
    fn test_different_operations_isolated() {
        let tracker = EtaTracker::new();

        tracker.record("model", "tag", SizeBucket::Medium, 1000);
        tracker.record("model", "caption", SizeBucket::Medium, 3000);

        assert_eq!(
            tracker.estimate_one("model", "tag", SizeBucket::Medium),
            Some(1000)
        );
        assert_eq!(
            tracker.estimate_one("model", "caption", SizeBucket::Medium),
            Some(3000)
        );
    }
}
