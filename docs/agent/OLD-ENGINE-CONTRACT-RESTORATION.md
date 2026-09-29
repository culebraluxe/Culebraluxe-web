# Old-engine contract restoration — every behavior and contract, no shortcuts

**Authority:** the Captain, 2026-09-29: *"I want every behavior and contract of the old engine back! no shortcuts!"*
and *"the database contracts enforced an explicit structured hand off; the agents' output was not well formed and
it wandered — these are not scars, these are guard rails."*

**What that means, as a rule this program is held to:** where the old engine made the **database** the enforcement
point — a `NOT NULL`, a foreign key, a `CHECK`, a column that must be filled before a handoff counts — the Rust
engine must **satisfy the rail**, never route around it. A placeholder that exists so a struct compiles
(`story_id: String::new()`), a convenient default (`MemoryLedger::new()`), a discarded `Result` (`let _ =` on a
canonical write), or a fallback to process environment when the authoritative row is unreadable are all
**stepping around a guard rail**. They are the failures this program exists to remove.

## 0. The finding that shapes the program: the rails are still installed

Read from the database on 2026-09-29 (DEV target, `information_schema`, read-only):

```
TABLES: agent_work_item, calendar_intake_receipt, forge_dispatch_score, forge_engine_task_execution,
        forge_hold_record, forge_migration_ledger, forge_open_holds, forge_story_run_receipt,
        relationship_follow_up_receipt, signature_envelope_recipient, workflow_command_receipt

agent_work_item COLUMNS (35): id, story_id, state, priority, queued_at, claimed_at, claimed_by, started_at,
        finished_at, story_run_id, error_text, created_at, updated_at, role, model_profile,
        special_instructions, runtime_adapter, external_run_id, attempts, max_attempts, execution_policy,
        execution_environment, lane, run_phase, player_id, provider_id, model_id, harness_id, field_id,
        parallel_group_id, parallel_slot, parallel_size, split_assignment, candidate_shas, stop_after,
        launch_intent, kind, model_policy, learn_pattern_key
```

**Consequence: restoration is code obeying the existing schema, not new schema.** No new table and no new column is
expected for any item below. If the audit proves a rail the old engine relied on is genuinely missing from the
database, that becomes an explicit Captain decision in its own right — named, with the rail it restores — and never
a silent side effect of a code fix.

## 0.1 The invariant, as corrected by the Captain (2026-09-29)

*"I don't care about any of the data in production with context to Forge or existing engine tests — they are not
really tied to any business transaction and I own the Forge, so it's OK to delete the data there when I say.
When I say protect production I mean the schema — the schema is the contract requirement and critical to the
hand off."*

Held as five rules from here on:

1. **The schema is the contract.** It is the specification the engine must satisfy. It is not an implementation
   detail, and it is never a side effect of engine code. "Does it conform to the schema" is the acceptance question
   for a seam, not "does the run appear to work".
2. **Forge data is disposable.** The Captain owns it and it is not business data. Engine rows may be deleted or
   rewritten on his word: `agent_work_item`, `forge_hold_record`, `forge_open_holds`, `forge_engine_task_execution`,
   `forge_story_run_receipt`, `workflow_command_receipt`, `storyboard_story_run`, `forge_dispatch_score`. That
   permission is scoped to the Forge engine's own tables — `property`, `media`, `property_media`, buyers and every
   other business table are untouched by it, absolutely and always.
3. **The engine's existing tests are not precious.** They assert the port's own shape, not the rails. They are
   rewritten to assert refusal, or dropped. The 465 restored legacy tests are **reference**, not scripture: they say
   what the old engine enforced.
4. **The schema moves in one direction only: toward more enforcement.** A rail may be **restored** or **added** when
   a contract requires it. No rail is ever relaxed, dropped, made nullable, or given a default to accommodate engine
   code. Every move is a named rail, with the contract it enforces, on the Captain's word.
5. **A fix that would need a weaker schema is not a fix.** If conforming appears to require relaxing a rail, stop and
   report it — that is either a missing rail being discovered or the engine's shape being wrong.

