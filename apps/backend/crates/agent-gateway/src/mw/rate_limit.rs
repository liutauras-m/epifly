use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::warn;

struct Bucket {
    count: u32,
    window_start: Instant,
}

/// Generic sliding-window rate limiter keyed by an arbitrary string key.
///
/// Used for tenant quotas (key = tenant_id) and auth rate limiting (key = IP prefix).
pub struct RateLimiter {
    buckets: Mutex<HashMap<String, Bucket>>,
    window: Duration,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
            window: Duration::from_secs(60),
        }
    }

    /// Returns `true` if the request is allowed, `false` if the limit is exceeded.
    pub fn check(&self, key: &str, limit_rpm: u32) -> bool {
        let mut buckets = self.buckets.lock().unwrap();
        let now = Instant::now();

        let bucket = buckets.entry(key.to_string()).or_insert(Bucket {
            count: 0,
            window_start: now,
        });

        if now.duration_since(bucket.window_start) >= self.window {
            bucket.count = 0;
            bucket.window_start = now;
        }

        if bucket.count >= limit_rpm {
            warn!(key, limit_rpm, "rate limit exceeded");
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

/// Extract an IP-based rate-limit key from request headers.
/// Prefers `CF-Connecting-IP`, falls back to `X-Forwarded-For`, then `unknown`.
pub fn ip_key(headers: &axum::http::HeaderMap) -> String {
    if let Some(v) = headers
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
    {
        return v.trim().to_string();
    }
    if let Some(v) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        return v.split(',').next().unwrap_or("unknown").trim().to_string();
    }
    "unknown".to_string()
}
