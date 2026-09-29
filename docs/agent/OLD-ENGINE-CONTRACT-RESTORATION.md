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

## 1. Contract inventory

`old` = what the legacy engine enforced · `now` = what the Rust engine does today · `rail` = where the old engine
made it enforceable.

| # | Contract | old | now | rail |
| --- | --- | --- | --- | --- |
| 1 | Durable completion receipt / repair-replan ledger | post-transition completion unit persisted; `reconcile_completions()` could answer "did this task already complete" after a restart | `MemoryLedger`, process-local; a new engine process per dispatch starts blank | `workflow_command_receipt`, `forge_engine_task_execution`, `forge_story_run_receipt` (all exist) |
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

## 5. Blocking decisions (Captain)

1. **Keep or drop** the uncommitted contract 2 patch: five Rust files, `rust/forge/src/engine/runtime.rs`,
   `rust/forge/src/engine/runner.rs`, `rust/forge/src/engine/writer.rs`, `rust/forge/src/engine/db_writer.rs` and
   `rust/forge/tests/forge_runtime.rs`. Nothing committed, nothing pushed, no schema touched.
2. **Ledger storage shape** for contract 1: which existing receipt table owns the completion unit. Design first, no
   code, and no schema change without the Captain's word.
3. **The dispatch scheduler is stopped**, as a precondition for touching engine code, and stays stopped until the
   Captain says restart.