What this unlocks, and why it matters more than it sounds: because Forge data may be deleted and the schema is the
rail, the acceptance test for a seam can be a **real write that the database must refuse**. A hold with no story id
is not merely a Rust unit test — it is an `INSERT` the foreign key rejects, and the test can assert both the
engine's refusal and the row's absence. That is the strength the legacy engine got from its schema and the port
never tested.

## 1. Contract inventory

`old` = what the legacy engine enforced · `now` = what the Rust engine does today · `rail` = where the old engine
made it enforceable.

| # | Contract | old | now | rail |
| --- | --- | --- | --- | --- |
| 1 | Durable completion receipt / repair-replan ledger | post-transition completion unit persisted; `reconcile_completions()` could answer "did this task already complete" after a restart | **RESTORED 2026-09-29 (`DbCompletionLedger`)**: the unit is claimed and finalized in `workflow_command_receipt` (`forge.completion:{taskId}`), evidence merges into `forge_workflow_evidence`, the counters move on `storyboard_story`, and the runtime will not build without a ledger named at the call site — the process-local `MemoryLedger` is reachable only from test fixtures. The defect it replaced: `MemoryLedger` as the production default, so each per-dispatch process re-applied every completion in the instance history | `workflow_command_receipt`, `forge_workflow_evidence`, `forge_engine_task_execution`, `forge_story_run_receipt` (all exist) |
| 2 | Story identity into every role task | runner resolved `process_instance.subject_id → storyboard_story.id` before any write | `story_id: String::new()`, substituted with the process-instance UUID at three write sites | `forge_hold_record.story_id → storyboard_story(id)`; ids are human keys |
| 3 | Durable dispatch envelope reaching execution | `AgentWorkItem` carried role, model_profile, special_instructions, execution_policy, execution_environment, kind, model_policy, stop_after, launch_intent, runtime_adapter … and the child ran with them | queue object reads 6 fields; child launched with `--story --work-type --work-item`; model chosen by environment (`OpenCodeHarness::from_env()`); `special_instructions: None` | `agent_work_item` columns (35 exist) |
| 4 | Story Packet is authoritative; unreadable means no run | could not resolve the Story Board command/context → fail, no agent turn | `StoryPacket::load_from_neon` error → `eprintln!` + environment packet, run proceeds | Story Board rows are the authority |
| 5 | Self-heal supplies the corrective instruction | retry named what was missing (`FORGE_ARCHITECT_HANDOFF`) | `_directive` computed and dropped; retry is the same prompt again | role deliverable set |
| 6 | Canonical state writes are never discarded | failed Story Board write was surfaced | `let _ = mark_story_in_progress(...)`, `let _ = mark_story_human_hold(...)`, `let _ = open_forge_hold_record(...)` | `storyboard_story`, `forge_hold_record` |
| 7 | **Unknown count** — every other behavior the legacy suite asserts | — | — | — |

Item 7 is the honest part: six is what has been found so far. The only way to know the number is the parity audit in
§3, and no report may claim the list is complete until that audit is finished.


## 2. Method — the legacy suite is the spec, and it is ported seam-first

`3718bc83` restored all **465 legacy TypeScript test files** to `legacy/` byte-identical. They are the executable
statement of the old engine's contracts and they are the standard this program is measured against. The port that
produced these six gaps failed because it was done **function-first**: the legacy suite was mapped onto new Rust
functions, and a unit test of a function the author just wrote asserts the author's own understanding — a test of
`map_role_task` passes happily with `story_id: String::new()`.

So every restoration goes **seam-first**:

1. Find the legacy test that asserts the **contract at the seam** (identity, handoff, receipt, refusal), not the one
   that asserts an internal function's shape.
2. Read the legacy production code that satisfied it, for the rail it was satisfying.
3. Write the Rust test from the **legacy assertion**, before the Rust fix.
4. Make the Rust fix, and remove the placeholder / default / `let _ =` / env fallback that stood in for it.
5. Record the row in the parity inventory: contract, rail, legacy test file, Rust test, status.

**Definition of done for a contract**: a Rust test exists that fails if the seam is re-cut, and the enforcing rail is
named in the record. A fix with no such test is not a restoration — it is another placeholder.

## 3. Parity audit (in progress, no shortcuts)

