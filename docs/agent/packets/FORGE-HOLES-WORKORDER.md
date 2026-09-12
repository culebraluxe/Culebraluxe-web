# FORGE HOLES — Work order for Cline / DeepSeek
**Date:** 2026-09-12  
**Basis:** Grok Forge review vs HEAD `46ad142` (Forge movement through 11 Sep; last 36h on main is Projects UI)  
**Engine suite last recorded:** 405 pass / 0 fail / 4 skipped  
**Goal:** close measured honesty holes. Do not add a second orchestration brain.

---

## Authority (never violate)

- `WorkflowEngine` + `FORGE_SDLC` own the next node. Do not add a reducer that routes.
- OpenCode is the inner harness only. No MCP wave, no Maestro store, no Shepherd Python.
- `hold-recommend` from `forge-alerts` is NOT a HOLD. The runner / engine throws HOLD.
- Captain paths: do not delete or “clean” `workflow_engine/**`, `lib/mq/**`, `grok/**`, `workflow_app/scripts/**`. Report only.
- Forge execution target is **PROD only**. DEV is not a Forge lane. Fail closed.
- `cost_usd` is vendor-reported only. Never invent a widgets→dollars rate.
- Do not convert knip unused-file lists into deletions on captain paths.
- Before deleting any “dead” file: (1) import graph AND (2) path-string references in tests/scripts.

---

## Never-do (this order)

1. Do not hook Alerts to throw HOLD on serial Smith.
2. Do not port Shepherd intercept / live `tool.intend`.
3. Do not run or default Forge lanes to DEV.
4. Do not fill `cost_usd` or tokens with estimates.
5. Do not weaken PROD check constraints to match DEV.
6. Do not disable dependency-cruiser to ship UI.
7. Do not grow `engine.ts`.
8. Do not add `.maestro/`, wave state, or a second trace table.

---

## Story order (do in this sequence)

| # | ID | Points | Why this order |
|---|---|---|---|
| 1 | FORGE-OBS-SERIAL-01 | 8 | Observer is built and hooked on the empty path |
| 2 | FORGE-SYNC-GUARD-01 | 8 | Closes SYNC-01 part 3; stops another DEV/PROD split-brain |
| 3 | FORGE-SESSION-ID-01 | 5 | Unlocks real cost later; small |
| 4 | FORGE-OBS-LIST-01 | 5 | Makes RETRY_UNCHANGED_INPUT true across attempts |
| 5 | FORGE-PARITY-CHECK-01 | 5 | Parity currently lies about constraints |
| 6 | FORGE-RECEIPT-PRODUCE-01 | 8 | Receipt validator exists; no producer |
| 7 | FORGE-ARCH-BYPASS-01 | 5 | Cruiser already found 6 UI→db imports |
| 8 | FORGE-PACKET-OBS-01 | 8 | First Maestro acquisition; observer-mode only |
| 9 | FORGE-SCORECARD-BOARD-01 | 3 | Surface the scorecard; no new store |

Total: **55**. Do 1–3 before any “new capability” story. 8–9 are features; 1–7 are holes.

---

# 1. FORGE-OBS-SERIAL-01 — Observer on serial Smith and Lead
**Points:** 8  
**Issue:** Persistent sink and alert rules exist. The only runner hook is the **split-child** block. Scorecard split health is 0. Serial Smith / Lead / QA produce no `SCOPE_CHECK` / `GIT_COMMIT` / `HOLD` observer events. The worker-execution layer is dark on the path that actually runs.

### Scope
- `workflow_app/forge/agent-runtime-role-runner.ts` (and any serial Smith admit / Lead complete path that already has `scopeViolations` or HOLD)
- Reuse `workflow_app/forge/forge-observer/*` and `forge-alerts/*`
- Do **not** change HOLD policy
- Do **not** hook QA/Assay beyond recording `run.end` / existing HOLD if the call site is already there and cheap

### Technical fix
- After serial Smith candidate commit + `scopeViolations` (same moment split already records): call `recordGitCommit` + `recordScopeCheck` on `createPersistentTraceSink`.
- On existing HOLD throws / lead_pre reject: `recordHold` with `reasons` and optional `retryHash` via `retryInputHash({ missReasons })` when a miss list exists.
- On Lead PRE accept (SOLO/SMITH/SPLIT): `recordRunEnd` or a `run.start` for the chosen route — keep event kinds in the existing union. Do not invent `lead.decide`.
- Evaluate `evaluateAlerts` and `recordAlert` only. Log / persist. **Do not throw** on `hold-recommend`.
- Sink write must remain non-throwing.

