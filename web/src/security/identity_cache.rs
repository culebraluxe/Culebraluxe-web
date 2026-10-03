//! Startup-warmed identity map for the trusted application edge.
//!
//! WHY. Resolving an identity is two queries against a database that is a network away: one to find the application
//! user from the provider subject, one to load that user's roles and authorities. Measured here, a round trip is 72ms,
//! so that is ~144ms on every single Rust request - and a screen makes several requests, so the same person pays it
//! over and over inside one page load. The service loads every active identity before it accepts traffic, so request
//! handling translates the authenticated provider subject in memory.
//!
//! Only known active identities are held. A miss may still use the database for a newly provisioned identity, then
//! joins the map; ordinary signed-in requests do not reauthenticate or re-read the principal.

use std::collections::HashMap;
use std::sync::Mutex;

use model::SecurityPrincipal;

/// Bounded so a process cannot accumulate principals without limit. Full means cleared, not evicted one at a time:
/// this is a latency cache, and losing it costs a query rather than correctness.
const MAX_ENTRIES: usize = 1024;

#[derive(Default)]
pub struct IdentityCache {
    entries: Mutex<HashMap<(String, String), SecurityPrincipal>>,
}

impl IdentityCache {
    pub fn get(&self, provider: &str, provider_subject: &str) -> Option<SecurityPrincipal> {
        self.entries
            .lock()
            .ok()?
            .get(&(provider.to_owned(), provider_subject.to_owned()))
            .cloned()
    }

    pub fn put(&self, provider: &str, provider_subject: &str, principal: &SecurityPrincipal) {
        let Ok(mut guard) = self.entries.lock() else {
            return;
        };
        if guard.len() >= MAX_ENTRIES {
            guard.clear();
        }
        guard.insert(
            (provider.to_owned(), provider_subject.to_owned()),
            principal.clone(),
        );
    }

    pub fn replace(&self, entries: impl IntoIterator<Item = (String, String, SecurityPrincipal)>) {
        let Ok(mut guard) = self.entries.lock() else {
            return;
        };
        guard.clear();
        guard.extend(
            entries
                .into_iter()
                .map(|(provider, subject, principal)| ((provider, subject), principal)),
        );
    }
}
