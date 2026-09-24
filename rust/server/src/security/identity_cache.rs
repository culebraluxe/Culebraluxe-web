//! An optional cache for identity resolution.
//!
//! WHY. Resolving an identity is two queries against a database that is a network away: one to find the application
//! user from the provider subject, one to load that user's roles and authorities. Measured here, a round trip is 72ms,
//! so that is ~144ms on every single Rust request - and a screen makes several requests, so the same person pays it
//! over and over inside one page load. The typescript path never paid it at all, because it did not re-resolve.
//! Entitlement enforcement defaults this cache to disabled so role revocation takes effect on the next request.
//!
//! WHAT IS CACHED: `Known` resolutions only, briefly. A miss caches nothing, so a user who is mapped a moment after
//! being looked up is not locked out for the life of an entry - the failure mode of caching a "no" is much worse than
//! the cost of asking again.
//!
//! WHAT IS NOT CHANGED: authorization still runs on every request. Only the two lookups that produce the principal are
//! cached, so the audit trail records the operation exactly as before. The trade is that a role change can take up to
//! the TTL to take effect when an operator explicitly enables it (`FORGE_IDENTITY_CACHE_MS`, 0 disables).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use domain::SecurityPrincipal;

/// Bounded so a process cannot accumulate principals without limit. Full means cleared, not evicted one at a time:
/// this is a latency cache, and losing it costs a query rather than correctness.
const MAX_ENTRIES: usize = 1024;

/// The TTL, parsed once. Disabled by default so changed or revoked grants
/// take effect on the next request. Set `FORGE_IDENTITY_CACHE_MS` to opt into
/// a bounded delay before role and entitlement changes become effective.
fn ttl() -> Option<Duration> {
    static TTL: OnceLock<Option<Duration>> = OnceLock::new();
    *TTL.get_or_init(|| {
        let millis = match std::env::var("FORGE_IDENTITY_CACHE_MS") {
            Ok(raw) => raw.trim().parse::<u64>().unwrap_or(DEFAULT_TTL_MS),
            Err(_) => DEFAULT_TTL_MS,
        };
        (millis > 0).then(|| Duration::from_millis(millis))
    })
}

// Default to fresh role and entitlement resolution on every request. A stale
// grant after revocation is more costly than the identity lookup round trips.
const DEFAULT_TTL_MS: u64 = 0;

fn cache() -> &'static Mutex<HashMap<(String, String), (Instant, SecurityPrincipal)>> {
    static CACHE: OnceLock<Mutex<HashMap<(String, String), (Instant, SecurityPrincipal)>>> =
        OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn get(provider: &str, provider_subject: &str) -> Option<SecurityPrincipal> {
    let ttl = ttl()?;
    let mut guard = cache().lock().ok()?;
    let entry = guard.get(&(provider.to_owned(), provider_subject.to_owned()))?;
    if entry.0.elapsed() >= ttl {
        guard.remove(&(provider.to_owned(), provider_subject.to_owned()));
        return None;
    }
    Some(entry.1.clone())
}

pub fn put(provider: &str, provider_subject: &str, principal: &SecurityPrincipal) {
    let Some(_ttl) = ttl() else {
        return;
    };
    let Ok(mut guard) = cache().lock() else {
        return;
    };
    if guard.len() >= MAX_ENTRIES {
        guard.clear();
    }
    guard.insert(
        (provider.to_owned(), provider_subject.to_owned()),
        (Instant::now(), principal.clone()),
    );
}
