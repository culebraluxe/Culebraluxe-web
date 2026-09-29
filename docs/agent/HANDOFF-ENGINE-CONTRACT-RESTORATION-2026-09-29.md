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
| S4 | ~~The completion ledger is still process-local in production~~ — **FIXED `ae16ef38`**: `DbCompletionLedger` over `workflow_command_receipt`, and `ForgeRuntime::from_store` now takes the ledger so the memory one is fixture-only | `rust/forge/src/engine/db_ledger.rs`, `rust/core/db/src/forge_engine.rs` (`claim_workflow_receipt` → `WorkflowReceiptClaim`) |
| S5 | The receipt verbs exist in `db`, and the DEV proof passed: claim → `HeldByAnother` → `AlreadyFinal` → stale `pending` reclaimed → finalize-without-claim refused → story counters move | `rust/core/db/tests/forge_completion_receipt_dev.rs` (run with `DATABASE_URL_DEV … -- --ignored`) |
| S6 | `model_policy` and `launch_intent` are carried on the claim and printed, wired to no decision | `rust/forge/src/engine/worker.rs:261-270` |
| S7 | The scheduler is stopped and nothing is in flight | `pnpm forge:doctor` (`open engine tasks: 0`, `active claims: 0`) |
| S8 | Working tree clean, `origin/main` at `ae16ef38` | `git status --short`, `git --no-pager log --oneline -3 origin/main` |

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
| The artifact/verdict funnel (next rail) | §6.1a, legacy `legacy/workflow_app/tests/artifact-verdict.test.ts` | `rust/core/db/src/forge_engine.rs` (new verbs), `rust/forge/src/engine/artifact.rs` (new), `rust/forge/src/qa_consistency.rs` (the polarity vocabulary already exists there) |
| Wire the dispatch model policy | §6.3, legacy `legacy/workflow_app/tests/forge-kind-routing.test.ts` | `rust/forge/src/engine/worker.rs`, `bin/forge.rs`, `engine/opencode.rs` |
| Wire the lead launch cap | §6.4, legacy `legacy/workflow_app/tests/forge-lead-routing-bench.test.ts` | `rust/forge/src/engine/phase.rs` (`RoleEffectPorts`), `engine/agents.rs` |
| Find remaining parity gaps | §6.5, the 465 restored legacy tests | `legacy/workflow_app/tests/**` vs `rust/forge/tests/**` |

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `4ee9d6e2` | Story identity into role tasks (no process-UUID substitution); Story Packet fail-closed (`FORGE_PACKET_FROM_ENV=1` = attended escape); `mark_story_in_progress` and the human-gate hold no longer discarded | `cargo test -p forge` → lib 90 ok, forge_runtime 34 ok |
| `00044bd1` | `RoleHarness::run_role(node, task, self_heal)`; the OpenCode harness appends the corrective directive to the task text | `cargo test -p forge` → lib 90 ok; `--test self_heal_directive` 1 ok (fails on the old code) |
| `d68c9634` | Dispatch envelope read on claim: the poller excludes non-`Unattended OK`; the claim returns the policy; worker **and** engine binary refuse a human-gated dispatch (`FORGE_ATTENDED=1` override); `stop_after` off the row into `--stop-after` and the driver; the claim carries `model_policy`/`launch_intent` | `cargo test -p db -p forge` → db 50 ok, forge 93 ok (3 new), forge_runtime 34 ok, self_heal 1 ok; `cargo check --workspace --all-targets` clean |
| `ae16ef38` | **The completion unit is durable** (`DbCompletionLedger`): `WorkflowReceiptClaim::{Acquired,HeldByAnother,AlreadyFinal}` + stale-`pending` takeover, watermark over finalized receipts only, `read_workflow_receipt_outcome`, story repair/replan counters, `finalize_workflow_receipt` moving `updated_at`; `CompletionLedger` is fallible; `ForgeRuntime::from_store` **takes** the ledger (memory one is fixture-only, `bin/forge.rs` passes the durable one); the reconcile count is carried (`WakeResult::reconciled`, `DriveForgeStoryResult::reconciled`, `reconciled=` in the summary line) | `cargo test -p db -p forge` → db 50 ok, forge 93 ok, `--test durable_completion_ledger` 6 ok; `--test forge_completion_receipt_dev -- --ignored` **1 ok against DEV**; `cargo test -p server -p workflow` → 114 ok; `cargo check --workspace --all-targets` clean |

Raw output behind the last gate (`ae16ef38`):

