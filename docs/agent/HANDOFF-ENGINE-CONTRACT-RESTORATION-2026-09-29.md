# Handoff — old-engine contract restoration — 2026-09-29

Program doc: `docs/agent/OLD-ENGINE-CONTRACT-RESTORATION.md` (contracts, seam-first method, order of work).
Invariant in force: **the schema is the contract.** Forge data is disposable; the engine's tests are not precious;
rails move only toward more enforcement; no schema change.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | Seven seams were inventoried, ten mask sites named, seven rails found with no execution reader | `docs/agent/OLD-ENGINE-CONTRACT-RESTORATION.md`, Phase 0 |
| S2 | Four rails have execution readers now: story identity into role tasks, authoritative Story Packet, canonical Story Board writes, `execution_policy` + `stop_after` from the claimed row | `rust/forge/src/engine/runtime.rs:404-431`, `rust/forge/src/bin/forge.rs:136-200`, `rust/core/db/src/forge_engine.rs:347-380` |
| S3 | The self-heal retry carries its directive | `rust/forge/src/engine/runner.rs:30-41`, `rust/forge/tests/self_heal_directive.rs` |
| S4 | The completion ledger is still process-local in production: `MemoryLedger::new()` at `ForgeRuntime` construction, `with_ledger()` has no production caller | `rust/forge/src/engine/runtime.rs:145`, `:91` |
| S5 | The durable ledger's storage is already decided by the schema and unused: `READ_EVIDENCE`, `INC_REPAIR`, `INC_REPLAN`, `STORY_LEDGER` in `neon_sql.rs` have no Rust caller | `rust/forge/src/engine/neon_sql.rs:22-49`; `rg -n 'INC_REPAIR\|READ_EVIDENCE' rust/` |
| S6 | `model_policy` and `launch_intent` are carried on the claim and printed, wired to no decision | `rust/forge/src/engine/worker.rs:261-270` |
| S7 | The scheduler is stopped and nothing is in flight | `pnpm forge:doctor` (`open engine tasks: 0`, `active claims: 0`) |
| S8 | Working tree clean, `origin/main` at `d68c9634` | `git status --short`, `git --no-pager log --oneline -3 origin/main` |

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | Scheduler stays stopped | the Captain | Do not run `pnpm agent:scheduler:install` until he says `restart scheduler` |
| H2 | Engine runs stay on PROD (`dev` is free read-only) | `docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md` §0 | Never run `pnpm forge:clean` or an engine lane without his explicit go |
| H3 | Which model bills (`model_policy`) | the Captain | Do not pick it; the legacy table and the Rust pin disagree (§6.3) |
| H4 | `launch_intent` semantics (benchIntent) | the Captain | Do not invent a cap check; the legacy tests are the specification (§6.4) |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| Finish the durable completion ledger | §6.1, `rust/forge/src/engine/completion.rs`, legacy `legacy/workflow_app/tests/interrupted-sequences.test.ts` | `rust/core/db/src/forge_engine.rs`, `rust/forge/src/engine/completion.rs`, `runtime.rs:91,145` |
| Wire the dispatch model policy | §6.3, legacy `legacy/workflow_app/tests/forge-kind-routing.test.ts` | `rust/forge/src/engine/worker.rs`, `bin/forge.rs`, `engine/opencode.rs` |
| Wire the lead launch cap | §6.4, legacy `legacy/workflow_app/tests/forge-lead-routing-bench.test.ts` | `rust/forge/src/engine/phase.rs` (`RoleEffectPorts`), `engine/agents.rs` |
| Find remaining parity gaps | §6.5, the 465 restored legacy tests | `legacy/workflow_app/tests/**` vs `rust/forge/tests/**` |

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `4ee9d6e2` | Story identity into role tasks (no process-UUID substitution); Story Packet fail-closed (`FORGE_PACKET_FROM_ENV=1` = attended escape); `mark_story_in_progress` and the human-gate hold no longer discarded | `cargo test -p forge` → lib 90 ok, forge_runtime 34 ok |
| `00044bd1` | `RoleHarness::run_role(node, task, self_heal)`; the OpenCode harness appends the corrective directive to the task text | `cargo test -p forge` → lib 90 ok; `--test self_heal_directive` 1 ok (fails on the old code) |
| `d68c9634` | Dispatch envelope read on claim: the poller excludes non-`Unattended OK`; the claim returns the policy; worker **and** engine binary refuse a human-gated dispatch (`FORGE_ATTENDED=1` override); `stop_after` off the row into `--stop-after` and the driver; the claim carries `model_policy`/`launch_intent` | `cargo test -p db -p forge` → db 50 ok, forge 93 ok (3 new), forge_runtime 34 ok, self_heal 1 ok; `cargo check --workspace --all-targets` clean |

Raw output behind the last gate (`d68c9634`):

```
$ cd rust && cargo test -p db -p forge 2>&1 | rg -e '^error' -e 'FAILED' -e 'test result: FAILED'
(no output: no errors, no failures)
$ cargo check --workspace --all-targets 2>&1 | rg -e '^error' -e 'Finished|error: could not'
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.26s
```

## 5. NOT VERIFIED — the honest gaps

