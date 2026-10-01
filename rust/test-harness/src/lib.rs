//! The CulebraLuxe Rust contract-test harness.
//!
//! The harness exists to make architectural contracts executable. A contract test built on it exercises the SAME
//! boundary production uses — the abstract service, the MVI `Screen`, DAO/store ownership, the Vault, security and
//! entitlement resolution, the `WorkflowEngine`, and the Forge runtime/worker seams — rather than a re-declared,
//! test-only copy of them. Raw SQL is reserved for tests whose subject *is* the database contract; everywhere else
//! the test goes through the production DAO/repository.
//!
//! # The one-way rule
//!
//! `test-harness` may depend on production crates. Production crates may **not** depend on `test-harness`. That is
//! what keeps the harness a harness: if a production crate linked it, the harness would become part of the system
//! under test, and a test could pass because the harness, not the code, implemented the behaviour. Production crates
//! do not name this crate; only `dev-dependencies` and this crate's own tests do.
//!
//! # The test levels
//!
//! [`TestLevel`] names the taxonomy the harness supports:
//!
//! - **L0 Pure** — domain and reducer functions, no I/O. [`clock`], [`ids`], [`fixtures`], [`mvi`], [`snapshot`].
//! - **L1 Component** — one crate's boundary with deterministic collaborators. [`fault`], [`providers`], [`mvi`].
//! - **L2 Persistence** — the database contract against an isolated, disposable database. [`database`].
//! - **L3 Composition** — the composed application across seams. [`http`], [`actors`], [`barrier`].
//! - **L4 Adversarial** — concurrency, faults and hostile input under load. [`barrier`], [`fault`].
//!
//! # What the harness will not do
//!
//! - It will not connect to the PRODUCTION database. [`database::guard_target`] refuses it, and the self-tests prove
//!   the refusal, before any socket is opened.
//! - It will not call a live external provider by default. [`providers`] ships deterministic fakes only.
//! - It will not become a second production implementation. Helpers wrap production types; where a helper's subject
//!   is production code, the helper hands that code the values it needs and reads back what it did.

#![forbid(unsafe_code)]

pub mod actors;
pub mod barrier;
pub mod clock;
pub mod database;
pub mod engine;
pub mod fault;
pub mod fixtures;
pub mod forge;
pub mod git;
pub mod http;
pub mod ids;
pub mod level;
pub mod mvi;
pub mod pool;
pub mod providers;
pub mod runtime;
pub mod snapshot;
pub mod source;

pub use clock::TestClock;
pub use database::{
    guard_target, resolve_test_target, HarnessDbError, TestDatabase, TestTransaction,
};
pub use engine::EngineHarness;
pub use fixtures::FixtureFactory;
pub use forge::ForgeHarness;
pub use ids::DeterministicIds;
pub use level::TestLevel;
pub use pool::{DbPoolFaultHarness, PoolFault};
pub use runtime::RuntimeHarness;
