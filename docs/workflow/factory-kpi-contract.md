# Software Factory KPI + Dispatch Model — Metric Contract (ENG-17)

**Owner:** AI Software Factory (Forge) / Pippin (watcher)
**Status:** V1 — deterministic rules over persisted control-plane data
**Read model:** `lib/factory-kpi.ts` (pure), `lib/factory-command-center-data.ts` (projection wiring)
**Display:** Factory Command Center (`/portal/command-center`, ENG-16/17)
**Principles**

- Operations research / decision support, not dashboard decoration.
- Every KPI below has a precise formula, source tables, window, edge cases and
  an intended decision.
- A KPI whose telemetry is absent is reported as `null` with a `missingReason`
  — **unavailable data is never rendered as a fabricated zero**.
- Executive completion/flow metrics use **rollup-participating stories only**
  (`storyboard_story.rollup = true`). Parent/non-rollup rows (e.g. ENG-20-SMOKE-001)
  are counted separately and never double-counted (see "Executive scope").
- V1 recommendations are deterministic rules over priority, dependency
  eligibility, capability and age. Optimization/solver logic is deferred until
  data volume justifies it.

**Terminology**

| Term | Meaning |
|---|---|
| rollup story | `storyboard_story.rollup = true` — participates in completion aggregates |
| parent story | `rollup = false` — a tracking/rollup row whose children carry the weight |
| run | one `storyboard_story_run` row — a persisted execution OUTCOME |
| work item / command | one `agent_work_item` row — the durable queue command |
| slot | the single execution slot (migration 025: at most one Claimed/Running system-wide) |

---

## 1. Outcome KPIs — delivery + quality

### O1. Net-net completion (`net-net completion`)
- **Formula:** `Σ (workstream_completion_percent × workstream_weight) / 100`, where
  `workstream_completion_percent = AVG(storyboard_story.completion)` over the
  workstream's **rollup** stories and weights come from the Story Board model
  (PUBLIC 20, CRM 20, PORTAL 20, TXN 15, ADMIN 10, AUTH 5, CONTENT 5, HARDEN 5).
- **Source:** `storyboard_story` (completion, workstream, rollup).
- **Window:** instantaneous (current board state).
- **Edge cases:** workstream with zero rollup stories contributes 0%. Parent
  rows excluded. Completion is the stored 0–100, never status scoring.
- **Intended decision:** executive trajectory; a declining net-net with rising
  WIP signals queue build-up rather than delivery.

