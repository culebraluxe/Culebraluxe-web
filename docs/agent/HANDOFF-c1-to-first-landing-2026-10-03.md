# Handoff — getting ONE story all the way through (ENG-FORGE-C1-BUILD-INFO-01), 2026-10-03

The Captain's instruction this session: *"we are just trying to get one to work fully and properly — the bulk run is
not ready yet, we need to fix every step in the path for that to work."* This file is what is true at the point the
window got long, so the next agent does not re-derive it.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | The layout move `80cfc9da` (2026-10-03, "the tree is three tiers") deleted `rust/`, and the story rows still carry it. **697 of 889** rows have `assay_commands like '%rust/%'`; C1 is the only one fixed. | `cli forge sql --target prod --sql "select count(*) from storyboard_story where assay_commands like '%rust/%'"` |
| S2 | The gate executes a row's commands VERBATIM, so the stale prefix was a guaranteed `CMD_FAIL` before any candidate code ran: `cargo run --manifest-path rust/Cargo.toml …` → `error: manifest path 'rust/Cargo.toml' does not exist`, cargo exit=101. | `forge/src/engine/qa_adjudicate.rs:102-113` |
| S3 | The non-contract acceptance map had **no producer in the product**: the gate read `turn.out.acceptance_mapped`, which `OpenCodeHarness` only ever constructs `false` and nothing sets true (only 3 test doubles do; the QA prompt never asks for a map). No non-contract story has reached PASS since **2026-09-19**; the only PASSes since are RUST_CONTRACT, which reads the other branch. | `forge/src/roles/qa.rs` (fixed), `forge/src/engine/opencode.rs:418,1005`; census: `select coalesce(r.test_mode_snapshot,'(null)'), t.verdict, count(*), max(t.created_at) from forge_tool_artifact t join storyboard_story_run r on r.id=t.story_run_id where t.kind='qa-assay-evidence' group by 1,2` |
| S4 | The map the row CAN prove is the rule the contract path already measures — every assay command appears verbatim in the acceptance-criteria text — and on run `e868570c`'s own snapshot it is TRUE for all four C1 commands. | `forge/src/bin/forge.rs:224`; per-command check against `storyboard_story_run.acceptance_criteria_snapshot` |
| S5 | The scheduler wrapper records its checkout at **install** time; installing from a lane points every tick at a branch its own guard refuses. Install FROM THE MAIN CHECKOUT. | `stop: checkout-not-main branch='lane/deep'` in the invocation log; `docs/agent/LAYOUT.md:163-181`; guard at `scripts/agent-worker-once.sh:196-214` |
| S6 | Run 2 (`913000d4-78a1-4e4a-abb2-89ce7aed1751`, base `f22f976b`) **PASSED and PUBLISHED**: `qa_verify` evidence `verdict=PASS` 23:31:53, run `result_status=Complete` (`commit_hash=38fcd3c5`), `forge_workflow_evidence`: `qa_passed=true`, `publish_succeeded=true`, `published_sha=38fcd3c5`; story row `status=Complete`, `completion=100`, `completed_at=23:32:10`. **This is the first non-contract story to reach PASS and to publish to `main` since 2026-09-19.** | the rows above; `git -C /Users/Shared/dev/src/Culebraluxe-web log --oneline -1 origin/main` → `38fcd3c5 feat(cli): add read-only forge build-info command` |
| S7 | `pnpm forge:doctor` reports `POSTCARD board vs table: DRIFT — this is a bug (board 0 vs table 1)` and `QA CONSISTENCY: qa runs checked: 0`. Both readings disagree with the rows and are unexplained. | `forge/src/doctor_report.rs:88-215` |

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | The other 697 stale story rows | the Captain (bulk run explicitly "not ready") | Fix ONE row at a time, on request. Do not sweep the board. |
| H2 | C1's rows while run 2 is in flight | the engine's claim | Do not edit `storyboard_story` for C1 until the run settles; a board flip mid-run arms a second item. |
| H3 | The production deploy of a landed candidate | the Captain | `scripts/deploy-prod.sh` is his action; publishing to `main` is not deploying. |
| H4 | A push to `main` while a Forge run is in flight | this handoff's author | The publisher pushes from the main checkout; a lane push mid-run can leave it behind. Wait for the run to settle. |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| Understand why a story's gate fails | `forge/src/engine/qa_adjudicate.rs`, `forge/src/engine/assay.rs` | — |
| Understand who decides a verdict | `forge/src/roles/qa.rs` (`read_assay_measurement`), `forge/src/bin/forge.rs:216-240` | — |
| Read a run's truth | the rows: `storyboard_story_run`, `forge_tool_artifact`, `forge_workflow_evidence`, `jobs`, `agent_work_item`, `forge_hold_record` | — |
| Move a story's text | `db/loads/*.sql` (one idempotent load per change; apply with `cli db-tool apply <load> prod`) | the load you add |
| Install/repair the scheduler | `docs/agent/LAYOUT.md:163-181`, `scripts/agent-worker-once.sh` | nothing (install from the MAIN checkout) |

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `85833981` | `db/loads/fix_eng_forge_c1_stale_rust_paths_2026_10_03.sql`: C1's `assay_commands`, `acceptance_criteria`, `scope`, `context_refs` lose the dead `rust/` prefix; board Hold→Ready in the same load. Applied to PROD (recorded in `schema_migration`). | applied clean; row read back clean (`position('rust/' in …)` = 0 in all four fields) |
| `8b6dec4e` | `forge/src/roles/qa.rs`: off-contract acceptance map `= turn.out.acceptance_mapped \|\| ctx.contract_acceptance_mapped`, + test `a_non_contract_story_maps_acceptance_from_the_row` | `cargo fmt -p forge` clean; `cargo test -p forge` → **221 passed, 0 failed**; `qa::` → 3 passed |
| `f22f976b` | `db/loads/rearm_eng_forge_c1_after_acceptance_map_fix_2026_10_03.sql`: board Hold→Ready so the poller arms a run on the fixed gate. Applied to PROD. | applied clean; the trigger minted exactly one item (`23:01:19Z`), claimed `23:03:18Z` |