Goal: replace the "six" in section 1 item 7 with a number that can be defended. For each of the 465 legacy test
files: what contract does it assert, is that contract in force in the Rust engine, and is it pinned by a Rust test.
Output is an inventory row per contract, not a patch. The six above are the known rows. This audit precedes code for
anything not already diagnosed, and it continues after the six are closed, because the whole point is that the
port's failure was invisible rather than localized.

Evidence for the table sweep and the column list is the two `information_schema` reads recorded in section 0
(DEV target, read-only, no rows written).

## 4. Order of work

Smallest risk is not the order here; **what decides whether an unattended agent may touch this repository** is.

1. **Contract 4 — packet fail-closed.** Cannot read the authoritative Story Packet means abandon, requeue, and no
   model turn. Highest safety per line, and it is a `match` arm, not a design.
2. **Contract 6 — stop discarding canonical writes.** `mark_story_in_progress`, `mark_story_human_hold` and
   `open_forge_hold_record` must fail the lane. Diagnostic observer writes may stay best-effort, and the code must
   say which is which.
3. **Contract 5 — the self-heal directive reaches the retry.** The directive is already computed; it must be
   supplied to `harness.run_role()`. Without this the retry budget manufactures a second copy of the same failure.
4. **Contract 3 — the durable envelope reaches execution.** Read the persisted row's `model_policy`,
   `model_profile`, `special_instructions`, `stop_after` and `launch_intent`, and give them to the child, so the
   factory policy (cheap to cheap model, judgment to judgment model) is in force rather than decorative.
5. **Contract 1 — the durable completion ledger.** Back `reconcile_completions()` receipts, watermark, merged
   evidence, repair count and replan count with the existing receipt tables so they survive the per-dispatch process.
6. **Contract 2 — story identity.** Fixed and awaiting the Captain's keep-or-drop decision on the uncommitted patch.
7. **Contract 7 — the audit's findings**, in the order the audit ranks them.

## 5. Phase 0 inventory (evidence, no code)

Root cause and scope agreed by the Captain 2026-09-29. Everything below is read from the tree as it stands.

### 6.1 The seams

Seven, not six, and each one is a place where an agent's output must either conform to the interface or the run
stops: (1) Story Packet in; (2) dispatch envelope in, on claim; (3) story identity into every role task; (4) artifact
out of a role into a row; (5) completion receipt out, durable across the per-dispatch process; (6) hold and verdict
out; (7) canonical Story Board state writes.

### 6.2 Rails that no execution code reads

Searched `rust/forge/src`, `rust/server/src`, `rust/core/db/src` for each column:

| column | rail in the schema | readers found | what the readers are |
| --- | --- | --- | --- |
| `execution_policy` | **NOT NULL**, CHECK `Unattended OK / Daytime Only / Human Gate / Manual Only` | **none** | — the Human Gate rail has no reader anywhere |
| `model_profile` | legacy envelope field | **none** | — |
| `model_policy` | CHECK `cheap / judgment` | 2 files | `core/db/forge_doctor.rs`, `core/db/forge_read.rs` — reporting only, never execution |
| `launch_intent` | CHECK `SOLO / SMITH / SPLIT / HOLD`, DB comment "Carried to the Lead as benchIntent" | 1 file | `server/src/tech.rs` — the **writer** (Cockpit `set_dispatch_options`); no engine reader |
| `stop_after` | CHECK `scout / architect / lead`, DB comment "read by the engine worker when it claims the item" | type only | `engine/executor.rs:250`, defaulted `None` at `:266`; no read of the column |
| `execution_environment` | CHECK `DEV / PROD / TEST / LOCAL` | 1 file | `core/db/tech.rs` — inside a `json_build_object` for the cockpit, reporting |
| `special_instructions` | legacy envelope field | 1 file | `engine/packet.rs:9` type, `:40` hardcoded `None`, `:106` read — from the environment packet, not the work item |

The pattern is exact: **the rails are read for display and not for execution.** The doctor can report a dispatch's
model policy while the engine ignores it; the Cockpit writes `launch_intent` and nothing carries it to the Lead.

### 6.3 Mask sites on write or decision paths (the sweep)