```
$ cd rust && cargo test -p db -p forge 2>&1 | rg -e '^error' -e 'FAILED'
(no output)
$ cargo test -p db --test forge_completion_receipt_dev -- --ignored
running 1 test
test a_receipt_is_claimed_refused_finalized_and_reclaimed ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.71s
$ cargo check --workspace --all-targets 2>&1 | rg -e '^error' 
(no output: only pre-existing warnings)
```

Raw output behind the last gate (`d68c9634`):

```
$ cd rust && cargo test -p db -p forge 2>&1 | rg -e '^error' -e 'FAILED' -e 'test result: FAILED'
(no output: no errors, no failures)
$ cargo check --workspace --all-targets 2>&1 | rg -e '^error' -e 'Finished|error: could not'
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.26s
```

## 5. NOT VERIFIED — the honest gaps

- **The durable ledger is proven at the row level, not inside a live engine run.** The DEV test drives the verbs
  directly; no engine lane was run (H2), so "a real dispatch re-reads its receipt" is expected, not measured.
- **The whole-funnel DEV proof is partial by design.** `workflow_command_receipt` writes are proven; the
  `forge_workflow_evidence` merge through `DbCompletionLedger::merge_evidence` and the story counters through
  `increment_forge_{repair,replan}_attempts` were exercised by the DEV test's DAO half (counters) but the ledger's
  merge path only by unit test — it needs a story + instance pair on DEV, which is the next DEV proof to write.
- Earlier commits' gaps still stand: the `ForgeAgentWorkRow` shape change is compiled workspace-wide, `pnpm ui:check`
  / `pnpm build` were not run (nothing under `rust/ui` changed), and the self-heal directive's effect on a real model
  turn is unmeasured.
- `cargo fmt --check` is not clean in this repository and was not made clean: pre-existing diffs sit in files this
  work did not touch (`core/db/src/lib.rs`, `core/db/tests/forge_work_claim_dev.rs`, `engine/db_writer.rs`,
  `engine/runner.rs`, `engine/worker.rs`). Everything this commit added or changed is fmt-clean.
- "Everything is fixed" is not true and cannot be claimed from this handoff: §6 is the remaining work.

## 6. OPEN — the next actions, in order

1. ~~**Durable completion ledger (the P0).**~~ **DONE `ae16ef38`** — see §4 and §7.1 of
   `docs/agent/OLD-ENGINE-CONTRACT-RESTORATION.md`. Sub-item fixed in the same change: `claim_workflow_receipt` no
   longer answers `None` for two different facts, and a stale `pending` is reclaimable.
1a. **The artifact/verdict funnel (the next rail, seam 4).** Specification:
   `legacy/workflow_app/tests/artifact-verdict.test.ts` (six assertions; the implementation it tested was deleted with
   `legacy/db/`). The rail: `forge_tool_artifact` (migration 130) has **no Rust reader or writer at all** —
   `rg -n 'forge_tool_artifact' rust/` returns nothing — so a role's verdict, summary and detail have nowhere durable
   to land. The legacy funnel's contract, from the test: read `storyboard_story_run.result_status` for the run; for
   `kind = 'run-verdict'` keep the verdict **only if its polarity agrees with the run's ruling** (by polarity, not
   spelling: `Complete`+`PASS` agree, `Hold`+`Failed` agree, `Complete`+`Hold` does not, an unruled run certifies
   nothing, a failed ruling read fails closed to no verdict) and still insert the row with its summary; for any other
   kind (`qa-assay-evidence`) the verdict is the artifact's own. The vocabulary needed for the polarity comparison
   already exists in Rust — `expected_verdict_for_run_status` / `normalize_qa_verdict_token` in
   `rust/forge/src/qa_consistency.rs` — **but note the difference**: `expected_verdict_for_run_status('Hold')` is
   `None` ("a run that did not complete cleanly certifies nothing"), while the artifact guard must read `Hold` as a
   *negative* ruling that `Failed` agrees with. Do not collapse the two readings; write the polarity function next to
   the artifact funnel and keep the doctor's stricter one. **Open question to settle before wiring** (and the reason
   this rail was not started): the legacy caller is gone with the deleted implementation, so the Rust caller has to
   be chosen — the QA/assay lane's completion (where `assay.rs` sets `evidence.qa_passed` and the work item carries
   `story_run_id`) is the obvious one, and choosing it is a design decision, not a mechanical port.
2. **Then the remaining seams** in Phase-0 order (completion receipt out; hold and verdict out; canonical Story Board
   state writes), each seam-first: find the legacy test, write the Rust refusal test, fix, record the row.
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
