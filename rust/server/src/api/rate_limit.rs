//! In-process rate limiting for the routes anyone on the internet can call.
//!
//! WHY THIS EXISTS: the public write routes — the website's lead form, and the guest sign-in code — were the only
//! unauthenticated endpoints with no ceiling of any kind. A form post costs a database write and two emails; the
//! sign-in verify is a six-digit guess. Neither had a limit that a script would notice.
//!
//! WHY IN PROCESS, NOT IN THE DATABASE: this is a cheap first line — one counter per address, no round trip — and
//! the router is served by one long-lived process (the same reason the identity cache is in-process). It is
//! deliberately NOT the only line: the guest sign-in codes are limited in the DATABASE (`security/guest.rs`:
//! five an hour per address, twenty an hour per IP), because a limit that must survive a redeploy cannot live in
//! the memory of the process being replaced. Anything load-bearing for money or security belongs there too.
//!
//! WHAT IT IS NOT: not distributed (two processes count separately), and not a replacement for authorization —
//! every route this guards is public by design, and this only decides how fast one address may use it.
//!
//! Limits and window come from the environment with defaults in code, so no deployment has to set anything:
//! `FORGE_RATE_LIMIT_INTAKE_PER_MIN`, `FORGE_RATE_LIMIT_GUEST_CODE_PER_MIN`.

use axum::http::{HeaderMap, StatusCode};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::ApiError;

/// Above this many tracked keys, expired buckets are dropped. One public route can be called with unlimited
/// distinct keys, so the map would otherwise be a memory leak an attacker controls.
const PRUNE_ABOVE: usize = 4096;

/// A fixed-window counter per client key.
pub(crate) struct RateLimiter {
    limit: u32,
    window: Duration,
    buckets: Mutex<HashMap<String, Bucket>>,
}

struct Bucket {
    count: u32,
    started: Instant,
}

impl RateLimiter {
    pub(crate) fn new(limit: u32, window: Duration) -> Self {
        Self {
            limit,
            window,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// `true` when this call is within the limit, `false` when it is over it.
    ///
    /// The window is fixed, not sliding: a caller that spends its allowance in the last second of a window gets a
    /// fresh one immediately. That is deliberate — a sliding window costs per-call bookkeeping for a sharper edge,
    /// and this limit exists to blunt a script, not to meter a customer.
    pub(crate) fn allow(&self, key: &str, now: Instant) -> bool {
        let mut buckets = self
            .buckets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if buckets.len() > PRUNE_ABOVE {
            let window = self.window;
            buckets.retain(|_, bucket| now.duration_since(bucket.started) < window);
        }

        let bucket = buckets.entry(key.to_owned()).or_insert(Bucket {
            count: 0,
            started: now,
        });
        if now.duration_since(bucket.started) >= self.window {
            bucket.count = 0;
            bucket.started = now;
        }
        bucket.count += 1;
        bucket.count <= self.limit
    }

    #[cfg(test)]
    fn tracked_keys(&self) -> usize {
        self.buckets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }
}
/// The address to count against.
///
/// The RIGHT-most `x-forwarded-for` entry, not the left-most: a caller can prepend anything it likes, so the
/// left-most value lets an attacker pick a fresh bucket on every request and the limit would never fire. The last
/// entry is the one our own edge appended. `x-real-ip` is the fallback, and a request carrying neither is counted
/// under one shared key — the safe direction: unknown callers share a bucket rather than each getting their own.
pub(crate) fn client_key(headers: &HeaderMap) -> String {
    let forwarded = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next_back())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    forwarded
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .unwrap_or("unknown")
        .to_owned()
}

fn limit_from_env(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|limit| *limit > 0)
        .unwrap_or(default)
}

fn intake_limiter() -> &'static RateLimiter {
    static LIMITER: OnceLock<RateLimiter> = OnceLock::new();
    LIMITER.get_or_init(|| {
        // Ten a minute is generous for a person filling in a form and invisible to one; a script posting a
        // thousand leads gets a 429 after ten.
        RateLimiter::new(
            limit_from_env("FORGE_RATE_LIMIT_INTAKE_PER_MIN", 10),
            Duration::from_secs(60),
        )
    })
}

