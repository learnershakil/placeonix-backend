use std::sync::Arc;

use api_contracts::AppError;
use axum::{body::Body, extract::State, middleware::Next, response::Response};
use http::{HeaderMap, Request};
use placeonix_config::RateLimitConfig;
use redis::{aio::ConnectionManager, Script};
use tokio::sync::Mutex;

const USER_ID_HEADER: &str = "x-user-id";
const FORWARDED_FOR_HEADER: &str = "x-forwarded-for";
const REAL_IP_HEADER: &str = "x-real-ip";

#[derive(Clone)]
pub struct RateLimiter {
    connection: Arc<Mutex<ConnectionManager>>,
    config: RateLimitConfig,
}

impl RateLimiter {
    pub async fn connect(
        redis_url: &str,
        config: RateLimitConfig,
    ) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(redis_url)?;
        let connection = client.get_connection_manager().await?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            config,
        })
    }

    fn buckets_for_request(&self, request: &Request<Body>) -> RateLimitBuckets {
        RateLimitBuckets::from_request(request, &self.config)
    }

    async fn check_buckets(&self, buckets: RateLimitBuckets) -> Result<(), AppError> {
        for bucket in buckets {
            let current =
                increment_bucket(&self.connection, &bucket.key, self.config.window_secs).await?;
            if current > u64::from(bucket.limit) {
                return Err(AppError::rate_limited("rate limit exceeded"));
            }
        }
        Ok(())
    }
}

pub async fn enforce_rate_limits(
    State(rate_limiter): State<RateLimiter>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    let buckets = rate_limiter.buckets_for_request(&request);
    rate_limiter.check_buckets(buckets).await?;
    Ok(next.run(request).await)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RateLimitBucket {
    key: String,
    limit: u32,
}

struct RateLimitBuckets {
    buckets: Vec<RateLimitBucket>,
}

impl RateLimitBuckets {
    fn from_request(request: &Request<Body>, config: &RateLimitConfig) -> Self {
        let headers = request.headers();
        let mut buckets = Vec::with_capacity(3);
        buckets.push(RateLimitBucket {
            key: format!("rl:route:{}", sanitize_key(request.uri().path())),
            limit: config.per_route_requests,
        });

        if let Some(ip) = client_ip(headers) {
            buckets.push(RateLimitBucket {
                key: format!("rl:ip:{}", sanitize_key(&ip)),
                limit: config.per_ip_requests,
            });
        }

        if let Some(user_id) = header_value(headers, USER_ID_HEADER) {
            buckets.push(RateLimitBucket {
                key: format!("rl:user:{}", sanitize_key(&user_id)),
                limit: config.per_user_requests,
            });
        }

        Self { buckets }
    }
}

impl IntoIterator for RateLimitBuckets {
    type Item = RateLimitBucket;
    type IntoIter = std::vec::IntoIter<RateLimitBucket>;

    fn into_iter(self) -> Self::IntoIter {
        self.buckets.into_iter()
    }
}

async fn increment_bucket(
    connection: &Arc<Mutex<ConnectionManager>>,
    key: &str,
    window_secs: u64,
) -> Result<u64, AppError> {
    let script = Script::new(
        r#"
        local current = redis.call("INCR", KEYS[1])
        if current == 1 then
            redis.call("EXPIRE", KEYS[1], ARGV[1])
        end
        return current
        "#,
    );

    let mut connection = connection.lock().await;

    script
        .key(key)
        .arg(window_secs)
        .invoke_async(&mut *connection)
        .await
        .map_err(|_| AppError::service_unavailable("rate limiter unavailable"))
}

fn client_ip(headers: &HeaderMap) -> Option<String> {
    header_value(headers, FORWARDED_FOR_HEADER)
        .and_then(|value| value.split(',').next().map(str::trim).map(str::to_owned))
        .filter(|value| !value.is_empty())
        .or_else(|| header_value(headers, REAL_IP_HEADER))
}

fn header_value(headers: &HeaderMap, key: &str) -> Option<String> {
    headers
        .get(key)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn sanitize_key(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '/' => ch,
            _ => '_',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use http::Request;
    use placeonix_config::RateLimitConfig;

    use super::RateLimiter;
    use super::{client_ip, sanitize_key, RateLimitBuckets};

    fn assert_send_sync_clone<T: Send + Sync + Clone + 'static>() {}
    fn assert_send<T: Send>(_: T) {}
    fn assert_check_future_send(limiter: &RateLimiter, buckets: RateLimitBuckets) {
        assert_send(limiter.check_buckets(buckets));
    }

    fn config() -> RateLimitConfig {
        RateLimitConfig {
            window_secs: 60,
            per_ip_requests: 10,
            per_user_requests: 20,
            per_route_requests: 30,
        }
    }

    #[test]
    fn derives_route_ip_and_user_buckets() {
        let request = Request::builder()
            .uri("/api/v1/courses?ignored=true")
            .header("x-forwarded-for", "10.0.0.1, 10.0.0.2")
            .header("x-user-id", "user-1")
            .body(Body::empty())
            .unwrap();

        let buckets = RateLimitBuckets::from_request(&request, &config())
            .into_iter()
            .collect::<Vec<_>>();

        assert_eq!(buckets.len(), 3);
        assert!(buckets
            .iter()
            .any(|bucket| bucket.key == "rl:route:/api/v1/courses"));
        assert!(buckets.iter().any(|bucket| bucket.key == "rl:ip:10.0.0.1"));
        assert!(buckets.iter().any(|bucket| bucket.key == "rl:user:user-1"));
    }

    #[test]
    fn extracts_first_forwarded_ip() {
        let request = Request::builder()
            .header("x-forwarded-for", "10.0.0.1, 10.0.0.2")
            .body(())
            .unwrap();

        assert_eq!(client_ip(request.headers()).as_deref(), Some("10.0.0.1"));
    }

    #[test]
    fn sanitizes_redis_key_segments() {
        assert_eq!(sanitize_key("user:one two"), "user_one_two");
    }

    #[test]
    fn limiter_state_satisfies_middleware_bounds() {
        assert_send_sync_clone::<RateLimiter>();
        let _ = assert_check_future_send;
    }
}
