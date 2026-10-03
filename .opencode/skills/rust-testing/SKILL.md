---
name: rust-testing
description: Use when adding, running or fixing Rust tests - nextest in CI, cargo test locally, the format gate and the deferred trees
---

# Skill: rust-testing

- CI runs `cargo nextest run --workspace --profile ci`. Locally `cargo test -p <crate>` is the same tests with a
  friendlier reporter; `cargo nextest` only helps if it is installed.
- Database-touching suites are `#[ignore]`d and run explicitly, single-threaded:
  `cargo test -p web --test service_harness_dev -- --ignored --test-threads=1`. An ignored test that nothing
  invokes is a test nobody runs.
- Formatting is a gate, not a preference: `cargo fmt --all -- --check`. Read the deferral list in `gates.yml`
  before "fixing" a formatting failure — four module trees are deliberately deferred, and reformatting them buries
  the real diff.
- Write the test that fails for the reason the change is wrong. A test asserting a detail the bug cannot reach
  proves nothing and still costs a run.
- A test needing the network, a database or a vendor CLI belongs behind `#[ignore]`, named in its doc comment, not
  quietly skipped.
- Prefer one assertion that names the contract over five that restate the implementation.

## Anchored to
- `.github/workflows/gates.yml` — the runner, the profile, the ignored-suite invocations, the deferred trees.
