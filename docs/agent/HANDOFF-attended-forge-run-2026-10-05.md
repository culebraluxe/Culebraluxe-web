# Handoff — the attended Forge run of `TST-HARNESS-FOUNDATION-001` (2026-10-05)

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | The story **ran across Forge on the opencode adapter with `deepseek/deepseek-v4-flash`** and executed two real model turns (`architect`, `lead_pre`) before the engine died: `No valid transition from decision node execution_shape`, `engine_exit=1` at `14:11:30`. | `/tmp/forge-tst-retry3.log`; boot lines `harness_backend=OpenCode model=deepseek/deepseek-v4-flash`, `opencode-harness turn node=architect … tokens_in=107293` |
| S2 | `deepseek/deepseek-v4-flash` exists and answers today; the pin's own comment says it does not. | `$HOME/.opencode/bin/opencode models \| grep deepseek` lists it; `opencode run --model deepseek/deepseek-v4-flash 'Reply with exactly: ok'` → `ok`; the stale note is `forge/src/engine/opencode.rs:27-32` (pin itself: `deepseek/deepseek-flash`, line 33) |
| S3 | **The engine binary is not the CLI.** `forge` reads `DATABASE_URL_PROD` from the process environment only (and `.env.local` quotes must be stripped); it never loads `.env.local`. | `forge/src/engine/vendor_session.rs:46-52`; the working recipe is `scripts/agent-worker-once.sh:125-141` |
| S4 | Without `FORGE_PROVISION=1` the run works **in the invoking checkout**; with it, in a disposable worktree. | `forge/src/bin/forge.rs:374`; boot line `worktree …/T/culebraluxe-forge-worktrees/tst-harness-foundation-001-run-…` |
| S5 | The story is left **`In Progress` / 0%** with an **active token at `lead_pre`** and an active instance — it cannot be re-driven until that state is cleared. | `forge sql --target prod --sql "select status from storyboard_story where id='TST-HARNESS-FOUNDATION-001'"`; `process_instances.business_key='TST-HARNESS-FOUNDATION-001'`; `tokens.node_id='lead_pre'` |
| S6 | The scheduler is restored **and points at the main checkout** (it was briefly mis-pointed at `lane-deep`, which made every tick stop `checkout-not-main`). | `plutil -p ~/Library/LaunchAgents/com.culebraluxe.agent-worker.plist` → `AGENT_WORKER_REPO = /Users/Shared/dev/src/Culebraluxe-web`; a manual pass prints `forge-worker target=prod … no work` |
| S7 | The story's packet is stale against `main`: every context ref (`rust/test-harness/…`) is gone, folded into `tests/` by `ea990923`. | `git cat-file -e origin/main:rust/test-harness/Cargo.toml` fails; `git log --diff-filter=D -- rust/test-harness` → `ea990923`; `tests/src/{clock,actors,barrier,database,fault,fixtures,git,engine}.rs` exist |

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | Clearing the story's engine state (`pnpm forge:clean`, `pnpm forge:story:reset`) | the Captain | `forge:clean` is production-mutating by design and needs his explicit word; do not run it as cleanup reflex |
| H2 | Re-running the story to completion | the Captain | a re-run reaches the same `execution_shape` transition and dies the same way until that is fixed — fix first (OPEN 1), run second |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| Run one story attended, on a named adapter and model | the recipe below | nothing |
| Diagnose the lead-decision death | the lead node's transitions + the SPLIT lane | `forge/src/engine/definition.rs`, `forge/src/engine/executor/dispatch.rs`; `docs/agent/MEMORY.md` (2026-09-10, SPLIT dark) |
| Check what a run actually did | the rows, never a log | `storyboard_story_run` (`agent_runtime`, `model_used`), `process_instances`, `tokens`, `jobs`, `forge_engine_task_execution` |

**The attended-run recipe (this machine, PROD only):**

```sh
cd <lane>
# DATABASE_URL_PROD from .env.local with surrounding quotes stripped — scripts/agent-worker-once.sh:125-141
APP_ENV=production EXECUTION_ENV=PROD FORGE_PROVISION=1 \
FORGE_HARNESS=opencode OPENCODE_MODEL=deepseek/deepseek-v4-flash \
  /Users/Shared/dev/build/rust/debug/forge --story <STORY-ID>
```

Three traps, each measured today:

1. A bare `cargo run -p forge --bin forge -- --story <ID>` dies twice over: `DATABASE_URL_PROD is not configured`
   (nothing loads `.env.local`), and a quote-wrapped value is reported as `invalid database connection URL`.
2. Without `FORGE_PROVISION=1` the run executes **in the checkout you launched it from** with `publish=on` — do not
   launch it in a lane that holds another agent's uncommitted files.
3. A killed run leaves a durable job whose `locked_until` gates the next attempt: `drive.rs:151` recovers stale jobs,
   so until that lease expires a retry exits `steps=[]` with `engine_exit=0` — indistinguishable from "nothing to do".

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| this file (`git log -1 -- docs/agent/HANDOFF-attended-forge-run-2026-10-05.md`) | the run recipe, the three traps, the defect, the left state | `pnpm forge:packet-lint` → 0 failures |

**No product code was written and no candidate was published.** The run wrote only workflow rows to PROD (§1 S5).

## 5. NOT VERIFIED — the honest gaps

- Whether the lead's `execution_shape` value was the SPLIT fork (wired-but-dark without `FORGE_SPLIT_ENABLED`). The
  error names the *node*, and no row records the chosen shape.
- Whether a fixed run reaches `smith` → QA → publish. Nothing past `lead_pre` was observed.
- `cost_usd=0.000000` on both turns while `tokens_in/out` were captured (matches the known "widgets on 0" gap).
- The engine's fatal error left **no `app_error` row** (0 rows in a 40-minute window) — capture of engine faults is unproven.

## 6. OPEN — the next actions, in order

1. **Fix the lead-decision transition.** Reproduce with §3's recipe; the death is `No valid transition from decision
   node execution_shape` after the `lead_pre` turn. Finished when a run passes `lead_pre`.
2. **Clear the stale token** (§1 S5) and re-run — production-mutating, so it waits on H1.
3. **Capture engine faults durably** so a dead run leaves a row and not only an exit code.

## 7. ASK THE OWNER

- `TST-HARNESS-FOUNDATION-001` is left `In Progress` with a stale token: **clean and re-run, or fix the transition first?**
- May I run `pnpm forge:clean` (production-mutating, needs `--force`) to clear that one story's state, or do you want a
  story-scoped `forge reset` instead?