Distinction the guard must encode: `let _ = f()?;` propagates the error and discards an uninteresting value — allowed.
`let _ = f();` discards the **failure** — that is the violation. On that rule:

1. ~~`engine/runtime.rs:145` — `ledger: Arc::new(MemoryLedger::new())` as the production default.~~
   **FIXED 2026-09-29.** `ForgeRuntime::from_store` now takes the ledger as a parameter, so the memory ledger is
   only reachable from the `in_memory*` fixtures; `bin/forge.rs` passes `durable_completion_ledger()`, fenced by
   `rust/forge/tests/durable_completion_ledger.rs::the_engine_binary_installs_the_durable_ledger`.
2. ~~`engine/process.rs:24`, `engine/process.rs:47`, `engine/executor.rs:289` — `let _ = self.reconcile_completions(..)?;`
   the reconcile result (how many completions were reconciled) is discarded at all three call sites.~~
   **FIXED 2026-09-29.** The count is now carried: `WakeResult::reconciled` and `DriveForgeStoryResult::reconciled`,
   reported by the engine binary as `reconciled=…`. It was never *just* a diagnostic while the ledger was
   process-local — a non-zero count on every dispatch was the double-application itself.
3. `bin/forge.rs:156` — `OpenCodeHarness::from_env()`: the model is chosen from the process environment while the
   rail-selected policy sits unread in the claimed row.
4. `engine/packet.rs:40` — `special_instructions: None`.
5. `engine/architect.rs:78`, `:84` — `capture_string(slice, "baseRef")` / `"summary"` `.unwrap_or_default()`: a
   provider response missing a required field becomes an empty string instead of a refusal. This is the
   "output was not well formed and it wandered" case, unguarded.
6. `engine/decisions.rs:46`, `:60` — `String::new()` / `unwrap_or_default()` on the role-decision path.
7. `engine/hold.rs:42` — `originating_node.unwrap_or_default()` inside hold handling.
8. `bin/forge_task.rs:46`, `:48` — `let _ = eng.seed_definition(def)` twice: a failed canonical definition write is
   discarded.
9. `server/src/api/engine.rs:116` — `let _ = reply_tx.send(..)`: a lost engine reply is silent. `:350`
   `.unwrap_or_default()` needs its rail named.
10. Benign and to be left alone (buffer accumulators and test-shaped values, not rails): `engine/xml.rs:114,161,204`,
    `engine/architect.rs:123`, `engine/workspace_id.rs:4`, `core/workflow/src/json_codec.rs:10,126`, `roi.rs`,
    `sync_conflict.rs:77`, `doctor_report.rs:240-244`, `opencode_client.rs:79-89`.

### 6.4 Also swept, lower urgency (workflow engine proper)

`core/workflow/src/json_codec.rs:386-387` (`id`, `node_type` via `unwrap_or_default()` while parsing a workflow
definition), `handle_join.rs:40,63`, `execute_node_leave.rs:210`, `fire_timer_job.rs:162,231,264`. These are the
workflow crate's own seams and belong in the inventory, but they are not the Forge handoff path.

### 6.5 What Phase 0 does not claim

The count is now **seven seams, ten mask sites on execution paths, seven rails with no execution reader**. This is a
floor and it is bounded by the two sweeps in 6.2 and 6.3, run over three crates. It is not yet a claim that the
count is complete: the authoritative completion is a per-contract row (contract, rail, legacy test, Rust refusal
test, status), which is Phase 1 work, and the audit of the 465 legacy tests adds rows the sweeps cannot see.

### 6.6 Rails with no writer at all (swept 2026-09-29, during the ledger work)

Two categories, and the difference is the answer to "what do we do about it":

