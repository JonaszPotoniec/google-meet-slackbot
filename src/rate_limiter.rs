use anyhow::{bail, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct RateLimiter {
    user_limits: Arc<RwLock<HashMap<String, UserRateLimit>>>,
    endpoint_limits: Arc<RwLock<HashMap<String, EndpointRateLimit>>>,
}

#[derive(Debug, Clone)]
struct UserRateLimit {
    request_count: u32,
    window_start: Instant,
    last_blocked: Option<Instant>,
    backoff_duration: Duration,
}

#[derive(Debug, Clone)]
struct EndpointRateLimit {
    request_count: u32,
    window_start: Instant,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            user_limits: Arc::new(RwLock::new(HashMap::new())),
            endpoint_limits: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn check_user_limit(&self, user_id: &str, endpoint: &str) -> Result<()> {
        let now = Instant::now();

        let (max_requests, window_duration) = match endpoint {
            "/slack/commands" => (10, Duration::from_secs(60)),
            "/auth/google" => (5, Duration::from_secs(300)),
            "/auth/google/callback" => (10, Duration::from_secs(300)),
            _ => (100, Duration::from_secs(60)),
        };

        let mut user_limits = self.user_limits.write().await;
        let user_limit = user_limits
            .entry(user_id.to_string())
            .or_insert_with(|| UserRateLimit {
                request_count: 0,
                window_start: now,
                last_blocked: None,
                backoff_duration: Duration::from_secs(1),
            });

        if let Some(last_blocked) = user_limit.last_blocked {
            if now.duration_since(last_blocked) < user_limit.backoff_duration {
                bail!(
                    "User {} is in backoff period for {} seconds",
                    user_id,
                    user_limit.backoff_duration.as_secs()
                );
            }
        }

        if now.duration_since(user_limit.window_start) >= window_duration {
            user_limit.request_count = 0;
            user_limit.window_start = now;
        }

        if user_limit.request_count >= max_requests {
            user_limit.last_blocked = Some(now);
            user_limit.backoff_duration = std::cmp::min(
                user_limit.backoff_duration * 2,
                Duration::from_secs(15 * 60),
            );

            bail!(
                "Rate limit exceeded for user {}: {} requests in {} seconds",
                user_id,
                max_requests,
                window_duration.as_secs()
            );
        }

        user_limit.request_count += 1;
        Ok(())
    }

    pub async fn check_endpoint_limit(&self, endpoint: &str) -> Result<()> {
        let now = Instant::now();

        let (max_requests, window_duration) = match endpoint {
            "/slack/commands" => (1000, Duration::from_secs(60)),
            "/auth/google" => (200, Duration::from_secs(60)),
            "/auth/google/callback" => (500, Duration::from_secs(60)),
            _ => (5000, Duration::from_secs(60)),
        };

        let mut endpoint_limits = self.endpoint_limits.write().await;
        let endpoint_limit = endpoint_limits
            .entry(endpoint.to_string())
            .or_insert_with(|| EndpointRateLimit {
                request_count: 0,
                window_start: now,
            });

        if now.duration_since(endpoint_limit.window_start) >= window_duration {
            endpoint_limit.request_count = 0;
            endpoint_limit.window_start = now;
        }

        if endpoint_limit.request_count >= max_requests {
            bail!(
                "Global rate limit exceeded for endpoint {}: {} requests in {} seconds",
                endpoint,
                max_requests,
                window_duration.as_secs()
            );
        }

        endpoint_limit.request_count += 1;
        Ok(())
    }

    pub async fn cleanup_old_entries(&self) {
        let now = Instant::now();
        let cleanup_threshold = Duration::from_secs(60 * 60); // 1 hour

        // Clean up user limits
        {
            let mut user_limits = self.user_limits.write().await;
            user_limits
                .retain(|_, limit| now.duration_since(limit.window_start) < cleanup_threshold);
        }

        // Clean up endpoint limits
        {
            let mut endpoint_limits = self.endpoint_limits.write().await;
            endpoint_limits
                .retain(|_, limit| now.duration_since(limit.window_start) < cleanup_threshold);
        }
    }
}

/// Background task to periodically clean up old rate limit entries
pub async fn start_cleanup_task(rate_limiter: RateLimiter) {
    let mut interval = tokio::time::interval(Duration::from_secs(10 * 60)); // 10 minutes

    loop {
        interval.tick().await;
        rate_limiter.cleanup_old_entries().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_user_rate_limiting() {
        let rate_limiter = RateLimiter::new();
        let user_id = "U1234567890";
        let endpoint = "/slack/commands";

        // First few requests should pass
        for _ in 0..5 {
            assert!(rate_limiter
                .check_user_limit(user_id, endpoint)
                .await
                .is_ok());
        }
    }

    #[tokio::test]
    async fn test_endpoint_rate_limiting() {
        let rate_limiter = RateLimiter::new();
        let endpoint = "/slack/commands";

        // Test global endpoint limiting
        assert!(rate_limiter.check_endpoint_limit(endpoint).await.is_ok());
    }
}
