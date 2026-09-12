//! High-performance in-memory Token Bucket rate limiter (issue #162).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;
use pyo3::prelude::*;

const NUM_SHARDS: usize = 16;
const MAX_SHARD_ENTRIES: usize = 8192;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RateLimitKeyStrategy {
    Ip,
    Header(String),
    Global,
}

impl RateLimitKeyStrategy {
    pub fn parse(s: &str) -> Self {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("ip") || trimmed.eq_ignore_ascii_case("client") {
            Self::Ip
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
        let now = Instant::now();
        let idx = self.shard_index(key);
        let mut shard = self.shards[idx].lock();

        if shard.len() > MAX_SHARD_ENTRIES {
            let expire_cutoff = self.window_secs * 2.0;
            shard.retain(|_, v| (now - v.last_update).as_secs_f64() < expire_cutoff);
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
            if let Ok(client) = scope.getattr("client") {
                if let Ok(tuple) = client.extract::<&pyo3::types::PyTuple>() {
                    if let Ok(item) = tuple.get_item(0) {
                        if let Ok(ip) = item.extract::<String>() {
                            return ip;
                        }
                    }
                }
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
}
