# FORGE OPENS — Afternoon work order (2026-09-12)
**Against:** HEAD `3223902` · morning hardening closed OBS-SERIAL + SYNC-GUARD + env declaration + ForgeDB + Smith-cannot-plan  
**Suite baseline:** `pnpm test` 2062 / 0 fail · `test:forge:engine` 430 / 0 fail  
**Goal:** close the residuals from Grok’s 91 review. Do not reopen the engine. Do not add a second brain.

---

## Already done — do not redo

- Serial observer seam (`forge-observer-seam.ts`) + serial scope MISS/HOLD (`b484301`)
- Lane start PROD-only (`assertForgeLaneMayStart`)
- Environments declared, never inferred
- One pool (`db/forge-db.ts`), engine on shared pool, empty ratchet
- Smith cannot launch without an accepted Lead assignment (`57b64b7`)
- `lead_solo_implement` stays assignment-free
- Alerts never throw HOLD
- `cost_usd` is vendor-reported only — do not fill it from widgets

## Never-do

1. Do not run `pnpm forge:sync-history` to “clean” the 306 DEV runs into PROD.
2. Do not let Alerts throw HOLD.
3. Do not invent widgets→dollars into `cost_usd`.
4. Do not explore Pi / Swarm / Shepherd intercept.
5. Do not grow `engine.ts`.
6. Do not weaken PROD check constraints.
7. Do not disable dependency-cruiser.

---

## Order (do in this sequence)

| # | ID | Pts | Why now |
|---|---|---|---|
| 0 | FORGE-SMITH-DOOR-01 | 3 | Two commits must not drift; cheap test |
| 1 | FORGE-OBS-LIST-01 | 5 | Retry-hash is a lie across worker restart |
| 2 | FORGE-SESSION-ID-01 | 5 | Unlocks export later; small |
| 3 | FORGE-PARITY-CHECK-01 | 5 | Parity still cannot see CHECKs |
| 4 | FORGE-RECEIPT-PRODUCE-01 | 8 | Validator exists; no producer |
| 5 | FORGE-ARCH-BYPASS-01 | 5 | Gate runs; 6 hits still sit under it |
| 6 | FORGE-PACKET-OBS-01 | 8 | Feature; after honesty |

Skip 2 if the captain decides widgets-only ROI is enough this month. Do **not** skip 0 or 1.

Total if all land: **39**.

---

# 0. FORGE-SMITH-DOOR-01 — Both serial doors stay shut together
**Points:** 3  
**Issue:** `b484301` said “no assignment → nothing to enforce.” `57b64b7` said “no assignment → HOLD at launch.” If any path can still start `smith` / `repair_smith` / `fast_smith` / `fast_repair_smith` with an empty assignment **and** skip the scope lock, the lock is decorative again.

### Scope
- `workflow_app/forge/agent-runtime-role-runner.ts` (read + test; change only if a hole exists)
- Tests next to the existing serial / assignment suites

### Technical fix
- One test file (or cases on the existing runner tests) that names **both** doors:
  1. Launch without accepted Lead assignment → HOLD containing `Smith does not choose its own scope` (or the exact current string). Covers all four Smith roles. `lead_solo_implement` does **not** HOLD for this reason.
  2. Launch *with* assignment, candidate paths outside `allowedScope` → MISS / self-heal directive names the path; exhaustion HOLD is thrown by the runner, not by `evaluateAlerts`.
- If a hole exists, close it in the runner the same way `57b64b7` / `b484301` already do. Do not invent a third policy.

### Acceptance
- [ ] Four Smith roles: no assignment → HOLD before OpenCode spawn (mocked).
- [ ] `lead_solo_implement` with no assignment → does not take that HOLD.
- [ ] Assigned Smith + out-of-scope paths → miss reasons include `smith-scope:` and Alerts do not throw.
- [ ] `pnpm test:forge:engine` 0 fail.

---

# 1. FORGE-OBS-LIST-01 — Alerts see prior attempts after restart
**Points:** 5  
**Issue:** Persistent sink writes through to `workflow_execution_trace_event` but `list()` is the in-process memory sink. `RETRY_UNCHANGED_INPUT` cannot see attempt N−1 after the worker process dies.

### Scope
- `workflow_app/forge/forge-observer/persistent-sink.ts`
- New reader (same folder) mapping trace rows → `TraceEvent`
- `forge-observer-seam.ts`: before `evaluateAlerts` on attempt ≥ 2, load prior events for that `storyId`
- Alerts stay pure

### Technical fix
- `load(storyId): Promise<TraceEvent[]>` reads `workflow_execution_trace_event` where `source_system = 'forge_observer'` (and storyId in metadata).
- Map `SCOPE_CHECK` → `scope.check`, etc. Missing optionals stay undefined. Never throw; failed load → `[]`.
- Seed the memory sink with loaded events **or** pass `loaded.concat(processLocal)` into `evaluateAlerts`. Do not double-append to the durable table (sourceEventId must stay unique).
- Write path stays fire-and-forget / non-throwing.

### Acceptance
- [ ] Fake write of two HOLDs with the same `retryHash` on attempts 1 and 2 → after `load()`, `evaluateAlerts` returns `RETRY_UNCHANGED_INPUT`.
- [ ] `load` of unknown story → `[]`, no throw.
- [ ] Re-running append of the same `sourceEventId` does not duplicate durable rows (replay-safe).
- [ ] Serial hook remains log-only for alerts (no throw).
- [ ] `pnpm test:observer` and `pnpm test:forge:engine` 0 fail.

---

# 2. FORGE-SESSION-ID-01 — Persist the OpenCode session id
**Points:** 5  
**Issue:** Session continuity / `--session` exists; the id actually used is not stored on the attempt. Export and vendor `cost_usd` cannot be attributed. This story stores the key only.

