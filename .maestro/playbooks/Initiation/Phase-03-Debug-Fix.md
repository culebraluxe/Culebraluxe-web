# Phase 03: Debug & Fix

This phase runs only when the work card is a `debug` card. It reproduces the failure, proves the root cause with a failing test, fixes it in the layer that owns it, and lands the fix on `main` with a receipt. Following the house rules, failures route through the durable capture framework, role behaviour stays in role-service hooks, no listing is special-cased, and a fix lands as a small conventional commit. For any other card kind every task records `N/A` and ticks, so the playbook keeps moving.

## Tasks

- [ ] Gate on card kind. Read `.maestro/playbooks/Initiation/Working/CARD.md`. If `kind` is not `debug` or `status` is not `open`, append `Phase 03: skipped (kind=<kind>)` to the card's `## Log` section and tick every remaining task in this phase with the note `N/A`. Otherwise append `Phase 03: started` and continue.

- [ ] Reproduce the failure exactly. Run the card's acceptance command (or the narrowest `cargo test -p <crate> <test_name>` that covers it) and save the output to `Working/logs/repro-<date>.log`. If the failure does not reproduce on the synced HEAD, check whether a recent `origin/main` commit already fixed it (`git log origin/main --oneline -20 -- <scoped files>`). If so, strike the inbox line with that sha, set the card to `status: already-fixed`, and tick the remaining tasks as `N/A`.

- [ ] Find the root cause and decide which layer owns the fix. Read the failing path end to end, starting from the innermost frame or assertion. Use `ripwire . --callers=<symbol>` or `--situ` to trace it when it is not obvious. Then record in the card under `## Root cause`: <!-- MAESTRO:MODEL tier="high" effort="high" reason="Root-causing Forge engine or JobService failures means reasoning about durable jobs, retries and settlement across services. A wrong diagnosis produces a fix that passes locally and fails in the next engine run." -->
  - The causal chain, with `file:line` references.
  - The owning layer: engine, role-service hook, product or harness. The fix goes in the owning layer, even when a docs file says to leave buggy code alone, because the docs are often stale.
  - Whether the failure was swallowed anywhere, since a swallowed catch is a defect of its own.
  - Whether the bug is in the code or in a stale test fixture or expectation. Recent history has both kinds, for example `ARCH-SEAM-001`'s fixture artifact.

- [ ] Write the failing test first. Add or tighten a test in the owning crate's existing test module or `tests/` file that reproduces the root cause and fails for that reason. Search for a nearby test that uses the same fixture builders, and copy its pattern before writing new scaffolding. Run it and confirm it is RED for the recorded reason, not for a compile error.

- [ ] Implement the fix in the owning layer only:
  - Keep the change minimal.
  - Route any new error path through the existing capture framework (search for how neighbouring code records failures) rather than logging and continuing.
  - Never special-case a single listing.
  - If the fix needs a schema change, stop here: set `needs_db: true` in the card and note what Phase 05 must apply, then continue with the code half only.

- [ ] Run the gates the change owes:
  - The new test and the card's acceptance command must be GREEN.
  - Run `cargo check --workspace --all-targets`, `cargo clippy -p <touched crates> -- -D warnings`, and `pnpm slice:check` (or `cargo test -p <touched crates>` if that script is missing).
  - If the change touched `forge/` or harness files, also run `pnpm test:harness` and `pnpm forge:packet-lint`.
  - Fix any regressions you introduced. A failure that already existed in the Phase 01 health report is noted in the card, not fixed here.

- [ ] Commit and land on `main`:
  - Stage only the files you changed (never `.env.local` or secrets), and commit in house style, for example `fix(<area>): <what was wrong, stated as the corrected behaviour>`, with a body that explains the root cause in 2–4 lines.
  - Then run `git fetch origin main && git rebase origin/main && git push origin HEAD:main`, and mirror the result with `git push --force-with-lease origin HEAD:lane/claude`.
  - If a hook refuses the push, do not retry it blindly. Read the refusal and fix the cause if it is in your change (re-running the gates), otherwise record it in the card. Record the landed sha in the card's `## Log`.

- [ ] Verify Phase 03:
  - `git log origin/main --oneline -3` shows the fix commit.
  - The acceptance command is GREEN on the landed HEAD.
  - Strike the inbox line in `INBOX.md` and append the sha.
  - Set the card's `status: done`, unless `needs_db: true`, in which case set `status: needs-db`.
