use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Local rate limiter for failed sign-in attempts (Phase 2). State is
/// intentionally in-memory: a restart clears the lockout window, but failures
/// are still recorded to the audit log by the caller. One bucket per key.
pub struct LoginThrottle {
    buckets: Mutex<HashMap<String, Bucket>>,
}

struct Bucket {
    failures: Vec<Instant>,
}

const MAX_FAILURES: usize = 5;
const WINDOW: Duration = Duration::from_secs(600);
const LOCKOUT: Duration = Duration::from_secs(300);

impl LoginThrottle {
    pub fn new() -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
        }
    }

    fn bucket(&self, key: &str) -> Option<Bucket> {
        let now = Instant::now();
        let buckets = self.buckets.lock().unwrap();
        let existing = buckets.get(key)?;
        let failures: Vec<Instant> = existing
            .failures
            .iter()
            .copied()
            .filter(|t| now.duration_since(*t) < WINDOW)
            .collect();
        if failures.is_empty() {
            return None;
        }
        Some(Bucket { failures })
    }

    /// Seconds remaining in a lockout for this key, if currently blocked.
    pub fn remaining_lockout(&self, key: &str) -> Option<u64> {
        let bucket = self.bucket(key)?;
        if bucket.failures.len() < MAX_FAILURES {
            return None;
        }
        // `failures` is inserted chronologically, so the MAX-th entry is the
        // moment the lockout began.
        let start = bucket.failures[MAX_FAILURES - 1];
        let unlock_at = start + LOCKOUT;
        let now = Instant::now();
        if now < unlock_at {
            unlock_at
                .checked_duration_since(now)
                .map(|d| d.as_secs().max(1))
        } else {
            None
        }
    }

    pub fn is_blocked(&self, key: &str) -> bool {
        self.remaining_lockout(key).is_some()
    }

    pub fn record_failure(&self, key: &str) {
        let now = Instant::now();
        let mut buckets = self.buckets.lock().unwrap();
        let bucket = buckets
            .entry(key.to_string())
            .or_insert_with(|| Bucket { failures: vec![] });
        bucket.failures.retain(|t| now.duration_since(*t) < WINDOW);
        bucket.failures.push(now);
    }

    pub fn record_success(&self, key: &str) {
        self.buckets.lock().unwrap().remove(key);
    }
}

impl Default for LoginThrottle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_after_five_failures() {
        let throttle = LoginThrottle::new();
        for i in 0..5 {
            if i > 0 {
                assert!(!throttle.is_blocked("alice"));
            }
            throttle.record_failure("alice");
        }
        assert!(throttle.is_blocked("alice"));
        assert!(throttle.remaining_lockout("alice").unwrap() >= 1);
    }

    #[test]
    fn success_resets_the_bucket() {
        let throttle = LoginThrottle::new();
        for _ in 0..4 {
            throttle.record_failure("bob");
        }
        assert!(!throttle.is_blocked("bob"));
        throttle.record_success("bob");
        for _ in 0..5 {
            throttle.record_failure("bob");
        }
        assert!(throttle.is_blocked("bob"));
    }

    #[test]
    fn keys_are_independent() {
        let throttle = LoginThrottle::new();
        for _ in 0..5 {
            throttle.record_failure("carol");
        }
        assert!(throttle.is_blocked("carol"));
        assert!(!throttle.is_blocked("dave"));
    }
}