| site | the fact | disposition |
| --- | --- | --- |
| `rust/forge/src/engine/observer.rs:13` `INSERT_OBSERVER_SQL` — "never used" (compiler warning) | the Flight Recorder trace. The legacy engine wrote a diagnostic observer row per step; the Rust port never does | **OPEN WORK, not a decision.** It is contract 7 (the audit's findings) and it needs a seam of its own — the same shape as the seven in §6.1: a writer for a rail that has a table. Nothing here needs the Captain |
| `rust/forge/src/engine/neon_sql.rs` — nine SQL constants, no caller | a second COPY of statements `db::ForgeEngineDao` already owns (receipt claim/finalize/read/watermark, evidence read, story ledger, repair/replan increments, packet read) | **CLOSED 2026-09-29: deleted, not wired.** Wiring them would give `forge_workflow_evidence` and `storyboard_story` two writers each, which AGENTS.md:172 forbids; and the column-writer audit counted this file as a writer of `storyboard_story` while the writer that runs is `forge_engine.rs`. Kept: `RECEIPT_PREFIX`, the one literal with no SQL body. The bodies are in git history (`git show ae16ef38:…neon_sql.rs`) |
| `rust/forge/src/engine/evidence_store.rs` `merge_forge_workflow_evidence` — no caller | a second door onto `forge_workflow_evidence` | **CLOSED 2026-09-29: deleted.** The merge runs through `db::ForgeEngineDao::merge_workflow_evidence`, driven by the completion ledger; the one mapping (`evidence_patch`) stays |

### 6.7 The dispatch rule had two writers (found 2026-09-29, closed the same day)

One rule owns the queue, and the database is its writer: `agent_work_item_dispatch()` inserts exactly one work item
for a story that BECOMES `Ready`, scores it with `story_priority_score()` and arbitrates the conflict against the
partial unique index (`db/migrations/025_agent_work_queue.sql:101`, restated in
`146_fix_storyboard_ready_dispatch_arbiter.sql:36`). The port had not honoured that in two places:

| site | the fact | disposition |
| --- | --- | --- |
| `rust/core/db/src/forge_engine.rs` (the sweep's repair, `insert into agent_work_item` at `:694` before this change) | the stranded-story repair spelled the trigger's rule out again in Rust — the same insert, the same `story_priority_score()` call, the same arbiter predicate typed out a second time. A rule with two spellings drifts: 146 exists only because the arbiter was not restated when 143 replaced the index underneath it, and 258 was written to repair rows a writer that was not the trigger had left behind | **CLOSED: the insert is deleted.** The repair restores the CHANGE the trigger fires on (off `Ready` and back, one transaction) and the row is the trigger's |
| `rust/server/src/tech.rs` (the board's ENGINE RUN Q move, `:348-361` before this change) | `set status='Ready'` plus a note that *claimed* an item had been queued. On a story already `Ready` — which is what a bench round trip leaves, because moving to the bench cancels the item and keeps the status — a same-value update fires no trigger, so the card moved, the human was told it was queued, and nothing was dispatched | **CLOSED: the note is read back, not assumed.** One verb, `ensure_story_dispatched`, returns `Queued { item }` / `AlreadyQueued { item }` / `Missing`, and the `captureCommit` scoping path (`:258-271`) uses it too, so "scope this dispatch" can no longer fail for a reason that is really "there is no dispatch" |
| `rust/core/db/src/forge_control.rs:168-173` (stale-claim requeue), `rust/core/db/src/forge_reset.rs:325` (claim recovery) | they move an EXISTING item back to `Ready` | **KEPT, deliberately, and the line is written down**: the database owns *which items exist* for a story; the engine owns *the state of a claim it holds*. Neither creates a row, so there is no rule for the trigger to own, and `requeue_stale_work` moves the story half in the same transaction, so the pair still moves together |

## 6. Blocking decisions (Captain)

1. ~~**Keep or drop** the uncommitted contract 2 patch~~ — **RESOLVED 2026-09-29**: landed on `main` as `4ee9d6e2`
   (story identity into role tasks, fail-closed Story Packet, canonical Story Board writes un-swallowed).
2. ~~**Ledger storage shape** for contract 1~~ — **RESOLVED 2026-09-29**: the completion unit lives in
   `workflow_command_receipt` under `forge.completion:{taskId}` (already the claim-first receipt table), with the
   evidence in `forge_workflow_evidence` and the counters on `storyboard_story`. **No schema change**: every column
   already existed and was unused.
3. **The dispatch scheduler is stopped**, as a precondition for touching engine code, and stays stopped until the
   Captain says restart. **Still stopped.**

## 7. Restoration log (appended as rails land)

### 7.1 Contract 1 — the durable completion ledger (2026-09-29)

Landed files:

- `rust/core/db/src/forge_engine.rs` — the receipt verbs. `claim_workflow_receipt` now answers
  `WorkflowReceiptClaim::{Acquired, HeldByAnother, AlreadyFinal}` instead of `Option<row>` (where `None` had meant
  both "you own it" and "someone else does", and a `pending` row was filtered out of the answer);
  `read_workflow_receipt_outcome`, `receipt_watermark_ms` (finalized receipts only — a claim must not advance the
  watermark), `increment_forge_repair_attempts`, `increment_forge_replan_attempts` and a `finalize_workflow_receipt`
  that moves `updated_at` and refuses to finalize a row that does not exist.
- `rust/forge/src/engine/db_ledger.rs` (new) — `DbCompletionLedger` + `durable_completion_ledger()`.
- `rust/forge/src/engine/completion.rs` — `CompletionLedger` is fallible (`workflow::Result`): a database that
  cannot answer must stop the caller, because `false` from `claim` means "already applied".
- `rust/forge/src/engine/runtime.rs` — `from_store` **takes** the ledger; the memory ledger survives only in the
  `in_memory*` fixtures. `reconcile_completions` propagates every ledger failure.
- `rust/forge/src/engine/process.rs`, `rust/forge/src/engine/executor.rs`, `rust/forge/src/bin/forge.rs` — the
  reconcile count is carried (`WakeResult::reconciled`, `DriveForgeStoryResult::reconciled`, `reconciled=` in the
  engine's summary line) and the binary installs the durable ledger.

Legacy spec: `legacy/workflow_app/tests/interrupted-sequences.test.ts` (transition durable, evidence unwritten,
receipt absent, resume applies once). Rust refusal tests: `rust/forge/tests/durable_completion_ledger.rs` (6 tests)
and `rust/core/db/tests/forge_completion_receipt_dev.rs` (DEV, `--ignored`: claim → refused-in-flight → no watermark
while pending → finalize → `AlreadyFinal` → watermark advances → stale `pending` reclaimed → finalize-without-claim
refused → counters move, missing story refused).

Not yet landed, named rather than implied: `model_policy` → model selection (billing: Captain's call) and
`launch_intent` → Lead bench intent.

### 7.2 Hygiene — the second copy of a statement, removed (2026-09-29)

Not a rail: the removal of two dead second doors, so the one-writer rule (§6.6) is what the audit reports.

- `rust/forge/src/engine/neon_sql.rs` — nine SQL constants deleted (receipt claim/finalize/read/watermark, evidence
  read, story ledger, repair/replan increments, packet read). Every one was a never-executed copy of a statement
  `db::ForgeEngineDao` owns; `RECEIPT_PREFIX` stays. This also removed the file from the column-writer audit's writer
  set for `storyboard_story`, where it had been counted while the writer that runs is `forge_engine.rs`.
- `rust/cli/src/forge/repo_guards.rs` — `TABLE_WRITERS_BASELINE` narrowed deliberately, in this commit, with the reason
  in the code: the fence must name the writer that serves.
- `rust/forge/src/engine/evidence_store.rs` — the unused `merge_forge_workflow_evidence` wrapper deleted; the merge has
  one door (`ForgeEngineDao::merge_workflow_evidence`, driven by the ledger) and one mapping (`evidence_patch`).
- `docs/agent/MAP-engine.md` — the "how the engine talks to Neon" row now points at `db_ledger.rs`, which is where the
  engine's database writes actually leave from.

Gates: `cargo test -p cli` (125 tests, the column-writer fence among them — it failed first and is what caught a
comment that spelled the statement out), `cargo test -p db -p forge` (db 50, forge 93, durable_completion_ledger 6,
forge_runtime 34, self_heal 1), `cargo check --workspace --all-targets` clean.

### 7.3 Contract 2 (dispatch) — the database owns the rule, and the repair restores the CHANGE (2026-09-29)

The Captain's word was "collapse it": the port stops writing the row the database's function writes. Closed with a
refusal rail first, then the change, then the proof on DEV (§6.7 has the finding).

- `rust/cli/src/forge/repo_guards.rs` — a fourth repo guard,
  `the_database_owns_dispatch_no_production_rust_file_inserts_a_work_item`: no production Rust file may contain
  `insert into agent_work_item`, because that rule has one writer and it is `agent_work_item_dispatch()`. It scans
  582 tracked production Rust files and exempts `tests/` deliberately (a fixture may build a shape the database would
  never create — `forge_work_claim_dev.rs` builds a `Ready` item for an `In Progress` story to prove dispatch refuses
  it) and itself (a guard names the token it hunts). It **failed first**, naming `rust/core/db/src/forge_engine.rs`
  as the second owner; that failure is the evidence this rail can see the thing it forbids.
- `rust/core/db/src/forge_engine.rs` — `dispatch_story_in` (module scope) is now the only place a story is put into
  the engine's queue: it locks the story, and if no open slot exists it restores the change into `Ready` that the
  trigger fires on — off `Ready` and back, as two statements in the caller's transaction, because a data-modifying
  CTE shares one snapshot and could not see its own update. It then **reads back the row the trigger wrote** and
  returns it; no slot there is a `SchemaMismatch` (a missing trigger is a schema disagreement, and it is captured,
  not shrugged). `ensure_story_dispatched_on` opens the transaction; `ForgeEngineDao::ensure_story_dispatched` and
  `TechCockpitDao::ensure_story_dispatched` are two doors onto it. `EnsureDispatch::{Queued, AlreadyQueued, Missing}`
  is what a caller's note to a human is built from.
- `rust/core/db/src/forge_engine.rs` — `reconcile_dispatch_queue` finds the stranded `Ready` stories (a read) and
  hands each to `dispatch_story_in`, the same writer the board uses. `queued` counts stories the database dispatched.
  The sweep no longer contains a second spelling of the score call or the arbiter predicate.
- `rust/server/src/tech.rs` — the ENGINE RUN Q move and the `captureCommit` scoping path go through the verb; the
  note is chosen from its answer, so "queued a work item" is only said when the database produced one.

Legacy spec: `legacy/workflow_app/tests/agent-work.test.ts:28-30` states the contract in the shape this change
restores — "the dispatch trigger behavior (status INTO Ready creates one Ready work item)". Rust tests:
`rust/core/db/tests/forge_dispatch_trigger_dev.rs` (DEV, `--ignored`, 2 tests) — the trigger writes and scores the
item (priority 80 = `story_priority_score('High')`), the stranded story is repaired to exactly one slot with the
database's score, the story the board sees stays `Ready`, a second sweep and a manual off/on cycle add nothing, a
benched-but-`Ready` story handed to the engine gets a real slot and the same id on a second call, and an absent story
reads `Missing`.

Raw DEV output:

```
dispatch trigger: state=Ready priority=80 (score('High')=80)
reconcile_dispatch_queue: queued=1 restated=0 cleared=0
ensure_story_dispatched (benched, still Ready): Queued { item: "435780f3-…" }
ensure_story_dispatched (already queued): AlreadyQueued { item: "435780f3-…" }
ensure_story_dispatched (absent story): Missing
test result: ok. 2 passed; 0 failed; 0 ignored
```

Gates: `cargo test -p cli` 127 passed (the four repo guards among them), `cargo test -p db -p forge -p server` clean
(db 50, forge 93, server 114, durable_completion_ledger 6, forge_runtime 34, self_heal 1),
`forge_work_claim_dev` (DEV) 2 passed — the pre-run sweep it exercises is the one that changed,
`cargo check --workspace --all-targets` clean.

Still open on this seam, named rather than implied: `model_policy` → model selection (billing: the Captain's call)
and `launch_intent` → Lead benchIntent; the artifact-out-of-role seam; hold/verdict out; canonical Story Board writes;
a read-only SQL verb in `cli` so live DEV functions and triggers can be audited (the gap in
`docs/agent/TEST-SAFETY-SWEEP-2026-09-29.md:90-105`); and the Phase 1 parity audit of the 465 legacy tests. The
scheduler stays stopped until the Captain says restart.

