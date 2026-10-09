# TST redirect security — 2026-10-08

## Status — publication authorized by owner

Seven final canonical test binaries passed (one test each, zero ignored). Chris explicitly authorized pushing the prepared tree to main on 2026-10-08. The preserved patch is applied on top of current main without replacing peer changes. The prior T1 gate failure remains recorded below; the frozen guard and allowlist are unchanged.

## Scope

TST-SEC-REDIRECT-001, 003, 004, 006, 007, 008 and 010. These seven failed stories had no canonical Rust test files. Added their exact canonical files/functions and SecurityHarness, which delegates to the production Google login/callback policy without duplicating it.

_Update, 2026-10-08 (lane/deep)._ The `SecurityHarness` named above is the **redirect-policy** harness this batch added; the SEC.AUDIT batch had independently given its own, unrelated harness the same name, and the collision briefly left one type answering both questions. They are now named for their jobs — `RedirectPolicyHarness` (L0, this batch's) and `AuditPersistenceHarness` (L2, the audit batch's), both exported from `test_harness`. No behaviour changed: this batch's cases call `RedirectPolicyHarness::redirect_target`, which is the same call to `safe_next`. See `docs/agent/HANDOFF-MERGE-QUEUE-2026-10-08.md` §11.

## Application defect

`web/src/api/google_auth.rs` accepted local paths containing raw CR/LF and other control characters. Axum 0.8 constructs an HTTP 500 response when such a value cannot become a Location header. The policy now falls back to `/portal/dashboard` for control characters. The unchanged policy permits local paths and rejects all absolute URLs, including same-origin absolute URLs. No database schema, service authorization, or MVI changes.

## Evidence

Before the control-character fix, `cargo test --locked -p test-harness --test sec_redirect__010__crlf` exited 101: expected `/portal/dashboard`, received the raw CR/LF path. Seven targeted tests passed after the fix. Final verification uses the repository T0/FMT/T1 gate, including final tests that construct the real Axum response and assert HTTP 303 plus Location.

## Remaining work

TST-SEC-REDIRECT-002, 005, 009 and 011 remain unclaimed; encoded variants, malformed Unicode and redirect loops need their own investigation. Client name-search fixtures assume an empty DEV database; client detail seeds an invalid archived status. Those files are unchanged in this batch. BoldSign remains Deferred. This batch requires no live database test connection or provider credentials.

## Gate environment

The first complete section gate exhausted the 32 GB scratch disk while linking the harness suite (not a test assertion failure). Cleared only this checkout’s generated Cargo target directory and reran the identical gate with `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`; this changes debug symbol size, not test scope. A shell-only mktemp wrapper adapts the repository’s macOS `mktemp -t slice-check` syntax to Linux. No gate code or allowlist was changed.

The reduced-size retry then hit undefined hidden symbols while linking the existing web unit-test binary. Rebuilt the web crate’s generated objects with `cargo clean -p web` and reran the same gate; no production sources or test exclusions were changed to handle this environment issue.

A subsequent parallel build ended before the gate emitted a verdict. Final retry also limits `CARGO_BUILD_JOBS=2` to reduce resource pressure; test scope remains unchanged.

## Final verification and blocker

Final gate receipt: T0 compile PASS (45s), FMT rustfmt PASS (2s), T1 sections FAIL (281s), app-core + harness. CLI ran 158 passing tests and two failures. The history-reader failure was caused by this shallow clone and passed after fetching 100 more commits. The remaining blocker is `forge::repo_guards::tests::no_tree_residue_the_worktree_capability_set_is_frozen`: current main contains `scripts/lane-cargo-config.sh` and `scripts/verify-checkout-build-dir.sh`, which match the scanner but are absent from its frozen capability set. These files and the guard are untouched in this batch. The verification script actually creates probe worktrees, so adding an exception would be a policy decision, not a fixture repair.

AGENTS.md forbids editing a baseline or allow-list to turn a gate green. The next action is to resolve that existing worktree policy mismatch, then apply the preserved patch, rerun the unchanged section gate, publish the application change, and update the seven Neon rows with the successful receipt. BoldSign stays Deferred.

After the gate built all targets, directly executed the seven final canonical test binaries: each returned `test result: ok. 1 passed; 0 failed; 0 ignored` with exit 0. No live database or provider was exercised.

## Publication receipt

Owner authorization: “I authorize you to push your tree to main.” Applied the exact preserved ten-file patch against main at `80e5f7d04c9eba7a8aff35590f52c95cfdc2f723`, checking all existing hunk contexts and confirming new paths are absent. Subsequent verification documented in `docs/agent/HANDOFF-TST-RED-LANDING-2026-10-08.md` reports seven canonical assays plus ten web Google-auth tests passing against `877d1f6f6`. The earlier scratch executor is unavailable in this turn, so no fresh build or test execution is claimed. Publish via the GitHub API with an expected-head lease; no force push, gate edits or deployment. Neon completion notes distinguish the targeted passing evidence from the non-green section gate.
