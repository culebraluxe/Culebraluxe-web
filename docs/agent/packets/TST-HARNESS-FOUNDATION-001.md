# TST-HARNESS-FOUNDATION-001 — Build the Rust contract test harness

## Goal

`rust/test-harness` is the dedicated workspace crate the taxonomy-driven Rust contract tests fence on. It makes the
production boundaries — Abstract Service, MVI `Screen`, DAO/store ownership, the Vault, Security/Entitlement, the
`WorkflowEngine`, and the Forge runtime/worker seams — executable in a test instead of re-declaring them. The legacy
TypeScript test estate is not a conversion target.

## Scope

In: the crate, its modules, and its self-tests.

Out: porting any TypeScript test; any live external provider call; any PRODUCTION database connection; any production
crate depending on the harness.

## Architect brief

Raw SQL is reserved for tests whose subject *is* the database contract; everywhere else the test goes through the
production DAO/repository. Dependency direction is one-way: `test-harness` may depend on production crates; production
crates may not depend on it. That is what keeps the harness a harness — if a production crate linked it, a contract
test could pass because the harness, not the code, implemented the behaviour.

## Context refs

- `rust/Cargo.toml:3-15` — the workspace member list, `test-harness` named.
- `rust/test-harness/Cargo.toml:1-33` — every dependency is a boundary under test.
- `rust/test-harness/src/lib.rs:1-56` — the one-way rule and the L0–L4 taxonomy.
- `rust/test-harness/src/database.rs:68-75` — `guard_target`, the pure PRODUCTION refusal.
- `rust/test-harness/tests/harness_self_test.rs:1-271` — the foundation self-tests.

## Acceptance criteria

1. `rust/test-harness` is a workspace member named `test-harness`. — met.
2. It provides deterministic clock/ID/fixture helpers, isolated database support, concurrency barriers, Axum request
   helpers, role/entitlement actors, disposable Git repositories/worktrees, deterministic fault injection, fake
   external-provider adapters, MVI helpers, and structural snapshot helpers. — met by `clock`, `ids`, `fixtures`,
   `database`, `barrier`, `http`, `actors`, `git`, `fault`, `providers`, `mvi`, `snapshot`, `engine`.
3. The harness exercises production APIs/boundaries; it must not become an alternative production implementation and
   production crates must not depend on it. — met; enforced by the `no_production_crate_depends_on_the_harness`
   self-test, not only documented.
4. Database helpers refuse PROD targets and own cleanup for disposable test data/databases. — met; `guard_target`
   refuses before a socket, transactions are rollback-only, isolated schemas drop `CASCADE`.
5. External-provider helpers never call live providers by default. — met; `providers` ships fakes only.
6. Harness self-tests prove deterministic IDs/clock and prove the PROD database guard refuses execution. — met,
   including the async constructor path.
7. The harness supports L0 Pure, L1 Component, L2 Persistence, L3 Composition, and L4 Adversarial tests. — met;
   `level::TestLevel`.
8. `cargo test --manifest-path rust/Cargo.toml -p test-harness` passes. — met.
9. `cargo check --manifest-path rust/Cargo.toml --workspace --all-targets` passes. — met.

## Preconditions

Rust workspace builds. No database, network or environment is required for the self-tests.

## Postconditions

A contract test can be written against a production boundary and run with `cargo test -p test-harness`.

## Skills

workflow
ui

## Loop

intent: build
loop: 1/1

## Test mode

SCOPED

## Assay commands

- cargo test --manifest-path rust/Cargo.toml -p test-harness
- cargo check --manifest-path rust/Cargo.toml --workspace --all-targets

## Verification (2026-09-29)

Landed by `5844cb00` (the crate and its modules), `9b012a6c` (self-tests classified and run), `519d0b08` (the one-way
rule enforced), `9c1b2980` (NO-TREES exception owned; per-instance DB isolation), `833ba76f` (the PROD guard proven on
the connect path). These commits are local only; this node's brief says do not push, so nothing was pushed from here.

- `cargo test --manifest-path rust/Cargo.toml -p test-harness` → **57 passed, 0 failed** (47 unit + 10 self-tests).
- `cargo check --manifest-path rust/Cargo.toml --workspace --all-targets` → **exit 0**.
