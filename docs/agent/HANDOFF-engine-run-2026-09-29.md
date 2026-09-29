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
| S9 | Working tree clean; `HEAD == origin/main` (`4ff9c1de`, then this handoff's own commit) | `git status --porcelain` → empty; `git --no-pager log --oneline -1 origin/main` |
| S10 | **The one engine run the Captain authorized started `2026-09-29T20:33:20Z`**: item → `Running`, run `5a60ad40-8cca-43f4-859a-d0347a098fc9` for `ENG-GUARD-AGENTS-LINT-01`, opened with `run_type=dispatch`, `execution_environment=PROD`, `goal_snapshot` 255 chars, `acceptance_criteria_snapshot` 1065 chars, `scope_snapshot` the full story scope | `forge sql --target prod` on `storyboard_story_run` for `5a60ad40-…` |
| S11 | The run's model policy came from the row: the lane runs `opencode run --model deepseek/deepseek-flash` (`model_policy` NULL → `cheap` default), and the board shows the story `In Progress` | `pgrep -fl 'opencode run'`; `storyboard_story.status` |

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

- The run the Captain authorized (S10) has written a PROD run row and the twelve columns are filled on it, but it has not
  reached a terminal state: `base_commit_hash` is still **unstamped** ~35 minutes in and `result_status` is unruled, so the
  `stamp_run_base_commit` step of this seam is unobserved. If that run ends unstamped, the stamp is not reached on this
  path — check whether the lane provisioned a worktree at all (`stamp_run_base_commit`'s only source).
- `model_policy` and `launch_intent` on the eight `Ready` PROD items are NULL: a run on them bills the default
  (`cheap` → flash tier) and carries no Lead cap. The read-from-the-row path is compiled and unit-tested, not observed live.
- PROD `db.connect` timed out once this session (`Timeout during db.connect`, incident `24b21e17-0cdc-4213-9cae-39da27266ace`); the next attempt succeeded. Whether that is what killed the 10:34 tick is not proven.
- The twelve values a PROD claim will write were read with the same select list as the insert (`forge sql` on item
  `59eb30b1-…` → `run_type=dispatch`, goal non-null, `ac_chars=757`, `packet_sha` NULL because the story has no sha). That
  is evidence about the sources, not about a written row.

## 6. OPEN — the next actions, in order

1. **The run answered `A1 = run`, one pass, and it is in flight** (S10). Watch it to a terminal state — do not start a
   second lane while this one holds the item:
   - `forge sql --target prod --sql "select state, attempts from agent_work_item where story_run_id='5a60ad40-8cca-43f4-859a-d0347a098fc9'::uuid"`
   - `forge sql --target prod --sql "select result_status, base_commit_hash from storyboard_story_run where id='5a60ad40-8cca-43f4-859a-d0347a098fc9'::uuid"`
   Finished when the item leaves `Running` and the run row carries a `result_status` (`Complete`/`Failed`) or stays unruled
   because the claim was **cleared** — the second is a legal outcome, not a defect.
2. Then check that the same row carries a non-null `base_commit_hash` once provisioning answers, and that
   `forge_tool_artifact` children hang off that `story_run_id`.
3. Decide whether the eight `Ready` stories keep running on defaults, or are dispatched from the Cockpit with a policy.

## 7. ASK THE OWNER

- **A1 — answered: `run`, one pass.** The lane for `ENG-GUARD-AGENTS-LINT-01` is the one run; nothing else is started.
- **A2 — answered: `one`.** The other seven `Ready` items wait for a separate decision (§6.3).
