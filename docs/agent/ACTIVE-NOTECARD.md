# ACTIVE NOTECARD — Harness Evolution / Phase 3 closure + CTO Office

**Status:** ACTIVE. This is the single source of truth for what is next. Any agent session may
pick this up: do the first OPEN item, update the row, run the gate, and move on.

**Cycle protocol**
1. Read this file top-to-bottom. Pick the first item with `status: OPEN` (respect `depends:`).
2. Do the work with the named owner/scope; do not reopen closed items without a new defect.
3. When done: set `status: DONE (commit <sha>)`, append one line to "Done log", run
   `cargo test -p forge --lib && cargo test -p db --lib && pnpm forge:harness`.
4. If a new defect is found, add it below with `status: OPEN` and a file:line reference.
5. Stop when every item is DONE/BLOCKED. Never invent work; never change this file's scope
   without the principal.
6. CTO Office items live at `/Users/Shared/dev/src/cto-office/` and are independent of the
   Culebraluxe gate; update their own files.

## A. CTO Office (personal) — first

| # | Item | status | notes |
| --- | --- | --- | --- |
| C1 | Phase 1 foundation: constitution, principal, authority, memory index, hot context, decisions, backlog | DONE | `/Users/Shared/dev/src/cto-office/` written 2026-10-18 |
| C2 | Principal answers: money threshold, high-stakes comms, first real skill | BLOCKED (principal) | see docs/agent/PRINCIPAL-DECISIONS.md §2 | asked in `memory/waiting_on_me.md`; conservative defaults in force |
| C3 | Durable backup target for CTO Office memory (git remote/encrypted) | DONE (local repo) | local git repo initialized + initial commit; off-machine/encrypted mirror still needs principal choice | survival requirement |
| C4 | Phase 3 runtime: `/start-session` + `/end-session` hooks, env, delta, alerts | BLOCKED (C2) | after C2 |
| C5 | Phase 4: first real skill with triggers/NOT FOR/tests | BLOCKED (C2) | after C2 |

## B. Hardening packet (Forge) — medium QA defects

| # | Item | status | scope |
| --- | --- | --- | --- |
| H1 | Sandbox: argument allowlist, process-group kill, capped streaming capture, no swallowed read errors | DONE (2026-10-18) | `forge/src/engine/harness_evolution/sandbox_command.rs` |
| H2 | Ledger→feed evidence adapter: expose `evidence_refs` on repeat rows; DB-level repeat-group test | DONE (2026-10-18) | `db/src/forge_corrections.rs`, `feed.rs` |
| H3 | Writeback honesty: confidence `Option` (never 0), no self-referential evidence, reject rationale field | DONE (2026-10-18) | `feed.rs`, matching `db` row |
| H4 | `correction_key` contract: comment/Rust preimage agree; store text verbatim; 295 in `REQUIRED_BOOT_MIGRATIONS` | DONE (2026-10-18) | migration 295, `db/src/forge_corrections.rs`, `db/src/boot_gate.rs` |
| H5 | Dogfood real-run boundary: `--authorize-sandbox` precondition, fail on partial loader, purity test | DONE (2026-10-18) | `dogfood.rs`, `scripts/harness-dogfood.sh` |
| H6 | Calibration: blended-Brier label, declared-by provenance, no double-count | DONE (2026-10-18) | `calibration.rs` |

## C. Operator surface

| # | Item | status | scope |
| --- | --- | --- | --- |
| O1 | CLI `forge harness-evolution feed|metrics|calibration` (read-only, `--format json`) | DONE (2026-10-18) | `cli/src/forge/harness_evolution.rs` |
| O2 | `forge harness-evolution probe` entrypoint that records probe/contrast rows through the durable store | DONE (2026-10-18) | CLI + `store_db.rs`/`probe_replay.rs` |
| O3 | Digest lines: corrections awaiting root cause, calibration status, active experiment | DONE (2026-10-18) | `scripts/orchestrate-status.py` |

## D. First real experiment

| # | Item | status | scope |
| --- | --- | --- | --- |
| D1 | Run dogfood: 3–5 historical Rust stories, prompt/tool-guidance only, k=1, command sandbox first | DONE (DSH) | first real run: experiment `probe-…1792353093608471000`, 2 DSH turns, elapsed 166s→68s, no regressions; tokens/cost NOT CAPTURED; 1 story only. OpenCode deferred to TD (2026-10-18 incident). See docs/agent/DOGFOOD-2026-10-18-DSH.md | wiring + command-sandbox E2E on DEV verified (persisted experiment/candidate/2 probes/contrast, cleaned); paid model run needs principal approval: `--authorize-model-run --model-backend opencode` = 2 opencode turns | `scripts/harness-dogfood.sh` + probe entrypoint |
| D2 | Review evidence manually before any adoption; write first experience/correction rows | DONE | reviewed the DSH run in docs/agent/DOGFOOD-2026-10-18-DSH.md; weak single-task signal, no adoption, no writeback; adoption OFF | DB + decision record |
| D3 | Automatic adoption stays OFF until effective-GREEN has production evidence | BLOCKED | blocked by D1/D2 + principal approval |

## E. Later (do not start until A–D empty)

| # | Item | status |
| --- | --- | --- |
| L1 | DSH per-seat web presets + skill-invocation capture | BLOCKED (A-D not empty) |
| L2 | Model-role linkage metrics feeding routing | BLOCKED (A-D not empty) |
| L3 | Correction→specification automation + regression tests | BLOCKED (A-D not empty) |
| L4 | Earned autonomy gates (evidence-only, never self-granted) | BLOCKED |
| L5 | Freshness/routing/cross-model audits | BLOCKED (A-D not empty) |

## Done log

- 2026-10-18: C1 CTO Office Phase 1 foundation written (outside Culebraluxe; no gate).
- 2026-10-18: H0/H0b initial Harness Evolution closure landed (`3e5dfa54`); QA verdict PASS WITH
  DEFECTS.
- 2026-10-18: H1-H6 hardening packet fixed (sandbox arg allowlist/process-group/bounded streaming,
  ledger->feed evidence adapter, writeback honesty, correction-key contract + boot gate,
  dogfood authorization/partial-load refusal, calibration provenance/label) — 1024 forge tests,
  63 db tests, gate PASS.
- 2026-10-18: OpenCode attempts failed pre-inference (provider.no-route); switched dogfood to DSH. First real DSH run: 2 turns, candidate matched base and was faster (166s→68s), no regressions, tokens/cost NOT CAPTURED, 1 story; no adoption. OpenCode deferred to TD. D1/D2 DONE.
- 2026-10-18: principal decision packet written (docs/agent/PRINCIPAL-DECISIONS.md); all remaining items blocked on principal approvals; goal stopped for input.
- 2026-10-18: D1 probe CLI wired for model turns (mutually exclusive with --command); command-sandbox dogfood E2E on DEV persisted and cleaned; model-turn dry-run verified without spend. Paid opencode run requires principal approval.
- 2026-10-18: D1 model-turn sandbox adapter landed (ModelTurnSandbox + ModelTurnAuthorization, DSH/OpenCode backends, fake-transport tests; no live model call). Remaining D1: wire into probe CLI and run.
- 2026-10-18: C3 CTO Office local git repo initialized + initial commit (off-machine mirror pending principal).
- 2026-10-18: O1-O3 operator surface landed: CLI feed|metrics|calibration + probe entrypoint
  (persists experiment/candidate/probes/contrast on DEV, cleans up), digest corrections/experiment/
  calibration lines; CLI suite fixed (stale opencode config test) — 1030 forge, 63 db, 181 cli, gate PASS.
