//! Fixed-window in-memory rate limiting.
//!
//! Limits are approximate under multiple actix workers because each worker
//! builds its own limiter; this is deliberate for spam mitigation at this
//! site's scale.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct RateLimiter {
    entries: Mutex<HashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a hit for `key` and reports whether the caller is under the
    /// limit of `max` hits per `window`.
    pub fn check(&self, key: &str, max: usize, window: Duration) -> bool {
        let now = Instant::now();
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let hits = entries.entry(key.to_owned()).or_default();
        hits.retain(|at| now.duration_since(*at) < window);
        if hits.len() >= max {
            return false;
        }
        hits.push(now);
        // Opportunistic cleanup so long-tail keys do not accumulate.
        if entries.len() > 10_000 {
            entries.retain(|_, times| {
                times
                    .last()
                    .is_some_and(|last| now.duration_since(*last) < window)
            });
        }
        true
    }

    /// Current hit count for `key` within `window` without recording one.
    pub fn count(&self, key: &str, window: Duration) -> usize {
        let now = Instant::now();
        let entries = self
            .entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        entries
            .get(key)
            .map(|hits| {
                hits.iter()
                    .filter(|at| now.duration_since(**at) < window)
                    .count()
            })
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_limit_then_blocks() {
        let limiter = RateLimiter::new();
        for _ in 0..3 {
            assert!(limiter.check("k", 3, Duration::from_secs(60)));
        }
        assert!(!limiter.check("k", 3, Duration::from_secs(60)));
        assert_eq!(limiter.count("k", Duration::from_secs(60)), 3);
        assert_eq!(limiter.count("other", Duration::from_secs(60)), 0);
    }
}
