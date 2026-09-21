//! Counters for what the pool is actually doing.
//!
//! WHY. Every performance question in this workspace has been answered by timing the outside of a request and reasoning
//! inwards, which is slow and can be wrong. The number that matters most for a remote database is how often a request
//! has to *open a connection* rather than reuse one - that is the 498ms case, and it is invisible from the outside
//! because it looks like "the database is slow".
//!
//! WHAT IS COUNTED. Checkouts, connections opened, idle probes, and the pool's current size. Not query duration: the
//! DAOs hold `&PgPool` directly, so there is no single place to wrap a query, and inventing one would mean touching
//! every call site. The counters here come from hooks sqlx already gives the pool, so they cost nothing on the hot path.

use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct Counters {
    /// Every time a connection is handed out.
    pub checkouts: AtomicU64,
    /// Of those, the ones that needed a NEW connection - the expensive case.
    pub connections_opened: AtomicU64,
    /// Checkouts where the connection had been idle long enough to be probed.
    pub idle_probes: AtomicU64,
    /// Probes that found a dead connection, which is what the probe exists to catch.
    pub probes_failed: AtomicU64,
}

pub static COUNTERS: Counters = Counters {
    checkouts: AtomicU64::new(0),
    connections_opened: AtomicU64::new(0),
    idle_probes: AtomicU64::new(0),
    probes_failed: AtomicU64::new(0),
};

pub fn record_checkout() {
    COUNTERS.checkouts.fetch_add(1, Ordering::Relaxed);
}

pub fn record_connection_opened() {
    COUNTERS.connections_opened.fetch_add(1, Ordering::Relaxed);
}

pub fn record_idle_probe() {
    COUNTERS.idle_probes.fetch_add(1, Ordering::Relaxed);
}

pub fn record_probe_failed() {
    COUNTERS.probes_failed.fetch_add(1, Ordering::Relaxed);
}

#[derive(Debug, Clone, Copy)]
pub struct Snapshot {
    pub checkouts: u64,
    pub connections_opened: u64,
    pub idle_probes: u64,
    pub probes_failed: u64,
    pub pool_size: u32,
    pub pool_idle: u32,
}

impl Snapshot {
    /// The share of checkouts that had to open a connection. This is the number to watch: high means the pool is cold
    /// and every request is paying a handshake, low means it is doing its job.
    pub fn connection_reuse_rate(&self) -> f64 {
        if self.checkouts == 0 {
            return 1.0;
        }
        1.0 - (self.connections_opened as f64 / self.checkouts as f64)
    }
}
