# Handoff — the engine-run seam (run snapshot, model policy, bench intent, base commit)

The seams are wired, pushed and measured in DEV. The one thing not done is the **single engine run against PROD**, which
is the owner's call to start (§7). This file is the state, not the story.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | A claim opens `storyboard_story_run` with all twelve specification columns copied from `storyboard_story` by the INSERT itself — no caller-supplied snapshot | `rust/core/db/src/forge_engine.rs` `begin_agent_work_run` |
| S2 | The INSERT is the only writer of those columns: `storyboard_story_run` carries **no trigger** | `cli forge sql --target prod --sql "select tgname from pg_trigger where tgrelid='storyboard_story_run'::regclass and not tgisinternal"` → `rows=0` |
| S3 | The twelve source columns exist on `storyboard_story` in DEV and PROD: `goal, preconditions, architect_brief, context_refs, acceptance_criteria, postconditions, dependencies, scope, operating_surface, test_mode, assay_commands, packet_sha` | `forge sql` on `information_schema.columns` → `spec_cols=12` on both targets |
| S4 | `base_commit_hash` is stamped separately, only while NULL | `rust/core/db/src/forge_engine.rs` `stamp_run_base_commit` |
| S5 | PROD queue: `Done` 1183, `Error` 39, `Ready` 8, `Running` 1 — the one Running is stale (item `10defc2a-…`, story `ENG-GUARD-REPO-RUST-01`, `claimed_by=scheduler`, started `14:35:55Z`) | `forge sql --target prod` on `agent_work_item` grouped by `state` |
| S6 | The scheduler is **installed but not loaded** (`running: no`, `disabled: yes`) | `rust/target/debug/cli launchd agent-worker status` |
| S7 | The 10:34 tick reached `pass=1 start` and never logged an end; its claim is the stale `Running` row in S5 | the invocation log named by `launchd agent-worker status` |
| S8 | Seven of the eight queued PROD stories have **no packet file** on disk; the packet the engine loads is the `storyboard_story` row | `ls docs/agent/packets/`; `StoryPacket::load_from_neon` |
| S9 | Working tree clean; `HEAD == origin/main == 4ff9c1de` | `git status --porcelain` → empty; `git --no-pager log --oneline -1 origin/main` |

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | The single engine run against PROD | the Captain (§7 A1) | do not launch it "to see": it claims a PROD row, bills a model lane and settles a story |
| H2 | `pnpm forge:clean` | the Captain | not needed for the run — the worker's own recovery requeues the stale claim first (`recover_stale_agent_work`, 10 min), and `forge:clean` is PROD-mutating |
| H3 | Any other PROD write | the Captain | read-only `forge sql` is free; a write is not |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| Run the engine once | `scripts/agent-worker-once.sh` (the pass loop) and `rust/forge/src/bin/forge_worker.rs` | nothing |
| Read what a run recorded | `rust/cli/src/forge/sql.rs`, `rust/core/db/src/forge_read.rs` | nothing |
| Change what a run snapshots | migration 024 §2, then the insert in `rust/core/db/src/forge_engine.rs` | `rust/core/db/src/forge_engine.rs`, `rust/core/db/tests/forge_work_claim_dev.rs` |
| Prove it without a model lane | `rust/core/db/tests/forge_work_claim_dev.rs` (assertion 5c) | as above |

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `3352673d` | the packet loads before the claim; `begin_agent_work_run` opens the run; model policy and launch intent are read from the row; `stamp_run_base_commit` | `cargo check --workspace --all-targets` |
| `882b8684` | `forge sql` — one read-only query against a database the caller must name | live on DEV and PROD; a `delete from app_error` was refused (exit 2) |
| `4ff9c1de` | the run's specification is copied from the story row inside the INSERT — twelve columns, was four, caller-passed | `cargo test -p db -p server -p forge -p workflow` → 32 binaries ok, 323 passed, 0 failed, `exit=0`; `cargo test -p db --test forge_work_claim_dev -- --ignored` → 2 passed |

## 5. NOT VERIFIED — the honest gaps

- No PROD run row has been opened by this code, so `goal_snapshot` and its eleven siblings on a **written** PROD row,
  `base_commit_hash`, and the run's `policy`/`model_policy` echo remain unobserved. The insert is measured in DEV; PROD is
  measured read-only.
- `model_policy` and `launch_intent` on the eight `Ready` PROD items are NULL: a run on them bills the default
  (`cheap` → flash tier) and carries no Lead cap. The read-from-the-row path is compiled and unit-tested, not observed live.
- PROD `db.connect` timed out once this session (`Timeout during db.connect`, incident `24b21e17-0cdc-4213-9cae-39da27266ace`); the next attempt succeeded. Whether that is what killed the 10:34 tick is not proven.
- The twelve values a PROD claim will write were read with the same select list as the insert (`forge sql` on item
  `59eb30b1-…` → `run_type=dispatch`, goal non-null, `ac_chars=757`, `packet_sha` NULL because the story has no sha). That
  is evidence about the sources, not about a written row.

## 6. OPEN — the next actions, in order

1. Run the engine once, if §7 A1 answers `run`: `APP_ENV=production EXECUTION_ENV=PROD AGENT_WORKER_MAX_PASSES=1 bash scripts/agent-worker-once.sh`
   Finished when `forge sql --target prod` shows a `storyboard_story_run` row for the claimed story with `goal_snapshot is
   not null`, `acceptance_criteria_snapshot is not null`, `run_type` and `execution_environment` set, and
   `agent_work_item.story_run_id` pointing at it.
2. Then check that the same row carries a non-null `base_commit_hash` once provisioning answers, and that
   `forge_tool_artifact` children hang off that `story_run_id`.
3. Decide whether the eight `Ready` stories keep running on defaults, or are dispatched from the Cockpit with a policy.

## 7. ASK THE OWNER

- **A1 — run the single PROD engine run now?** One word: `run` (I launch the one-pass command in §6.1 and report the run
  row), or `hold` (the seams stay landed and unobserved on PROD, and §5 stays as it is).
- **A2 — if `run`, one pass or the whole queue?** `one` (bounded; proves the seam) or `all` (the queue drains; every story
  is a model-billed lane).
