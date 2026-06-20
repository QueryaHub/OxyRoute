//! Runtime configuration from environment variables (read once on first use).

use std::sync::OnceLock;

fn parse_bool_env(v: &str) -> bool {
    v == "1" || v.eq_ignore_ascii_case("true")
}

/// When true, HTTP 500 responses may include exception detail (``OXYROUTE_DEBUG=1`` / ``true``).
pub fn oxyroute_debug() -> bool {
    static CACHE: OnceLock<bool> = OnceLock::new();
    *CACHE.get_or_init(|| {
        std::env::var("OXYROUTE_DEBUG")
            .map(|v| parse_bool_env(&v))
            .unwrap_or(false)
    })
}

/// Maximum request body size in bytes (``OXYROUTE_MAX_BODY_BYTES``). Default 8 MiB; ``0`` = no limit.
pub fn max_body_bytes() -> u64 {
    const DEFAULT: u64 = 8 * 1024 * 1024;
    static CACHE: OnceLock<u64> = OnceLock::new();
    *CACHE.get_or_init(|| {
        match std::env::var("OXYROUTE_MAX_BODY_BYTES")
            .ok()
            .and_then(|s| s.parse().ok())
        {
            None => DEFAULT,
            Some(0) => u64::MAX,
            Some(n) => n,
        }
    })
}