### Scope
- `agent-runtime/opencode/opencode-client.ts`
- `agent-runtime/opencode/opencode-harness-adapter.ts`
- Evidence metadata already on the run/work item — prefer an existing JSON/notes field: `opencodeSessionId`
- New column only if nothing honest exists; if added, DEV + PROD together

### Technical fix
- Capture the session id the process actually used. Empty → `null`, never `"unknown"`.
- Persist on attempt evidence.
- Do **not** call `opencode export`. Do **not** write widgets into `cost_usd`.
- Continuity marker stays env-gated default OFF.

### Acceptance
- [ ] Fake handle with a session id → evidence contains that id.
- [ ] Failed spawn → `null`.
- [ ] Scorecard still prints tokens/cost NOT CAPTURED.
- [ ] `pnpm test:forge:engine` 0 fail.

---

# 3. FORGE-PARITY-CHECK-01 — Parity compares CHECK constraints
**Points:** 5  
**Issue:** PROD has `agent_work_item_parallel_shape_check`. DEV does not. `pnpm db:parity` compared tables/columns/indexes/FKs and reported 0 drift. Two malformed DEV rows were correctly left out of PROD.

### Scope
- Parity script / comparator
- Add the missing CHECK to **DEV only** (copy PROD definition; idempotent)
- Do not insert the two bad DEV rows into PROD
- Do not change PROD

### Technical fix
- Diff check constraints (name + src) per table. Constraint-only drift is drift (non-zero exit).
- Migration: `IF NOT EXISTS` the PROD check onto DEV.

### Acceptance
- [ ] Before DEV constraint: parity names the missing check and exits non-zero.
- [ ] After apply: that item gone from the diff.
- [ ] PROD constraint text unchanged.
- [ ] No force-copy of the two malformed rows.

---

# 4. FORGE-RECEIPT-PRODUCE-01 — Produce a receipt from a real signal
**Points:** 8  
**Issue:** `forge-release-receipt.ts` rejects placeholders. Nothing fills `releaseEvidence` from a host signal. Do not mark WS-14 / TECH-DEBT-07 Complete without a real PROD deploy.

### Scope
- Existing `releaseReceiptFromDeploymentSignal` / `isRecordedDeploymentDeferral`
- DevOps / publish path that already talks to Vercel (or the host you use)
- `AgentRunEvidence.releaseEvidence`

### Technical fix
- Wire the actual adapter payload (deployment id + artifact sha) into the existing producer.
- No payload → `null` (gate does not pass).
- Explicit deferral → typed deferral, not a fake receipt.
- Placeholders (`n/a`, `tbd`, `waived`) still fail closed.

### Acceptance
- [ ] Fixture payload with id + sha → receipt `assessReleaseReceipt` accepts.
- [ ] No payload → null; gate does not pass on a placeholder.
- [ ] Deferral flag → deferral row, not a receipt.
- [ ] Existing receipt tests still pass.
- [ ] Board story stays In Progress unless a **live** PROD deploy was observed. Say so in the commit.

---

# 5. FORGE-ARCH-BYPASS-01 — The 6 cruiser hits
**Points:** 5  
**Issue:** First live dependency-cruiser run found review-storyboard, review-dashboard ×3, form-editor-surface ×2 importing `db/` directly. Confirm the live list with `pnpm forge:tools --run` before editing; do not expand scope.

### Scope
- Only those cited files (reconfirm)
- Route through the existing domain service or an existing server facade

### Technical fix
- Replace `db/` imports. Prefer zero cruiser exceptions. Max two named exceptions, each with a follow-on story id in the comment.

### Acceptance
- [ ] Architecture gate `archRan:true` and **zero** of these paths in violations.
- [ ] No new `from 'db/` in those files.
- [ ] `tsc --noEmit` clean for touched files.

---

# 6. FORGE-PACKET-OBS-01 — Context packet, observer mode
**Points:** 8  
**Feature.** Compile a provenance-bearing packet; do **not** replace the live OpenCode prompt in this story.

### Scope
- New `workflow_app/forge/forge-context-packet.ts` (pure)
- Inputs: story id, node, attempt, Scout text, Architect seams, accepted assignment paths, frozen assay commands, AGENTS excerpts already in prompts
- Output: `{ sources: [{ path, reason, bytes }], omittedCount, estimatedTokens, incomplete?: true }`
- Record `packetHash` on `run.start` via the existing observer seam when cheap

### Technical fix
- Required sources first; depth-1 relative imports only; skip `node_modules`.
- Split child: omit sibling implementation files unless they are in that child’s `allowedScope`.
- Same inputs → same sorted source list.
- Missing required context → `incomplete: true`, no throw, no whole-repo scan.

### Acceptance
- [ ] Deterministic manifest test.
- [ ] Sibling file omitted unless in allowedScope.
- [ ] Incomplete required context sets the flag.
- [ ] No FORGE_SDLC transition change.
- [ ] `pnpm test:forge:engine` 0 fail.

---

## Parked (not this afternoon)

- `opencode export` → `cost_usd` (needs story 2 first)
- Widgets × spot-rate `cost_usd_est` (captain’s notebook, not a column)
- Shepherd revert / tool.intend
- Soft-order field on the board
- Cleaning DEV history

---

## Verification after each story

```
pnpm exec tsc --noEmit
pnpm test:forge:engine
pnpm test:observer          # stories 1, 6
pnpm forge:tools --run      # story 5
pnpm db:parity              # story 3
git diff --check
```

One story per commit: `feat(forge): …` / `fix(db): …` with the story id in the body. One factual MEMORY.md line per landing.

Do not mark a board row Complete because a commit exists. Completion = the boxes above.
