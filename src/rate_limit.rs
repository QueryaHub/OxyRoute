//! High-performance in-memory Token Bucket rate limiter (issue #162).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;
use pyo3::prelude::*;

const NUM_SHARDS: usize = 16;
const MAX_SHARD_ENTRIES: usize = 8192;
/// Cap on the key string itself (issue #208): keys can come from client-controlled input
/// (e.g. a forwarded-IP header value), so without a bound a single request could grow a
/// shard's memory footprint arbitrarily via one oversized key.
const MAX_KEY_LEN: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RateLimitKeyStrategy {
    /// The actual TCP peer address (``scope.client``). Cannot be spoofed by the client —
    /// the safe default.
    Ip,
    /// `X-Forwarded-For` / `X-Real-IP`, falling back to ``scope.client``. **Only safe behind
    /// a trusted reverse proxy that overwrites these headers** — otherwise any client can pick
    /// its own rate-limit bucket, or frame another client's IP, by setting the header itself
    /// (issue #207). Opt in explicitly with ``rate_limit_key="trusted-forwarded-ip"``.
    TrustedForwardedIp,
    Header(String),
    Global,
}

impl RateLimitKeyStrategy {
    pub fn parse(s: &str) -> Self {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("ip") || trimmed.eq_ignore_ascii_case("client") {
            Self::Ip
        } else if trimmed.eq_ignore_ascii_case("trusted-forwarded-ip")
            || trimmed.eq_ignore_ascii_case("forwarded-ip")
        {
            Self::TrustedForwardedIp
        } else if trimmed.eq_ignore_ascii_case("global") {
            Self::Global
        } else if let Some(hdr) = trimmed.strip_prefix("header:") {
            Self::Header(hdr.trim().to_ascii_lowercase())
        } else {
            Self::Ip
        }
    }
}

#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    pub limit: u64,
    pub window_secs: f64,
    pub key_strategy: RateLimitKeyStrategy,
}

impl RateLimitConfig {
    pub fn parse(rate_str: &str, key_str: Option<&str>) -> Result<Self, String> {
        let trimmed = rate_str.trim();
        let parts: Vec<&str> = trimmed.split('/').collect();
        if parts.len() != 2 {
            return Err(format!(
                "invalid rate limit format: '{rate_str}' (expected 'limit/window', e.g. '100/minute')"
            ));
        }

        let limit: u64 = parts[0]
            .trim()
            .parse()
            .map_err(|_| format!("invalid rate limit count in '{rate_str}'"))?;

        if limit == 0 {
            return Err("rate limit count must be greater than 0".to_string());
        }

        let win_str = parts[1].trim().to_ascii_lowercase();
        let window_secs: f64 = match win_str.as_str() {
            "s" | "sec" | "second" | "seconds" => 1.0,
            "m" | "min" | "minute" | "minutes" => 60.0,
            "h" | "hr" | "hour" | "hours" => 3600.0,
            "d" | "day" | "days" => 86400.0,
            custom => {
                if let Some(num_str) = custom.strip_suffix('s') {
                    num_str
                        .parse::<f64>()
                        .map_err(|_| format!("invalid window duration in '{rate_str}'"))?
                } else {
                    return Err(format!("unknown rate limit window in '{rate_str}'"));
                }
            }
        };

        if window_secs <= 0.0 {
            return Err("rate limit window duration must be greater than 0".to_string());
        }

        let key_strategy = match key_str {
            Some(k) => RateLimitKeyStrategy::parse(k),
            None => RateLimitKeyStrategy::Ip,
        };

        Ok(Self {
            limit,
            window_secs,
            key_strategy,
        })
    }
}

/// Truncate `key` to `MAX_KEY_LEN` bytes on a valid UTF-8 boundary.
fn truncate_key(key: &str) -> &str {
    if key.len() <= MAX_KEY_LEN {
        return key;
    }
    let mut end = MAX_KEY_LEN;
    while end > 0 && !key.is_char_boundary(end) {
        end -= 1;
    }
    &key[..end]
}

#[derive(Clone, Copy, Debug)]
struct BucketState {
    tokens: f64,
    last_update: Instant,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RateLimitDecision {
    Allowed {
        limit: u64,
        remaining: u64,
        reset_secs: u64,
    },
    Denied {
        limit: u64,
        reset_secs: u64,
    },
}

pub struct RateLimiter {
    pub limit: u64,
    /// Kept for introspection alongside `limit`/`refill_rate_per_sec`; the eviction path no
    /// longer reads it directly (see issue #208 — replaced the periodic `retain` sweep with
    /// bounded per-shard eviction).
    #[allow(dead_code)]
    pub window_secs: f64,
    pub refill_rate_per_sec: f64,
    pub key_strategy: RateLimitKeyStrategy,
    shards: [Mutex<HashMap<String, BucketState>>; NUM_SHARDS],
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Arc<Self> {
        let refill_rate_per_sec = config.limit as f64 / config.window_secs;
        let shards = std::array::from_fn(|_| Mutex::new(HashMap::new()));
        Arc::new(Self {
            limit: config.limit,
            window_secs: config.window_secs,
            refill_rate_per_sec,
            key_strategy: config.key_strategy,
            shards,
        })
    }