- **No SQL changed in this handoff has touched a real database.** `cargo test -p db -p forge` runs unit tests plus
  DEV-gated tests that are `ignored` without `DATABASE_URL_DEV`. The changed statements are `claim_next_agent_work`
  (new `and w.execution_policy='Unattended OK'` predicate, four new returning columns), `claim_specific_agent_work`
  (four new returning columns) and `begin_agent_work_run` (`returning execution_policy`, `fetch_optional` instead of
  `execute`). Unit tests cannot catch a column typo.
- The `ForgeAgentWorkRow` shape change is compiled workspace-wide, but only exercised by `db` unit tests and the
  DEV-gated `forge_work_claim_dev.rs` (edited to the new API, `ignored` here).
- `pnpm ui:check` / `pnpm build` were not run: nothing under `rust/ui` changed in the three commits.
- The self-heal directive is verified to reach `run_role`; that the CLI turns it into a better second turn is not
  measured (it needs a real model call).
- "Everything is fixed" is not true and cannot be claimed from this handoff: §6 is the remaining work.

## 6. OPEN — the next actions, in order

1. **Durable completion ledger (the P0).** `ForgeRuntime` defaults to `MemoryLedger`, so a per-dispatch process starts
   with a blank receipt memory, while the legacy engine made the post-transition unit durable (transition → merge
   evidence → finalize receipt; absence of the receipt *is* the crash window —
   `legacy/workflow_app/tests/interrupted-sequences.test.ts`). The storage is already in the schema; do **not** add a
   table: `workflow_command_receipt` (claim-first `pending` → `success`, prefix `forge.completion:`),
   `storyboard_story.forge_repair_attempts` / `forge_replan_attempts` (INC_REPAIR / INC_REPLAN) and
   `forge_workflow_evidence` (merge read + `ForgeEngineDao::merge_workflow_evidence`). Needed: a `DbCompletionLedger`
   implementing `CompletionLedger` (`claim` = "we inserted the pending row"; `has_final` = outcome ≠ `pending`;
   `finalize` = update to `success`), DAO reads for the evidence row and the story ledger counters, and the wiring at
   `runtime.rs:145` with `MemoryLedger` kept for in-memory tests. **Fix first, in the same change:**
   `ForgeEngineDao::claim_workflow_receipt` (`rust/core/db/src/forge_engine.rs:1210-1241`) returns `Ok(None)` when
   *it inserted*, returns `Some(row)` for an existing final receipt, and filters `pending` out of what it returns — so
   a caller cannot tell "I claimed it" from "another process holds it". Reconcile must also treat a stale `pending`
   (older than the engine's own 15-minute claim window) as reclaimable, or a crash mid-unit strands its task forever.
   Done when a DEV test proves (a) two racers apply the unit once, (b) a crash between transition and merge is
   completed by the resume, (c) a second resume merges nothing.
2. **Then the remaining seams** in Phase-0 order (artifact out of a role into a row; completion receipt out; hold and
   verdict out; canonical Story Board state writes), each seam-first: find the legacy test, write the Rust refusal
   test, fix, record the row.
3. **`model_policy` → the model** (P1, blocked on H3). The legacy table is
   `legacy/workflow_app/tests/forge-kind-routing.test.ts`: exactly two policies, `cheap` and `judgment`, both naming
   `deepseek/deepseek-v4-flash`, unknown/null reading as `cheap`. The Rust pin is `deepseek/deepseek-flash`
   (`rust/forge/src/engine/opencode.rs:25`), so wiring the table **changes which model runs and what bills**.
4. **`launch_intent` → the Lead's cap** (P1, blocked on H4). Migration 167 says it rides the role-effect ports as
   `benchIntent` where the Lead's cap check enforces it; `RoleEffectPorts` (`rust/forge/src/engine/phase.rs:42`) has no
   such field, so the column is read for nothing but the log line. Specification:
   `legacy/workflow_app/tests/forge-lead-routing-bench.test.ts` (`benchIntent = 'HOLD' | 'SOLO'`).
5. **Phase 1 parity audit** (the inventory count is a floor). Walk the 465 restored legacy tests under
   `legacy/workflow_app/tests/` and decide per contract: ported (name the Rust test) / missing (open a seam) /
   obsolete (say why). Start with the three files this handoff already used as specification:
   `interrupted-sequences.test.ts`, `forge-kind-routing.test.ts`, `forge-lead-routing-bench.test.ts`.

## 7. ASK THE OWNER

- `restart scheduler` — resumes the 180s poller; nothing is in flight and the queue is clean, so it is safe at any
  time. Anything else leaves it stopped.
- `cap the model policy` or `leave the model policy` — on the first, wire the legacy policy table and name the model
  each policy should use in Rust; on the second, §6.3 stays open and the column stays a log line.
- `port the bench intent` or `hold the bench intent` — on the first, §6.4 lands with the legacy bench tests ported.
- `keep the packet fail-closed` (the new default: an unreadable Story Packet abandons the run and requeues the claim)
  or `attended packets only` — the second means an operator run that wants the environment packet must start with
  `FORGE_PACKET_FROM_ENV=1`.