### Acceptance
- [ ] A serial Smith candidate that leaves allowedScope records `SCOPE_CHECK` verdict=deny in `workflow_execution_trace_event` with `source_system='forge_observer'` **and** the runner still HOLDs by the existing throw, not by Alerts.
- [ ] A serial Smith candidate inside scope records `SCOPE_CHECK` allow + `GIT_COMMIT` when a commit exists.
- [ ] A Lead PRE HOLD records `HOLD` with reason text.
- [ ] Split path still records (no double-seq collision per attempt; sourceEventId stays `storyId:taskId:nodeId:attempt:seq`).
- [ ] `pnpm test:observer` green; new tests cover serial hook with a fake sink (no live OpenCode).
- [ ] `pnpm test:forge:engine` 0 fail.
- [ ] Scorecard `observerEvents` is non-empty after one serial fixture/test write (or a documented script that inserts via the sink).

---

# 2. FORGE-SYNC-GUARD-01 — Engine-start PROD fail-closed
**Points:** 8  
**Issue:** MEMORY says Forge never runs against DEV. Board-sync refuses non-PROD. The **engine / role-runner start** still can launch a lane when `APP_ENV` resolves to DEV. That is how the board went dark (WS series in DEV, board in PROD). ENG-FORGE-SYNC-01 part 3 is still open.

### Scope
- Start of Forge execution: role runner / invoker / engine-start wrapper — wherever a lane is launched
- Reuse `assertForgeExecutionTarget` if it exists; if it only lives on the sync script, lift it to a shared module under `workflow_app/forge/`
- Not: rewriting history, not re-running DEV work

### Technical fix
- Single function: resolved execution target must be `PROD` (or the exact enum the codebase already uses for production). Anything else throws before claim / before OpenCode spawn / before worktree provision.
- Surface `execution_environment` on work items / runs as today; do not invent a second field.
- Operator override, if any, must be an explicit env that is **not** documented as default and must still write the real target on the row. Prefer no override.
- Tests: unit the guard with fake env. One integration-style test that startExternal / execute refuses DEV.

### Acceptance
- [ ] With target DEV, Forge lane start throws and does not claim a task or spawn OpenCode.
- [ ] With target PROD, start proceeds (mocked harness).
- [ ] Board-sync already-PROD guard still passes.
- [ ] ENG-FORGE-SYNC-01 part 3 can be marked complete only after this ships; do not mark it complete on a docs commit.
- [ ] `pnpm test:forge:engine` 0 fail.

---

# 3. FORGE-SESSION-ID-01 — Persist OpenCode session id per attempt
**Points:** 5  
**Issue:** Client may pass `--session` / `--continue` but the new session id is not stored on the attempt. `cost_usd` and raw tokens cannot be attributed. `OpenCodeRunResult` has no usage field. Do **not** invent usage.

### Scope
- `agent-runtime/opencode/opencode-client.ts`
- `agent-runtime/opencode/opencode-harness-adapter.ts`
- Evidence / work-item / run columns that already exist for session or metadata — **prefer an existing JSON/notes/metadata field** over a new migration if one can hold `opencodeSessionId`
- New column only if nothing honest exists; if added, add to DEV and PROD together

### Technical fix
- Capture the session id OpenCode actually used (stdout/stderr parse or the API the client already has). Empty → null, never a fake id.
- Persist on the attempt evidence: `opencodeSessionId`.
- Do not call `opencode export` in this story. This story only stores the key.
- Session continuity marker (`.forge-session.continue`) stays env-gated default OFF.

### Acceptance
- [ ] Successful OpenCode fixture/test persists a non-empty session id when the fake handle provides one.
- [ ] Failed spawn persists null, not `"unknown"`.
- [ ] Scorecard still reports tokens/cost NOT CAPTURED (this story does not fill those columns).
- [ ] No widgets→dollars math.
- [ ] `pnpm test:forge:engine` 0 fail.

---

# 4. FORGE-OBS-LIST-01 — Alert rules can see prior attempts
**Points:** 5  
**Issue:** `list()` is the in-process memory sink. Durable write is fire-and-forget. `RETRY_UNCHANGED_INPUT` cannot see attempt N-1 after process restart.

### Scope
- `workflow_app/forge/forge-observer/persistent-sink.ts`
- Optional: `listFromTrace` reader over `workflow_execution_trace_event` where `source_system='forge_observer'`
- Alerts stay pure over `TraceEvent[]`

