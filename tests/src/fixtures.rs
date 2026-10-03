//! Deterministic fixtures: one object that owns a test's clock, its id stream and the small builders tests repeat.
//!
//! Most of the noise in a contract test is setup: a timestamp, a correlation id, an actor, a unique email, a
//! namespace that no other test is using. `FixtureFactory` owns all of it, seeded, so two runs of the same test
//! build byte-identical fixtures and a third test cannot collide with either.

use chrono::{DateTime, Utc};

use services::{ServiceActor, ServiceActorKind, ServiceContext, ServicePrincipal};

use crate::clock::TestClock;
use crate::ids::DeterministicIds;

/// A seeded source of clock values, identifiers and common test values.
#[derive(Debug, Clone)]
pub struct FixtureFactory {
    clock: TestClock,
    ids: DeterministicIds,
    namespace: String,
}

impl Default for FixtureFactory {
    fn default() -> Self {
        Self::new(1)
    }
}

impl FixtureFactory {
    /// A factory seeded with `seed`, stopped at the Unix epoch.
    pub fn new(seed: u64) -> Self {
        Self {
            clock: TestClock::epoch(),
            ids: DeterministicIds::new(seed),
            namespace: format!("tsth-{seed:016x}"),
        }
    }

    /// A factory with a caller-supplied clock (for a test that needs a specific instant).
    pub fn with_clock(seed: u64, clock: TestClock) -> Self {
        Self {
            clock,
            ids: DeterministicIds::new(seed),
            namespace: format!("tsth-{seed:016x}"),
        }
    }

    /// The clock this factory moves.
    pub fn clock(&self) -> &TestClock {
        &self.clock
    }

    /// The id stream this factory draws from.
    pub fn ids(&self) -> &DeterministicIds {
        &self.ids
    }

    /// A unique, filesystem- and SQL-safe namespace for this factory.
    ///
    /// It is stable for a seed and distinct between seeds, which is what a `CREATE SCHEMA` or a cleanup prefix needs.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// A namespaced value: `<namespace>-<suffix>`.
    pub fn namespaced(&self, suffix: &str) -> String {
        format!("{}-{}", self.namespace, suffix)
    }

    /// The clock's current instant.
    pub fn now(&self) -> DateTime<Utc> {
        self.clock.now()
    }

    /// The clock's current instant in milliseconds.
    pub fn now_millis(&self) -> i64 {
        self.clock.now_millis()
    }

    /// A deterministic id with a prefix.
    pub fn id(&self, prefix: &str) -> String {
        self.ids.id(prefix)
    }

    /// A deterministic UUID string.
    pub fn uuid(&self) -> String {
        self.ids.next_uuid_string()
    }

    /// A deterministic email that cannot reach a real mailbox: the `.test` TLD is reserved and never delivered.
    pub fn email(&self, local: &str) -> String {
        format!("{local}-{}@example.test", self.ids.next_sequence())
    }

    /// A `ServiceContext` for `principal`, correlated with a deterministic id.
    pub fn service_context(&self, principal: ServicePrincipal) -> ServiceContext {
        ServiceContext {
            actor: ServiceActor {
                id: principal.app_user_id.clone().into(),
                kind: ServiceActorKind::User,
            },
            correlation_id: self.id("corr"),
            causation_id: None,
            principal: Some(principal),
        }
    }

    /// A background (system) `ServiceContext`.
    pub fn system_context(&self) -> ServiceContext {
        ServiceContext {
            actor: ServiceActor {
                id: None,
                kind: ServiceActorKind::System,
            },
            correlation_id: self.id("corr"),
            causation_id: None,
            principal: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_builds_identical_fixtures() {
        let left = FixtureFactory::new(5);
        let right = FixtureFactory::new(5);
        assert_eq!(left.namespace(), right.namespace());
        assert_eq!(left.id("person"), right.id("person"));
        assert_eq!(left.uuid(), right.uuid());
        assert_eq!(left.email("buyer"), right.email("buyer"));
        assert_eq!(left.now_millis(), right.now_millis());
    }

    #[test]
    fn ids_and_namespaces_do_not_collide_between_seeds() {
        let left = FixtureFactory::new(1);
        let right = FixtureFactory::new(2);
        assert_ne!(left.namespace(), right.namespace());
        assert_ne!(left.uuid(), right.uuid());
    }

    #[test]
    fn a_fixture_email_can_never_be_delivered() {
        let fixture = FixtureFactory::new(1);
        assert!(fixture.email("guest").ends_with("@example.test"));
    }
}
