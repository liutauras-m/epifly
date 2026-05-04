use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::warn;

struct TenantBucket {
    count: u32,
    window_start: Instant,
}

pub struct RateLimiter {
    buckets: Mutex<HashMap<String, TenantBucket>>,
    window: Duration,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
            window: Duration::from_secs(60),
        }
    }

    /// Returns true if the request is allowed, false if rate-limited.
    pub fn check(&self, tenant_id: &str, limit_rpm: u32) -> bool {
        let mut buckets = self.buckets.lock().unwrap();
        let now = Instant::now();

        let bucket = buckets
            .entry(tenant_id.to_string())
            .or_insert(TenantBucket {
                count: 0,
                window_start: now,
            });

        if now.duration_since(bucket.window_start) >= self.window {
            bucket.count = 0;
            bucket.window_start = now;
        }

        if bucket.count >= limit_rpm {
            warn!(tenant_id, limit_rpm, "rate limit exceeded");
            return false;
        }

        bucket.count += 1;
        true
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}