### O2. Completion rate (`completion rate`)
- **Formula:** `count(rollup stories with status = 'Complete') / count(rollup stories) × 100`.
- **Source:** `storyboard_story` (status, rollup).
- **Window:** instantaneous.
- **Edge cases:** zero rollup stories → `null` ("no rollup-participating
  stories"); parents excluded (no double-count).
- **Intended decision:** how much of the committed backlog is closed.

### O3. Throughput — stories completed in window (`throughput (window)`)
- **Formula:** `count(storyboard_story where status = 'Complete' and completed_at >= now() - window)`.
- **Source:** `storyboard_story` (status, completed_at, rollup).
- **Window:** default 30 days (`windowDays`), configurable.
- **Edge cases:** stories completed before run telemetry existed still count
  (completed_at is a column). Stories with `completed_at IS NULL` but
  status Complete are excluded (edge documented in the story lifecycle:
  `finishStoryRun` always sets `completed_at` on Complete).
- **Intended decision:** delivery rate; compare against WIP and queue age to
  judge whether the factory is finishing or merely starting work.

### O4. Cycle time (`cycle time`)
- **Formula:** median over in-window completions of
  `(completed_at − actual_start_at)`. Also reported as mean where the UI needs it.
- **Source:** `storyboard_story` (actual_start_at, completed_at, status, rollup).
- **Window:** same as throughput (30d).
- **Edge cases:** `actual_start_at IS NULL` → that story contributes nothing and
  the KPI reports `null` with **missing telemetry** ("work started before run
  telemetry existed") — the first-start timestamp only began being recorded
  with migration 023 run telemetry.
- **Intended decision:** the active-execution efficiency of the factory; a
  growing cycle time with flat throughput indicates execution slowdown.

### O5. Lead time (`lead time`)
- **Formula:** median over in-window completions of `(completed_at − created_at)`.
- **Source:** `storyboard_story` (created_at, completed_at, status, rollup).
- **Window:** 30d.
- **Edge cases:** null when no in-window completions or missing timestamps
  (reported as missing telemetry, never 0).
- **Intended decision:** end-to-end responsiveness (backlog entry → delivered);
  the difference between lead time and cycle time is the queue wait.

### O6. Run pass rate (`run pass rate`)
- **Formula:** `count(storyboard_story_run where result_status = 'Complete') /
  count(storyboard_story_run where result_status IS NOT NULL) × 100`, over runs
  started in the window.
- **Source:** `storyboard_story_run` (result_status, started_at).
- **Window:** 30d.
- **Edge cases:** runs with `result_status IS NULL` are live/incomplete —
  excluded from the denominator (a running attempt is not a failed one). Zero
  terminal runs → `null`.
- **Intended decision:** quality of execution; a drop below the trailing norm
  triggers Pippin `watch` before it becomes systemic.

### O7. Run failure rate (`run failure rate`)
- **Formula:** `count(runs with result_status = 'Failed') / count(terminal runs) × 100`.
- **Source:** `storyboard_story_run`.
- **Window:** 30d.
- **Edge cases:** `Cancelled` is NOT a failure (operator decision) and is
  excluded from the numerator; it stays in the denominator as a terminal run.
- **Intended decision:** systemic failure pressure. Three or more distinct
  failed stories in the window flips Pippin to `watch`.

### O8. Command retry rate (`retry rate`)
- **Formula:** `count(agent_work_item where attempts >= 2) /
  count(agent_work_item where attempts >= 1) × 100`.
- **Source:** `agent_work_item` (attempts). `attempts` increments on each claim.
- **Window:** all-time (attempts is a cumulative counter; a windowed variant
  needs a claim-history table, which is a documented missing telemetry field —
  see §6).
- **Edge cases:** items with `attempts = 0` (trigger-created, never claimed) are
  excluded from the denominator — they were never attempted. Zero claimed
  commands → `null`.
- **Intended decision:** stability of the runtime; a high retry rate on a
  single story (≥2) flips Pippin to `watch` (repeated failure).

---

## 2. Flow KPIs — WIP / queue age / cycle / blocking

### F1. Work in progress (`wip`)
- **Formula:** `count(rollup stories with status in ('In Progress','Partial'))`.
- **Source:** `storyboard_story` (status, rollup).
- **Window:** instantaneous.
- **Edge cases:** parents excluded. Ready is NOT WIP (authorized but not started).
- **Intended decision:** concurrency limit signal. WIP above the sustainable
  level with flat throughput = queue build-up; the factory should finish before
  replenishing.

### F2. Ready (authorized) (`ready (authorized)`)
- **Formula:** `count(stories with status = 'Ready')`.
- **Source:** `storyboard_story`.
- **Window:** instantaneous.
- **Edge cases:** includes Ready stories whose dependencies are unmet — the
  "authorized" backlog.
- **Intended decision:** how much work is explicitly authorized to execute.

### F3. Ready & dependency-eligible (`ready eligible`)
- **Formula:** `count(Ready stories with all known dependencies Complete)` —
  exactly the ENG-16 pipeline `readyWork` (the tolerant dependency parser never
  false-blocks on external/unverifiable refs).
- **Source:** `storyboard_story` + dependency refs parsed from `dependencies`;
  satisfaction from the known storyboard_story set.
- **Window:** instantaneous.
- **Edge cases:** external refs (no board story) never block eligibility.
- **Intended decision:** the **ready-work count that respects dependencies and
  eligibility** — this is the number used for replenishment, not the raw Ready
  count.

### F4. Ready waiting on deps (`ready waiting on deps`)
- **Formula:** `F2 − F3` (Ready stories excluded from eligible).
- **Edge cases:** a Ready story can also be hard-blocked; it counts here only if
  it is not in `readyWork`.
- **Intended decision:** authorized work stuck behind unfinished dependencies —
  a reason to prioritize those dependencies.

### F5. Queue age — oldest eligible (`queue age (oldest eligible)`)
- **Formula:** `max over eligible Ready stories of (now − readySince)`, hours.
  `readySince` = the story's Ready work item `agent_work_item.queued_at` when
  present (the DB dispatch trigger creates it on Ready), else
  `storyboard_story.updated_at` (documented approximation).
- **Source:** `agent_work_item` (queued_at, state), `storyboard_story`.
- **Window:** instantaneous.
- **Edge cases:** null when no eligible ready work. The `updated_at` fallback
  resets if a human edits a Ready story — documented as an approximation.
- **Intended decision:** dispatch staleness. When the slot is free and the
  oldest eligible exceeds `schedulerWedgeMinutes` (default 60), Pippin escalates
  (scheduler wedge).

### F6. Blocked / waiting work (`blocked work`)
- **Formula:** `count(stories in the pipeline blocked stage)` = hard blocked
  (status Blocked/Failed or latest work item Error/Cancelled) + Ready/planned
  waiting on unmet dependencies.
- **Source:** ENG-16 pipeline (`blockedWork`), derived from `storyboard_story`
  and `agent_work_item`.
- **Window:** instantaneous.
- **Edge cases:** Planned stories with unmet deps are blocked in the pipeline
  but reported in this count as "waiting" (they do not consume execution).
- **Intended decision:** what is stuck and why; feeds the critical-dependency
  pressure signal.

### F7. Longest blocked age (`blocked age (max)`)
- **Formula:** `max over blocked stories of (now − storyboard_story.updated_at)`,
  hours.
- **Source:** `storyboard_story` (updated_at).
- **Window:** instantaneous.
- **Edge cases:** `updated_at` is the last story mutation — a human edit resets
  the proxy age. **Missing telemetry:** no dedicated `blocked_at` timestamp
  exists (documented in §6).
- **Intended decision:** how long work has been wedged; long blocked age on a
  Critical dependency escalates urgency.

### F8. Stale active commands (`stale active`)
- **Formula:** `count(agent_work_item where state in ('Claimed','Running') and
  (now − updated_at) > staleAfterMinutes)` (default 60m).
- **Source:** `agent_work_item` (state, updated_at — the heartbeat).
- **Window:** instantaneous.
- **Edge cases:** `updated_at IS NULL` counts as stale. Paused items are NOT in
  this count (they are held, not dead).
- **Intended decision:** the wedge detector — a stale active slot escalates
  Pippin to `FACTORY UNHEALTHY` because the queue is blocked behind a presumed
  dead worker (recover via `pnpm agent:work --recover`).

---

## 3. Capacity KPIs — busy / available / blocked / waiting

The acceptance criterion "capacity distinguishes busy, available, blocked and
waiting" is implemented by the four-state slot model
(`lib/factory-command-center-data.ts` `buildFactoryCapacity`).

### C1. Busy (`busyCount`)
- **Formula:** `count(agent_work_item where state in ('Claimed','Running'))`.
- **Meaning:** the execution slot is actively working.
- **Intended decision:** the factory is engaged; do not dispatch a second
  command (migration 025 forbids it anyway).

### C2. Waiting (`waitingCount`)
- **Formula:** `count(agent_work_item where state = 'Paused')`.
- **Meaning:** the slot is HELD but paused — awaiting operator resume/cancel.
  Not busy, not free.
- **Intended decision:** an operator action is the only way this slot moves;
  replenishment is NOT authorized while paused.

### C3. Blocked (`blockedCount`)
- **Formula:** latest terminal `Error`/`Cancelled` work item per worker.
- **Meaning:** the STORY is stuck (needs a human), but the execution slot is
  RELEASED — `claimNextAgentWork` only refuses Claimed/Running, so new work can
  proceed. One failed story costs one story, not the shift.
- **Intended decision:** human attention list; the blocked story must be
  resolved (or re-Readied) before it flows again.

### C4. Available (`availableCount`)
- **Formula:** `1` when no busy and no waiting, else `0`.
- **Meaning:** the slot can take a new claim right now — even when a blocked
  story still needs human attention.
- **Intended decision:** replenishment is safe. **Anti-vanity guard:** an
  available slot is NOT a failure — the KPI's intended decision is "take
  dependency-eligible work if it exists; otherwise idle is healthy". The
  factory is never scored on utilization; it is scored on finishing work.

### C5. Capability state (`byCapability`) and demand (`demandByCapability`)
- **Formula:** capability = `role/modelProfile` of the durable command envelope
  (`agent_work_item.role`, `agent_work_item.model_profile`). State rows: the
  active slot's capability (busy/waiting), each blocked command's capability,
  and "unassigned" when available. Demand = count of auto-dispatch-eligible
  commands per capability.
- **Source:** `agent_work_item` (role, model_profile, state).
- **Edge cases:** **missing telemetry:** there is no persisted roster of
  registered agents (who is online, which capabilities are free). Capability is
  therefore INFERRED from the command envelope, never from a roster (see §6).
- **Intended decision:** whether the next command's required capability matches
  the available slot; a demand with no matching available capability is a
  human/roster question, not a scheduler bug.

---

## 4. Decision signals — the highest-value next action

### D1. Auto-dispatch eligible (`autoDispatchEligible`)
- **Formula:** `readyEligible − humanGated` where humanGated = stories whose
  latest work item has execution policy `Human Gate | Manual Only | Daytime Only`.
- **Source:** `agent_work_item` (execution_policy), pipeline `readyWork`/`gatedWork`.
- **Intended decision:** the exact set an unattended worker may claim.

### D2. Human gate count (`humanGateCount`)
- **Formula:** `count(latest work items with a human execution policy)`.
- **Intended decision:** operator attention load; gated work is excluded from
  auto-dispatch but still shown as eligible to a human.

### D3. Critical dependency pressure (`criticalDependencyCount`)
- **Formula:** `count(stories whose blockedBy contains a dependency with status
  'Blocked' or 'Failed')` — a wedge that will not self-resolve.
- **Source:** ENG-16 pipeline `blockedBy` (dependency satisfaction graph).
- **Intended decision:** the highest-leverage unblock: fixing the wedged
  dependency unblocks everything waiting on it.

### D4. Recommended next dispatch (`recommended`)
- **Formula (deterministic V1 rule, no solver):**
  1. candidates = `autoDispatchEligible` (ready + deps satisfied + not gated);
  2. rank by `priorityRankOf(story.priority)` asc (Critical=0 … Later=6);
  3. ties broken by queue age desc (oldest eligible ready first);
  4. final tie-break by story id asc (stable).
- **Reasons trail:** the recommendation carries why it won (priority, age,
  capability configured, dependency-satisfied).
- **Intended decision:** the single highest-value next command to claim when the
  slot is free — priority first, age second, capability reported.

---

## 5. PIPPIN WATCH SOP — factory health assessment

`assessFactoryHealth` (`lib/factory-kpi.ts`) classifies the factory:

| Level | Trigger (deterministic) |
|---|---|
| `escalate` (FACTORY UNHEALTHY) | (a) stale active slot: Claimed/Running with heartbeat silent > `staleAfterMinutes`; or (b) scheduler wedge: slot free AND eligible ready work older than `schedulerWedgeMinutes` unclaimed |
| `watch` | (a) ≥2 terminal failures (Error/Cancelled) on the SAME story; or (b) ≥3 distinct stories failed in the outcome window (systemic pattern) |
| `healthy` | everything else, including isolated failures whose slot was released and later work progressed |

**Doctrine rules honored:**

1. A single isolated story failure is classified (`isolatedFailures`) and
   suppressed from alarm language — it never escalates the factory.
2. Escalation requires a wedge: stale slot, unclaimed eligible work, repeated
   failure on the same story, or systemic (≥3) failures. Failure count alone is
   never the stop condition.
3. Historical residue (old Error/Cancelled rows) is noted once and then folded
   into the healthy summary ("X is an isolated failed story; slot released, not
   blocking throughput").
4. Queue replenishment is NOT blocked by a single failed story: `available`
   remains 1 (slot released) and `readyEligible` still drives the
   recommendation.
5. Reporting language always distinguishes STORY FAILED from FACTORY UNHEALTHY:
   `health.summary` says "Forge healthy; ENG-14 Running with fresh heartbeat.
   AUTH-05 is an isolated failed story; slot released, not blocking
   throughput." — never "factory down" because one story failed.

---

## 6. Missing telemetry (explicitly identified)

| Gap | Impact | Fix path |
|---|---|---|
| No `ready_at` on `storyboard_story` | Queue age falls back to `agent_work_item.queued_at` (preferred) or `updated_at` (approximation) | Add `ready_at` set by the Ready transition |
| No `blocked_at` on stories | Blocked age uses `updated_at` proxy | Add `blocked_at` set on Blocked/Failed transitions |
| No agent roster (who is online / free) | Capability is inferred from the command envelope only | Persist an agent/capability registry |
| No claim-history table | Retry rate is all-time, not windowed | Persist per-claim timestamps (attempts counter exists) |
| `actual_start_at` absent on legacy completions | Cycle time reports `null` + reason for those | Historical backfill when available |

Every gap is surfaced to the operator as `missingReason` in the UI rather than
a fabricated zero.

---

## 7. Executive scope — no parent double-counting

- Completion/flow aggregates (net-net, completion rate, WIP, throughput, cycle
  time, lead time) compute over `rollup = true` stories **only**.
- `scope` exposes `rollupStoryCount`, `parentStoryCount`, `parentStoryIds` so
  the console can state "N rollup stories · M parent/non-rollup tracked
  separately".
- Run-level quality metrics (pass/fail/retry) include all runs (a parent can
  have its own run) but never sum parent completion into the rollup — the
  completion math is exclusively rollup-scoped.