| Run 2 (`913000d4`, base `f22f976b`) — **not mine, the engine's** | Full chain: `architect` 23:04:34→23:08:49, `lead_pre` →23:10:22, `smith` →23:27:18 (`candidate-code`), `lead_post` →23:28:51, `qa_verify` →23:32:08 | `forge_tool_artifact`: `qa-assay-evidence` `verdict=PASS` 23:31:53, sha `38fcd3c5` |
| **`38fcd3c5` on `origin/main`** | The delivered feature: `cli/src/forge/build_info.rs` (new, 112 lines), `cli/src/forge/mod.rs`, `cli/src/main.rs` — a compile-time `forge build-info` (`--format json`), no env/db/network | `git merge-base --is-ancestor 38fcd3c5 origin/main` → YES; `forge_workflow_evidence.publish_succeeded=true`, `published_sha=38fcd3c5`; story row `status=Complete`, `completion=100` at 23:32:10 |

Run 1 of the night (`e868570c`, base `85833981`) is the receipt that the row fix works: nodes
`architect+repair_smith` completed 22:49:54, `lead_post` 22:51:16, `qa_verify` 22:55:55; candidate
`1f5ada30c393b91531a6b66a28adcbc490566931` ("3 file(s), 8659 patch bytes"); and the gate's own evidence
`QA Unproven: blockers=[ACCEPTANCE_MAP_MISSING] failed=[]` — i.e. **no command failed**. The three `CMD_FAIL`
verdicts of 19:31/19:39/19:43 are the "before".

## 5. NOT VERIFIED — the honest gaps

- **The release/publish step HAS now run once** (`publish_succeeded=true`, `published_sha=38fcd3c5`, landed on `main` at
  23:32:08; `HostReleaseExecutor`, `forge/src/engine/git_publish.rs:289`). What it still has NOT done, for any story:
  require/verify a deployment — `deployment_required`, `deployment_succeeded` and `production_verified` are null, so
  "landed" ≠ "deployed" (H3), and `failed_release_stage` stayed null even on run 1's QA failure.
- The row-text rule has been applied to **one** row. The 697-row rewrite is untested at scale (the survey says one
  shape: `rust/Cargo.toml` in `assay_commands`, 1394 occurrences; `rust/cli/` in scope/refs).
- **A cross-run vendor-session trap is latent, not fixed**: the session is minted in a run's worktree, which the run's
  end deletes, and `forge/src/engine/opencode.rs:519-533` prefers the stored session — resuming one from a deleted
  directory is a vendor `UnexpectedStatus: 500` before the first token (that is what killed the attempts before
  `db/loads/unblock_eng_forge_c1_stale_vendor_session_2026_10_03.sql` cleared it). Run 2 minted a fresh session
  (`ses_efc112929ffe…`) and did not hit it.
- Run rows record **no command counts** (`commands_total/passed/failed` are null on `e868570c` and `2390a5af` although
  the gate ran four commands), and `forge_workflow_evidence.failed_release_stage` stays null on a QA failure — so
  "where did it stop" has to be read from `forge_tool_artifact`.
- The `POSTCARD` drift and `QA CONSISTENCY: qa runs checked: 0` (S7) are unexplained readings, not diagnoses.
- I did not run `cargo check --workspace --all-targets` inside a candidate worktree; the gate does.

## 6. OPEN — the next actions, in order

1. DONE — run 2's verdict was READ, not hoped for: `forge_tool_artifact` `kind='qa-assay-evidence'`
   `verdict='PASS'` at 23:31:53 (`38fcd3c5`), and the release followed it.
2. DONE — the release was followed to the artifact: `forge_workflow_evidence`: `qa_passed=true`,
   `publish_succeeded=true`, `published_sha=38fcd3c5`; `git merge-base --is-ancestor 38fcd3c5 origin/main` → YES.
   **The story that started this session is delivered, published and closed as `Complete`.**
3. Land this handoff and any remaining load files on `main` (deferred: see H4).
4. Only then the 697-row sweep — one idempotent rule: drop `--manifest-path rust/Cargo.toml `, `rust/cli/`→`cli/`.
5. Harden `cli launchd agent-worker install` (`cli/src/launchd/agent_worker.rs:240-262`) to refuse when the checkout
   it is about to point at is not the control-plane checkout on `main`: the run-time guard already exists
   (`scripts/agent-worker-once.sh`), so this only moves the refusal to the moment of the mistake.

## 7. ASK THE OWNER

- Sweep the other 697 rows now, or keep fixing one at a time? (**yes** = one load + one verification pass; **no** = C1 only)
- Harden the install-time guard in the same push? (**yes** = one small commit + test; **no** = leave it in §6.5)
- When a candidate lands, deploy it (the Captain's action) or leave it on `main`? (**deploy** / **leave**)
