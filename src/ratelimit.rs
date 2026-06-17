use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

/// Simple in-memory token-bucket rate limiter keyed by an arbitrary string
/// (we key by visitor id). Refills continuously at `rate` tokens/second up to
/// `burst`. No external dependency, good enough for write-endpoint protection.
pub struct RateLimiter {
    rate: f64,
    burst: f64,
    buckets: Mutex<HashMap<String, (f64, Instant)>>,
}

impl RateLimiter {
    /// `per_min` sustained writes, allowing a `burst` of immediate ones.
    pub fn new(per_min: f64, burst: f64) -> Self {
        RateLimiter {
            rate: per_min / 60.0,
            burst,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Returns true if the action is allowed (and consumes a token).
    pub fn check(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut map = self.buckets.lock().unwrap();

        // Opportunistic cleanup so the map can't grow unbounded.
        if map.len() > 50_000 {
            map.retain(|_, (tokens, last)| {
                *tokens + now.duration_since(*last).as_secs_f64() * self.rate < self.burst
            });
        }

        let entry = map.entry(key.to_string()).or_insert((self.burst, now));
        let (tokens, last) = entry;
        let elapsed = now.duration_since(*last).as_secs_f64();
        *tokens = (*tokens + elapsed * self.rate).min(self.burst);
        *last = now;
        if *tokens >= 1.0 {
            *tokens -= 1.0;
            true
        } else {
            false
        }
    }
}