### Technical fix
- Add an async `load(storyId)` (or sync cache fill at run start) that maps trace rows back to `TraceEvent` (underscore types → dotted kinds).
- Runner: before `evaluateAlerts` on attempt ≥ 2, load prior events for that storyId+nodeId.
- Mapping must be loss-tolerant: missing optional fields stay undefined.
- Write still never throws.

### Acceptance
- [ ] Test: persist two HOLD events with the same `retryHash` via the write fake, `load()`, `evaluateAlerts` returns `RETRY_UNCHANGED_INPUT`.
- [ ] Test: load of empty story returns `[]`, not throw.
- [ ] Serial hook from story 1 still log-only.
- [ ] `pnpm test:observer` and `pnpm test:forge:engine` 0 fail.

---

# 5. FORGE-PARITY-CHECK-01 — Parity compares check constraints
**Points:** 5  
**Issue:** PROD has `agent_work_item_parallel_shape_check`. DEV does not. `pnpm db:parity` reported 0 drift because it compares tables/columns/indexes/FKs, not CHECKs. DEV accepted rows PROD rejected. Two malformed parallel-shape rows remain in DEV on purpose.

### Scope
- The parity script / comparator (`pnpm db:parity` implementation)
- Apply the missing CHECK to **DEV only** (copy PROD definition). Do not alter PROD.
- Do not force the two malformed DEV rows into PROD.

### Technical fix
- Extend parity to list check constraints per table (name + src) on both DBs and diff them.
- Report constraint-only drift as drift (non-zero exit).
- Add the PROD `agent_work_item_parallel_shape_check` to DEV via a migration that is safe if already present.
- Document the two leftover DEV rows as out-of-scope (investigate, do not weaken the check).

### Acceptance
- [ ] Before DEV constraint: parity fails with a named missing check.
- [ ] After applying the check to DEV: that item disappears from the diff.
- [ ] PROD constraint text unchanged.
- [ ] No `ON CONFLICT` / insert of the two bad rows into PROD.
- [ ] `pnpm test:forge:engine` 0 fail (or parity unit tests if they exist).

---

# 6. FORGE-RECEIPT-PRODUCE-01 — Real release receipt producer
**Points:** 8  
**Issue:** `forge-release-receipt.ts` rejects placeholders. Nothing produces `releaseEvidence` from a real host signal. Deploy/production_smoke still blocks or waits. `MISSING_DEPLOY_RECEIPT` stays watch forever.

### Scope
- `workflow_app/forge/forge-release-receipt.ts` (keep fail-closed assessor)
- DevOps / publish path that already talks to Vercel or the host you actually use
- `AgentRunEvidence.releaseEvidence`
- Not: fabricating receipts for WS-14; not marking WS-14 Complete

### Technical fix
- One producer: `releaseReceiptFromDeploymentSignal` already returns null without a signal — wire the **actual** deploy adapter output (deployment id + artifact sha + url or equivalent) into that function.
- If this run is a recorded batch deferral, persist a **typed deferral** (`isRecordedDeploymentDeferral`), not a fake receipt.
- Gate continues to reject `n/a`, `tbd`, `waived`, non-sha artifacts.

### Acceptance
- [ ] Given a fixture provider payload with id + sha, evidence contains a receipt that `assessReleaseReceipt` accepts.
- [ ] Given no provider payload, producer returns null; gate does not pass on a placeholder.
- [ ] Given explicit deferral flag, row is a deferral not a receipt; scorecard/alerts can see watch not a fake pass.
- [ ] 10 existing receipt tests still pass; add producer tests.
- [ ] Do not flip TECH-DEBT-07 / WS-14 to Complete unless a **real PROD deploy** has been observed. If none, leave In Progress and say so in the PR.

---

# 7. FORGE-ARCH-BYPASS-01 — Close the 6 cruiser hits (or quarantine with a ticket)
**Points:** 5  
**Issue:** First live `dependency-cruiser` run found 6 violations: review-storyboard, review-dashboard ×3, form-editor-surface ×2 importing `db/` directly. Architecture hard gate is now real. Leaving them means QA will fail closed on arch.

### Scope
- Only the 6 cited files (confirm current list with `pnpm forge:tools --run` / the gate artifact — do not expand into a rewrite)
- Route reads through existing services or a thin already-used facade
- Not: redesigning review UI

