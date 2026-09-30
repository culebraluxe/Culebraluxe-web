//! Deterministic identifiers for tests.
//!
//! A test fixture needs identities (a row id, a correlation id, a request id) that are stable across runs, so a
//! failure can be reproduced from the log and a snapshot does not churn. `DeterministicIds` produces that: a seeded
//! stream where the same seed and the same step always yield the same value, and no value reaches for the OS random
//! source.
//!
//! The UUIDs it produces are valid RFC-4122-shaped v4 strings — version and variant bits are set — so they can be
//! written to a Neon `uuid` column in a persistence test. They are NOT cryptographically random and must never be
//! used for a secret; they are fixtures.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use uuid::Uuid;

/// A seeded, deterministic stream of identifiers.
#[derive(Debug, Clone)]
pub struct DeterministicIds {
    seed: u64,
    next: Arc<AtomicU64>,
}

impl DeterministicIds {
    /// A stream that yields its first value first. Two instances with the same seed yield the same sequence.
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            next: Arc::new(AtomicU64::new(0)),
        }
    }

    /// A stream that starts counting at `start`. Useful to continue a fixture's numbering.
    pub fn starting_at(seed: u64, start: u64) -> Self {
        Self {
            seed,
            next: Arc::new(AtomicU64::new(start)),
        }
    }

    /// The seed this stream was created with.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// How many values have been handed out.
    pub fn sequence(&self) -> u64 {
        self.next.load(Ordering::SeqCst)
    }

    /// The next counter value (1-based), advancing the stream.
    pub fn next_sequence(&self) -> u64 {
        self.next.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// The next deterministic value.
    pub fn next_u64(&self) -> u64 {
        mix(self.seed, self.next_sequence())
    }

    /// A deterministic, valid v4 UUID.
    pub fn next_uuid(&self) -> Uuid {
        uuid_from_mix(self.next_u64(), mix(self.seed ^ 0x5DEECE66D, self.sequence()))
    }

    /// A deterministic v4 UUID, formatted.
    pub fn next_uuid_string(&self) -> String {
        self.next_uuid().to_string()
    }

    /// A readable deterministic id with a prefix: `person-00000001`.
    pub fn id(&self, prefix: &str) -> String {
        format!("{}-{:08}", prefix, self.next_sequence())
    }
}

/// A splitmix64-style mix of a seed and a counter: deterministic, well-distributed, and cheap.
fn mix(seed: u64, counter: u64) -> u64 {
    let mut z = seed
        .wrapping_add(0x9E37_79B9_7F4A_7C15)
        .wrapping_mul(counter.wrapping_add(1));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Two mixed words become sixteen bytes; the version and variant bits make it a valid random-shaped UUID.
fn uuid_from_mix(high: u64, low: u64) -> Uuid {
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&high.to_be_bytes());
    bytes[8..].copy_from_slice(&low.to_be_bytes());
    // Version 4 (random) and the RFC-4122 variant, exactly as `workflow::ids::uuid_v4` sets them.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_yields_the_same_sequence() {
        let left = DeterministicIds::new(7);
        let right = DeterministicIds::new(7);
        for _ in 0..32 {
            assert_eq!(left.next_u64(), right.next_u64());
            assert_eq!(left.next_uuid_string(), right.next_uuid_string());
            assert_eq!(left.id("row"), right.id("row"));
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let left = DeterministicIds::new(1);
        let right = DeterministicIds::new(2);
        assert_ne!(left.next_u64(), right.next_u64());
        assert_ne!(left.next_uuid_string(), right.next_uuid_string());
    }

    #[test]
    fn uuids_are_valid_v4_shaped() {
        let ids = DeterministicIds::new(99);
        for _ in 0..64 {
            let text = ids.next_uuid_string();
            let parsed = Uuid::parse_str(&text).expect("a deterministic id must parse as a UUID");
            assert_eq!(parsed.get_version_num(), 4);
            assert_eq!(parsed.get_variant(), uuid::Variant::RFC4122);
        }
    }

    #[test]
    fn ids_are_readable_and_ordered() {
        let ids = DeterministicIds::new(3);
        assert_eq!(ids.id("person"), "person-00000001");
        assert_eq!(ids.id("person"), "person-00000002");
        assert_eq!(ids.sequence(), 2);
    }
}