fn guest_code_limiter() -> &'static RateLimiter {
    static LIMITER: OnceLock<RateLimiter> = OnceLock::new();
    LIMITER.get_or_init(|| {
        // Above the database's own twenty an hour per IP, so this only catches a burst. The durable limit stays
        // the one in `security/guest.rs`, which is where a limit that must survive a restart belongs.
        RateLimiter::new(
            limit_from_env("FORGE_RATE_LIMIT_GUEST_CODE_PER_MIN", 5),
            Duration::from_secs(60),
        )
    })
}

fn too_many() -> ApiError {
    ApiError::new(
        StatusCode::TOO_MANY_REQUESTS,
        "TOO_MANY_REQUESTS",
        "Too many requests from this address. Please try again in a minute.",
        true,
    )
}

/// Guards the website's lead form (`POST /api/rust-ui/website-intake`).
pub(crate) fn guard_intake(headers: &HeaderMap) -> Result<(), ApiError> {
    if intake_limiter().allow(&client_key(headers), Instant::now()) {
        Ok(())
    } else {
        Err(too_many())
    }
}

/// Guards the guest sign-in code routes (`POST /v1/security/guest-code` and `/verify`).
pub(crate) fn guard_guest_code(headers: &HeaderMap) -> Result<(), ApiError> {
    if guest_code_limiter().allow(&client_key(headers), Instant::now()) {
        Ok(())
    } else {
        Err(too_many())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_the_limit_and_refuses_the_next_call() {
        let limiter = RateLimiter::new(3, Duration::from_secs(60));
        let now = Instant::now();
        assert!(limiter.allow("1.2.3.4", now));
        assert!(limiter.allow("1.2.3.4", now));
        assert!(limiter.allow("1.2.3.4", now));
        assert!(!limiter.allow("1.2.3.4", now), "the fourth call is over");
    }

    #[test]
    fn a_new_window_restores_the_allowance() {
        let limiter = RateLimiter::new(1, Duration::from_secs(60));
        let start = Instant::now();
        assert!(limiter.allow("1.2.3.4", start));
        assert!(!limiter.allow("1.2.3.4", start + Duration::from_secs(30)));
        assert!(limiter.allow("1.2.3.4", start + Duration::from_secs(61)));
    }

    #[test]
    fn one_address_cannot_spend_another_addresses_allowance() {
        let limiter = RateLimiter::new(1, Duration::from_secs(60));
        let now = Instant::now();
        assert!(limiter.allow("1.2.3.4", now));
        assert!(!limiter.allow("1.2.3.4", now));
        assert!(
            limiter.allow("5.6.7.8", now),
            "a different address is unaffected"
        );
    }

    #[test]
    fn expired_buckets_are_dropped_once_the_map_grows() {
        let limiter = RateLimiter::new(1, Duration::from_secs(60));
        let start = Instant::now();
        for index in 0..(PRUNE_ABOVE + 10) {
            limiter.allow(&format!("10.0.{}.{}", index / 256, index % 256), start);
        }
        assert_eq!(limiter.tracked_keys(), PRUNE_ABOVE + 10);
        // A call in the next window prunes every bucket from the previous one.
        limiter.allow("10.0.0.1", start + Duration::from_secs(61));
        assert_eq!(limiter.tracked_keys(), 1);
    }

    #[test]
    fn the_key_is_the_last_forwarded_hop_so_a_caller_cannot_choose_its_bucket() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "9.9.9.9, 1.2.3.4".parse().expect("a valid header value"),
        );
        assert_eq!(client_key(&headers), "1.2.3.4");

        let mut real_ip_only = HeaderMap::new();
        real_ip_only.insert(
            "x-real-ip",
            "5.6.7.8".parse().expect("a valid header value"),
        );
        assert_eq!(client_key(&real_ip_only), "5.6.7.8");

        // No address at all: every such caller shares one bucket, which is the safe direction.
        assert_eq!(client_key(&HeaderMap::new()), "unknown");
    }
}