### Technical fix
- Replace `db/` imports with the domain service already used elsewhere for that read, or one shared server facade if one exists.
- If a call is inherently admin/debug and has no service, add a **named** cruiser exception with a story id in the comment — max 2 exceptions, each with a follow-on story id. Prefer zero exceptions.

### Acceptance
- [ ] `pnpm forge:tools --run` architecture gate `archRan:true` and **zero** of these 6 paths in violations.
- [ ] No new `from 'db/` or `from '../db/` in those files.
- [ ] `pnpm exec tsc --noEmit` clean for touched files.
- [ ] Review pages still typecheck; no behavior change required beyond import path.

---

# 8. FORGE-PACKET-OBS-01 — Context packet compiler (observer mode)
**Points:** 8  
**Feature (Maestro pirate #1).** Highest-value new work after holes 1–3. Changes what OpenCode *would* receive; does not change who routes.

### Issue
Scout has Ripwire `--pack-task`. Lead/Smith get large prompts. There is no provenance-bearing packet: seams + assignment files + 1-hop deps + assay commands + AGENTS excerpt, with a manifest.

### Scope
- New module `workflow_app/forge/forge-context-packet.ts` (pure compile + types)
- Inputs: story id, node, attempt, Scout findings text, Architect seams, accepted Lead assignment paths, frozen assay commands, AGENTS excerpts already used in prompts
- Output: manifest `{ sources: [{ path or ref, reason, bytes, hash? }], omittedCount, estimatedTokens }`
- **Observer mode:** compile and persist on evidence / observer event. Do **not** replace the live prompt in this story.
- Split child: packet must not include sibling implementation files; collision/integration facts only if already on the assignment.

### Technical fix
- Selection: required sources first; expand assignment files to **direct import neighbors only** when the import specifier is relative (same idea as Maestro indexer, bounded depth 1). Skip `node_modules`.
- Missing required context → manifest flag `incomplete: true` + reason. Do not scan the whole repo.
- Deterministic: same inputs → same source list (sort paths).
- Record `detail.packetHash` on `run.start` when the serial observer hook exists.

### Acceptance
- [ ] Unit: same seams+assignment → identical manifest source list.
- [ ] Unit: sibling file of a split child is omitted unless it is in that child’s allowedScope.
- [ ] Unit: incomplete required context sets `incomplete: true`, does not throw.
- [ ] No change to FORGE_SDLC transitions.
- [ ] `pnpm test:forge:engine` 0 fail.

---

# 9. FORGE-SCORECARD-BOARD-01 — Publish scorecard as a read-only CLI + note
**Points:** 3  
**Feature.** Scorecard function exists (`pnpm forge:scorecard`). Make the command the operator contract and pin the honesty rules in the printed output so a model cannot “fix” nulls to zero.

### Scope
- `scripts/` CLI if not already complete
- Do not add a dashboard page unless one already exists to hang a panel on
- Read-only PROD

### Technical fix
- CLI prints outcomes, roles, splitHealth, telemetry.note verbatim.
- Exit 0 even when rates are null.
- Refuse to run writes.

### Acceptance
- [ ] `pnpm forge:scorecard 30` runs against PROD read path (or documented execute injection) and prints `NOT CAPTURED` for tokens/cost while those columns are empty.
- [ ] Does not insert rows.
- [ ] Documented in `docs/agent/MEMORY.md` one line: command + honesty rules.

---

## Parked (do not implement in this order)

- Intra-worktree `git reset` to last-good (Shepherd revert). Wait until serial observer shows Smith-thrash is the leak.
- OpenCode tool proxy / `tool.intend`.
- Filling `cost_usd` via `opencode export` (needs story 3 first; separate story).
- Calibration / learned LEAD sizing.
- Deploy-receipt unblocking WS-14 without a real signal or captain waiver.
- Deleting knip unused files on captain paths.

---

## Verification for the whole batch

After each story:

```
pnpm exec tsc --noEmit
pnpm test:forge:engine
pnpm test:observer          # when observer files change
pnpm forge:tools --run      # when gate/catalog change
git diff --check
```

Do not mark a board story Complete because a commit exists. SYNC-01 taught that. Completion = acceptance boxes above.

## PR hygiene

- One story per PR if possible; 1+2 may combine if the runner file is already open.
- Commit messages: `feat(forge): …` / `fix(forge): …` with the story id in the body.
- Update `docs/agent/MEMORY.md` with one factual line per landed story (what is now true, not a pep talk).