    #[inline]
    fn shard_index(&self, key: &str) -> usize {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % NUM_SHARDS
    }

    pub fn check(&self, key: &str) -> RateLimitDecision {
        let key = truncate_key(key);
        let now = Instant::now();
        let idx = self.shard_index(key);
        let mut shard = self.shards[idx].lock();

        // Bound each shard's memory strictly: if this is a new key and the shard is already
        // at capacity, evict one existing entry first instead of letting the map grow without
        // limit (issue #208). This keeps `check` O(1) per request even under a flood of unique
        // keys — no full-shard scan on the hot path, unlike the previous periodic `retain`.
        if shard.len() >= MAX_SHARD_ENTRIES && !shard.contains_key(key) {
            if let Some(evict) = shard.keys().next().cloned() {
                shard.remove(&evict);
            }
        }

        let limit_f64 = self.limit as f64;
        let bucket = shard.entry(key.to_string()).or_insert_with(|| BucketState {
            tokens: limit_f64,
            last_update: now,
        });

        let elapsed = (now - bucket.last_update).as_secs_f64();
        let refilled = (bucket.tokens + elapsed * self.refill_rate_per_sec).min(limit_f64);

        if refilled >= 1.0 {
            let remaining_tokens = refilled - 1.0;
            bucket.tokens = remaining_tokens;
            bucket.last_update = now;

            let remaining = remaining_tokens.floor() as u64;
            let reset_secs = if remaining == 0 {
                ((1.0 - remaining_tokens) / self.refill_rate_per_sec)
                    .ceil()
                    .max(1.0) as u64
            } else {
                ((limit_f64 - remaining_tokens) / self.refill_rate_per_sec)
                    .ceil()
                    .max(1.0) as u64
            };

            RateLimitDecision::Allowed {
                limit: self.limit,
                remaining,
                reset_secs,
            }
        } else {
            let missing = 1.0 - refilled;
            let reset_secs = (missing / self.refill_rate_per_sec).ceil().max(1.0) as u64;

            RateLimitDecision::Denied {
                limit: self.limit,
                reset_secs,
            }
        }
    }
}

/// Read the peer address off ``scope.client``. Granian's RSGI scope exposes this as a
/// ``"host:port"`` string; the ASGI bridge / test scopes expose an ASGI-style ``(host, port)``
/// tuple. Handles both so the "safe" strategies below get the real peer, not (e.g.) the first
/// character of a ``"host:port"`` string via a stray ``__getitem__``.
fn scope_client_host(scope: &pyo3::Bound<'_, pyo3::PyAny>) -> Option<String> {
    let client = scope.getattr("client").ok()?;
    if let Ok(s) = client.extract::<String>() {
        if s.is_empty() {
            return None;
        }
        // "host:port" (IPv4) or "[::1]:port" (IPv6) — split off the trailing ":port".
        if let Some(rest) = s.strip_prefix('[') {
            if let Some(end) = rest.find(']') {
                return Some(format!("[{}]", &rest[..end]));
            }
        }
        return match s.rsplit_once(':') {
            Some((host, port)) if port.chars().all(|c| c.is_ascii_digit()) => {
                Some(host.to_string())
            }
            _ => Some(s),
        };
    }
    if let Ok(item) = client.get_item(0) {
        if let Ok(ip) = item.extract::<String>() {
            return Some(ip);
        }
    }
    None
}

/// Extract rate limit key from Granian / ASGI scope based on strategy.
pub fn extract_rate_limit_key(
    scope: &pyo3::Bound<'_, pyo3::PyAny>,
    strategy: &RateLimitKeyStrategy,
) -> String {
    match strategy {
        RateLimitKeyStrategy::Global => "global".to_string(),
        RateLimitKeyStrategy::Header(hdr_name) => {
            if let Ok(headers) = scope.getattr("headers") {
                if let Some(val) = crate::params::header_get_lax(&headers, hdr_name) {
                    return val;
                }
            }
            "unknown".to_string()
        }
        RateLimitKeyStrategy::Ip => {
            if let Some(ip) = scope_client_host(scope) {
                return ip;
            }
            "127.0.0.1".to_string()
        }
        RateLimitKeyStrategy::TrustedForwardedIp => {
            if let Ok(headers) = scope.getattr("headers") {
                if let Some(xf) = crate::params::header_get_lax(&headers, "x-forwarded-for") {
                    if let Some(first_ip) = xf.split(',').next() {
                        let trimmed = first_ip.trim();
                        if !trimmed.is_empty() {
                            return trimmed.to_string();
                        }
                    }
                }
                if let Some(xr) = crate::params::header_get_lax(&headers, "x-real-ip") {
                    let trimmed = xr.trim();
                    if !trimmed.is_empty() {
                        return trimmed.to_string();
                    }
                }
            }
            if let Some(ip) = scope_client_host(scope) {
                return ip;
            }
            "127.0.0.1".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rate_limit_config() {
        let c1 = RateLimitConfig::parse("10/second", None).unwrap();
        assert_eq!(c1.limit, 10);
        assert_eq!(c1.window_secs, 1.0);
        assert_eq!(c1.key_strategy, RateLimitKeyStrategy::Ip);

        let c2 = RateLimitConfig::parse("100/minute", Some("header:X-API-Key")).unwrap();
        assert_eq!(c2.limit, 100);
        assert_eq!(c2.window_secs, 60.0);
        assert_eq!(
            c2.key_strategy,
            RateLimitKeyStrategy::Header("x-api-key".to_string())
        );

        let c3 = RateLimitConfig::parse("500/hour", Some("global")).unwrap();
        assert_eq!(c3.limit, 500);
        assert_eq!(c3.window_secs, 3600.0);
        assert_eq!(c3.key_strategy, RateLimitKeyStrategy::Global);

        assert!(RateLimitConfig::parse("invalid", None).is_err());
        assert!(RateLimitConfig::parse("0/second", None).is_err());
    }

    #[test]
    fn test_rate_limiter_allow_and_deny() {
        let config = RateLimitConfig {
            limit: 2,
            window_secs: 10.0,
            key_strategy: RateLimitKeyStrategy::Ip,
        };
        let limiter = RateLimiter::new(config);

        match limiter.check("1.2.3.4") {
            RateLimitDecision::Allowed {
                limit, remaining, ..
            } => {
                assert_eq!(limit, 2);
                assert_eq!(remaining, 1);
            }
            RateLimitDecision::Denied { .. } => panic!("should be allowed"),
        }

        match limiter.check("1.2.3.4") {
            RateLimitDecision::Allowed {
                limit, remaining, ..
            } => {
                assert_eq!(limit, 2);
                assert_eq!(remaining, 0);
            }
            RateLimitDecision::Denied { .. } => panic!("should be allowed"),
        }

        match limiter.check("1.2.3.4") {
            RateLimitDecision::Denied { limit, reset_secs } => {
                assert_eq!(limit, 2);
                assert!(reset_secs >= 1);
            }
            RateLimitDecision::Allowed { .. } => panic!("should be denied"),
        }

        // Different IP is untouched
        match limiter.check("5.6.7.8") {
            RateLimitDecision::Allowed { remaining, .. } => {
                assert_eq!(remaining, 1);
            }
            RateLimitDecision::Denied { .. } => panic!("should be allowed"),
        }
    }

    #[test]
    fn test_shard_bounded_under_unique_key_flood() {
        // issue #208: a flood of unique keys must not grow a shard's map past
        // MAX_SHARD_ENTRIES, and `check` must keep working (not panic/hang) throughout.
        let config = RateLimitConfig {
            limit: 5,
            window_secs: 60.0,
            key_strategy: RateLimitKeyStrategy::Ip,
        };
        let limiter = RateLimiter::new(config);

        for i in 0..(MAX_SHARD_ENTRIES * NUM_SHARDS * 2) {
            let key = format!("unique-key-{i}");
            limiter.check(&key);
        }

        for shard in &limiter.shards {
            assert!(
                shard.lock().len() <= MAX_SHARD_ENTRIES,
                "shard grew past MAX_SHARD_ENTRIES"
            );
        }
    }

    #[test]
    fn test_key_length_is_capped() {
        let config = RateLimitConfig {
            limit: 5,
            window_secs: 60.0,
            key_strategy: RateLimitKeyStrategy::Ip,
        };
        let limiter = RateLimiter::new(config);

        let huge_key = "x".repeat(1_000_000);
        limiter.check(&huge_key);

        let total_len: usize = limiter
            .shards
            .iter()
            .flat_map(|s| s.lock().keys().map(|k| k.len()).collect::<Vec<_>>())
            .sum();
        assert!(total_len <= MAX_KEY_LEN);
    }
}
