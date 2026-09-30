//! The test taxonomy the harness supports.
//!
//! A contract test declares its level so that a reader knows what it is allowed to touch. The level is not ceremony:
//! L0 and L1 run in `cargo test -p test-harness` with no database and no socket; L2 and L3 need an isolated database;
//! L4 additionally needs concurrency or fault injection. A test whose level and behaviour disagree is the bug this
//! enum makes visible.

/// The five contract-test levels, from pure functions to adversarial composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TestLevel {
    /// L0 Pure — no I/O. Domain rules, reducers, codecs: a function of its arguments.
    L0Pure,
    /// L1 Component — one crate's boundary, with deterministic collaborators substituted for the real world.
    L1Component,
    /// L2 Persistence — the database contract, against an isolated and disposable database.
    L2Persistence,
    /// L3 Composition — the composed application across multiple boundaries/seams.
    L3Composition,
    /// L4 Adversarial — concurrency, injected faults and hostile input, under load.
    L4Adversarial,
}

impl TestLevel {
    /// All five levels, in order.
    pub const ALL: [TestLevel; 5] = [
        TestLevel::L0Pure,
        TestLevel::L1Component,
        TestLevel::L2Persistence,
        TestLevel::L3Composition,
        TestLevel::L4Adversarial,
    ];

    /// The short code (`L0`..`L4`), as a suite name or a filter would carry it.
    pub const fn code(self) -> &'static str {
        match self {
            Self::L0Pure => "L0",
            Self::L1Component => "L1",
            Self::L2Persistence => "L2",
            Self::L3Composition => "L3",
            Self::L4Adversarial => "L4",
        }
    }

    /// The long name (`L0 Pure`..`L4 Adversarial`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::L0Pure => "L0 Pure",
            Self::L1Component => "L1 Component",
            Self::L2Persistence => "L2 Persistence",
            Self::L3Composition => "L3 Composition",
            Self::L4Adversarial => "L4 Adversarial",
        }
    }

    /// Whether a test at this level needs an isolated database (`database::TestDatabase`).
    pub const fn requires_database(self) -> bool {
        matches!(self, Self::L2Persistence | Self::L3Composition)
    }

    /// Whether a test at this level may drive concurrency or inject faults.
    pub const fn allows_adversarial_input(self) -> bool {
        matches!(self, Self::L4Adversarial)
    }
}

impl std::fmt::Display for TestLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_five_levels_are_distinct_and_ordered() {
        assert_eq!(TestLevel::ALL.len(), 5);
        let codes: Vec<_> = TestLevel::ALL.iter().map(|level| level.code()).collect();
        assert_eq!(codes, ["L0", "L1", "L2", "L3", "L4"]);

        let mut sorted = TestLevel::ALL;
        sorted.sort();
        assert_eq!(
            sorted,
            TestLevel::ALL,
            "the enum order is the taxonomy order"
        );
    }

    #[test]
    fn only_the_higher_levels_need_a_database() {
        assert!(!TestLevel::L0Pure.requires_database());
        assert!(!TestLevel::L1Component.requires_database());
        assert!(TestLevel::L2Persistence.requires_database());
        assert!(TestLevel::L3Composition.requires_database());
        assert!(!TestLevel::L4Adversarial.requires_database());
    }
}
