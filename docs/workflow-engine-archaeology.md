# Workflow Engine Archaeology & Capability Assessment

Status: Read-only assessment. No code changes, no SQL, no Neon mutation, no
package installs. The preserved engine in [`workflow_engine/`](../workflow_engine/)
is evaluated against the brokerage needs and against the application-side
contract surface in [`lib/workflow/`](../lib/workflow/) and
[`docs/workflow-integration-contract.md`](workflow-integration-contract.md).

---

## 1. Architecture map

The engine is a single-file, transaction-based process executor plus a thin
persistence layer.

```
workflow_engine/
  lib/workflow/
    engine.ts      WorkflowEngine — the entire runtime (state machine + persistence)
    types.ts       Pure metamodel (definition-time + runtime types)
    expressions.ts expr-eval wrapper for decision conditions
    db.ts          neon() client factory + exported singleton `engine`
  scripts/schema.sql         6-table Postgres schema (partitioned event log)
  scripts/seed-loan-approval.sql  JSONB definition for the loan demo
  lib/forms/                  JSON-Schema (rjsf) task forms — DEMO ONLY
  app/ + components/workflow/ DEMO UI (Next.js App Router) — DEMO ONLY
```

The boundary is documented in
[`workflow_engine/ARCHITECTURE_BOUNDARY.md`](../workflow_engine/ARCHITECTURE_BOUNDARY.md):
engine runtime is generic; definitions/model are logically separate; the
CulebraLuxe bridge lives in [`workflow_app/`](../workflow_app/).

---

## 2. Execution trace (kernel trace)

Public entry points of [`WorkflowEngine`](../workflow_engine/lib/workflow/engine.ts:19)
and their call paths:

### `startProcess` — [`engine.ts:25`](../workflow_engine/lib/workflow/engine.ts:25)
Transaction: **one `sql.begin`** wraps everything.

1. [`_loadDefinition`](../workflow_engine/lib/workflow/engine.ts:875) — resolves
   `(key, version, tenant)`; explicit `version` pins it, otherwise latest `active`.
2. Validate `graph.startNodeId` exists.
3. INSERT [`process_instances`](../workflow_engine/lib/workflow/engine.ts:46)
   (`status='active'`, `variables=JSON.stringify(variables)`).
4. INSERT root [`tokens`](../workflow_engine/lib/workflow/engine.ts:61) at start node.
5. UPDATE `process_instances.root_token_id`.
6. INSERT [`process.started`](../workflow_engine/lib/workflow/engine.ts:80) event.
7. [`_executeNodeLeave`](../workflow_engine/lib/workflow/engine.ts:564) on the
   start node (immediately leaves it).

### `signalToken` — [`engine.ts:129`](../workflow_engine/lib/workflow/engine.ts:129)
Transaction: **one `sql.begin`**.
1. `SELECT ... FOR UPDATE` token; assert `active`.
2. `SELECT ... FOR UPDATE` instance; assert `active`.
3. Load definition by `instance.definitionId` (pinned version).
4. Shallow-merge `variables` into instance and bump `version`.
5. [`_executeNodeLeave`](../workflow_engine/lib/workflow/engine.ts:165).

### `claimTask` — [`engine.ts:179`](../workflow_engine/lib/workflow/engine.ts:179)
Transaction: **one `sql.begin`**.
1. `FOR UPDATE` task; status must be `ready`/`reserved`.
2. `canClaim = assignee===user || user∈candidates || candidates empty`.
3. Guarded UPDATE (`version = task.version`) → `reserved`, set assignee.
4. INSERT [`task.claimed`](../workflow_engine/lib/workflow/engine.ts:210) event.

### `releaseTask` — [`engine.ts:226`](../workflow_engine/lib/workflow/engine.ts:226)
Transaction: **one `sql.begin`**. Symmetric to claim; only the assignee;
`reserved`/`in_progress` → `ready`, clear assignee; `task.released` event.

### `completeTask` — [`engine.ts:263`](../workflow_engine/lib/workflow/engine.ts:263)
Transaction: **one `sql.begin`**.
1. `FOR UPDATE` task; status `ready`/`reserved`/`in_progress`; assignee check.
2. Guarded UPDATE → `completed`, merge `form_data`, set `completed_at/by`.
3. INSERT [`task.completed`](../workflow_engine/lib/workflow/engine.ts:289) event.
4. If `task.tokenId`: `FOR UPDATE` token (assert `active`), `FOR UPDATE`
   instance, load definition; merge `formData` into variables plus a
   `task_${name}_result` key; bump instance version; then
   [`_executeNodeLeave`](../workflow_engine/lib/workflow/engine.ts:334).

### `claimJobs` — [`engine.ts:349`](../workflow_engine/lib/workflow/engine.ts:349)
Transaction: **single atomic statement** (no explicit `begin`). One
`UPDATE ... WHERE id IN (SELECT ... FOR UPDATE SKIP LOCKED) RETURNING *`
flips `pending`→`locked`, sets a 5-minute lease, increments `attempts`.

### `completeJob` / `failJob` — [`engine.ts:371`](../workflow_engine/lib/workflow/engine.ts:371), [`engine.ts:407`](../workflow_engine/lib/workflow/engine.ts:407)
Transaction: **one `sql.begin` each**. `FOR UPDATE` job; verify lease owner.
`failJob` applies exponential backoff `now() + 1min * 2^attempts` and flips to
`failed` once `attempts >= max_attempts`.

### `createJob` — [`engine.ts:449`](../workflow_engine/lib/workflow/engine.ts:449)
Transaction: **single INSERT** (no `begin`). Manual timer creation.

### `_executeNodeLeave` dispatch — [`engine.ts:564`](../workflow_engine/lib/workflow/engine.ts:564)
The recursive heart of the engine:

- `end` node or no transitions → [`_completeToken`](../workflow_engine/lib/workflow/engine.ts:662)
  + [`_checkProcessCompletion`](../workflow_engine/lib/workflow/engine.ts:685).
- `decision` → [`_evaluateDecision`](../workflow_engine/lib/workflow/engine.ts:762)
  → [`_moveToken`](../workflow_engine/lib/workflow/engine.ts:633) → **recurse**.
- `fork` → [`_handleFork`](../workflow_engine/lib/workflow/engine.ts:780).
- `join` → [`_handleJoin`](../workflow_engine/lib/workflow/engine.ts:828).
- otherwise (`start`/`task`/`state`/`subprocess`/custom) → select transition
  (first, or `preferredTransition` by name) → `_moveToken`; if the target node
  is type `task`, create the human gate via
  [`_createHumanTask`](../workflow_engine/lib/workflow/engine.ts:717) and **stop
  (wait for a human)**; otherwise re-read the token and **recurse**.

### `_moveToken` — [`engine.ts:633`](../workflow_engine/lib/workflow/engine.ts:633)
Guarded UPDATE `node_id`, `version+1` (`WHERE version = token.version`) + a
`token.moved` event. **The UPDATE affected-row count is not checked** — see §7.

### `_handleFork` — [`engine.ts:780`](../workflow_engine/lib/workflow/engine.ts:780)
Complete the parent token; for **each** outgoing transition insert a child token
(`parent_token_id` = parent token id), emit `token.forked`, then recurse
`_executeNodeLeave` on each child. Unconditional fan-out.

### `_handleJoin` — [`engine.ts:828`](../workflow_engine/lib/workflow/engine.ts:828)
Complete the arriving token; count remaining **active** siblings sharing
`parent_token_id`; if any remain, wait. When the last sibling arrives, take the
join node's first transition and insert one new token whose
`parent_token_id` = the original fork parent (one level up), then recurse.

---

## 3. Metamodel summary (types)

Definition-time types (immutable once seeded):

- [`ProcessDefinition`](../workflow_engine/lib/workflow/types.ts:16) — key,
  version, `definition: ProcessGraph`, status `draft|active|deprecated`.
- [`ProcessGraph`](../workflow_engine/lib/workflow/types.ts:30) — `nodes`
  map + `startNodeId`.
- [`NodeDefinition`](../workflow_engine/lib/workflow/types.ts:35) — id, `type`
  (`start|end|task|decision|fork|join|state|subprocess|string`), transitions,
  `formKey`, `candidateGroups`, `priority`, `decisions[]`, `subprocessKey`,
  `inputMappings`.
- [`TransitionDefinition`](../workflow_engine/lib/workflow/types.ts:49) —
  `name`, `to`, `condition?`.

Runtime types:

- [`ProcessInstance`](../workflow_engine/lib/workflow/types.ts:55) — status,
  `definition_id` (pinned), `businessKey`, `variables` (jsonb), `version`.
- [`Token`](../workflow_engine/lib/workflow/types.ts:72) — hierarchical
  (`parent_token_id`), `node_id`, status `active|completed|suspended`,
  `is_able_to_reactivate_parent` (declared, **never used**).
- [`Task`](../workflow_engine/lib/workflow/types.ts:87) — status enum, assignee,
  `candidates[]`, `swimlane`, `priority`, `due_date`, `form_key`, `form_data`,
  `version`.
- [`Job`](../workflow_engine/lib/workflow/types.ts:110) — `type`
  (`timer|async|message|signal`), `due_at`, status, lease, attempts, payload.

Status enums declared but **never produced by the engine**:
`ProcessStatus.suspended|aborted|error`, `TokenStatus.suspended`,
`TaskStatus.failed|exited|obsolete`, `JobStatus.cancelled`.

---

## 4. Persistence map

Six tables in [`schema.sql`](../workflow_engine/scripts/schema.sql:1).

| Table | Role | Key constraints / indexes |
|---|---|---|
| [`process_definitions`](../workflow_engine/scripts/schema.sql:7) | DEFINITION (versioned, immutable-in-practice) | PK `id`; `UNIQUE(tenant_id, key, version)`; idx `(tenant_id, key)`; status CHECK |
| [`process_instances`](../workflow_engine/scripts/schema.sql:26) | RUNTIME case | PK `id`; FK `definition_id`→definitions; FK `parent_instance_id` self; partial idx `status='active'`; idx `(tenant_id, business_key)`; `version` optimistic |
| [`tokens`](../workflow_engine/scripts/schema.sql:50) | RUNTIME execution pointer (hierarchical) | PK `id`; FK `process_instance_id` CASCADE; self FK `parent_token_id`; partial idx `(process_instance_id, status) WHERE active`; `version` |
| [`tasks`](../workflow_engine/scripts/schema.sql:71) | HUMAN TASK | PK `id`; FK instance CASCADE; FK `token_id`; GIN idx on `candidates`; partial idx `(assignee,status)`, `(due_date)`; `version` |
| [`jobs`](../workflow_engine/scripts/schema.sql:105) | TIMER/JOB queue | PK `id`; FK instance CASCADE; FK token; partial idx `(due_at) WHERE pending`, `(locked_until) WHERE locked`; attempts/max_attempts |
| [`process_events`](../workflow_engine/scripts/schema.sql:130) | EVENT/AUDIT (append-only) | `bigserial id`, **partitioned by range(created_at)**; PK `(id, created_at)`; idx `(process_instance_id, created_at)`, `(event_type, created_at)` |

Optimistic `version` columns exist on `process_instances`, `tokens`, `tasks`
(and the [`set_updated_at()`](../workflow_engine/scripts/schema.sql:157) trigger
touches `updated_at`). `jobs` uses a lease (`locked_by`/`locked_until`) rather
than a version column.

---

## 5. Definition language

- **Representation:** pure **JSONB** stored in `process_definitions.definition`
  as a `ProcessGraph`; seeded by SQL
  ([`seed-loan-approval.sql`](../workflow_engine/scripts/seed-loan-approval.sql:1)).
  No code-gen, no separate DSL file format.
- **Nodes:** JSON objects keyed by `id` in the `nodes` map
  ([`seed-loan-approval.sql:9`](../workflow_engine/scripts/seed-loan-approval.sql:9)),
  with `type`, `name`, `transitions`, and task-specific `formKey` /
  `candidateGroups` / `priority`.
- **Transitions:** arrays of `{ name, to }`
  ([`seed-loan-approval.sql:13`](../workflow_engine/scripts/seed-loan-approval.sql:13)).
- **Conditions:** decision nodes carry a `decisions[]` list of
  `{ condition: string, transition: string }`
  ([`seed-loan-approval.sql:30`](../workflow_engine/scripts/seed-loan-approval.sql:30)),
  evaluated by expr-eval. **Note:** `TransitionDefinition.condition` exists in
  the type but is **ignored by the engine** — only `node.decisions[].condition`
  is evaluated.
- **Human forms:** referenced by `formKey` string; resolved **in the demo app**
  via [`formRegistry`](../workflow_engine/lib/forms/index.ts:5) (rjsf JSON Schema).
  The engine itself only stores the `form_key`.
- **Versioning:** integer `version`, `UNIQUE(tenant_id, key, version)`, status
  `draft|active|deprecated`. Instances pin `definition_id`.
- **Parsing/validation:** **none.** `definition.definition as ProcessGraph` is a
  blind cast; there is no JSON-Schema validation, no node/transition referential
  integrity check, no parse step. Malformed graphs fail at runtime with generic
  `Error`s and roll back the transaction.

---

## 6. Expression engine

[`expressions.ts`](../workflow_engine/lib/workflow/expressions.ts:9) wraps
`expr-eval`:

- Supported: arithmetic, comparison, logical (`&&`, `||`, `!`), ternary,
  string/number literals, member access on the variables object.
- Variable access: direct identifiers from the `variables` object
  (`amount > 100000` reads `variables.amount`).
- Comparison/logical: `==`, `!=`, `>`, `<`, `>=`, `<=`, `&&`, `||`, `!`.
- Missing/null behavior: expr-eval throws on undefined identifiers; the wrapper
  catches **any** error, logs it, and returns `false` — i.e. a malformed or
  missing-variable condition silently evaluates to `false` (not an error, not
  `true`).
- Security: `expr-eval` is a safe parser (no `eval`, no function/prototype
  access, no arbitrary code execution). Expressions **cannot call arbitrary
  code**. This is appropriate.
- Determinism: pure evaluation over the variables object; deterministic for a
  given input. But "condition fails → `false`" is an important silent-failure
  mode: a typo in a condition becomes "take the fallback transition," not a
  loud failure.
- Brokerage fit: simple scalar decisions (`amount`, `stage`, `status`) fit
  naturally. Anything needing date math, list membership beyond simple equality,
  or cross-record lookups would have to be pre-computed into variables by the
  application fact projection.

---

## 7. Transaction semantics

| Operation | Atomic? | Mechanism |
|---|---|---|
| `startProcess` | ✅ one tx | full `begin` wraps instance+token+event+leave |
| `signalToken` (transition) | ✅ one tx | `begin` + `FOR UPDATE` + recursive leave |
| `completeTask` (transition) | ✅ one tx | `begin` + `FOR UPDATE` + leave |
| `claimTask` / `releaseTask` | ✅ one tx | `begin` + `FOR UPDATE` |
| `claimJobs` | ✅ one statement | single `UPDATE ... RETURNING` (SKIP LOCKED) |
| `completeJob` / `failJob` | ✅ one tx | `begin` + `FOR UPDATE` |
| `createJob` | ✅ one statement | single INSERT |
| fork | ✅ inside caller tx | `_handleFork` runs in the surrounding `begin` |
| join | ✅ inside caller tx | `_handleJoin` runs in the surrounding `begin` |
| event append | ✅ with owning mutation | events are INSERTs inside the same tx |

**Gaps (do not fix yet):**

1. `_moveToken` does **not check the UPDATE row count** of its optimistic
   `WHERE version = token.version`. A stale move would still emit a
   `token.moved` event and continue recursing on a re-read token.
2. `completeTask` bumps `process_instances.version` **without an optimistic
   guard** and without checking the affected-row count.
3. `_completeToken` has **no version guard at all** (relies solely on the
   caller having locked the token).
4. There is **no compensating/rollback concept**; every failure is a thrown
   `Error` → full transaction rollback, leaving the process in its prior state
   but with **no error record** (§11).

---

## 8. Concurrency semantics

| Scenario | Classification | Evidence |
|---|---|---|
| Two humans completing the same task | **STRONG** | `completeTask` `FOR UPDATE`s the task; second tx sees `completed` and throws |
| Two workers claiming the same timer | **STRONG** | `claimJobs` uses `FOR UPDATE SKIP LOCKED` in one statement |
| Concurrent transitions on the same token | **STRONG** | both `signalToken`/`completeTask` `FOR UPDATE` the token first |
| Duplicate retry of the same command | **WEAK** | no `commandId`/idempotency key anywhere; a replayed `completeTask` throws rather than returning success |
| Stale token execution | **WEAK** | `_moveToken` ignores the optimistic-update affected-row count; a stale move still writes a `token.moved` event |
| Worker lease expiry (locked job) | **WEAK** | `locked_until` is written but there is **no requeue** of expired leases |

Version columns + `FOR UPDATE` give a solid single-node foundation; the weak
spots are the unchecked optimistic guards and the absence of command-level
idempotency (which the Ogden contract explicitly assigns to the application
side — class C/D in [`command-inventory.ts`](../lib/workflow/command-inventory.ts:25)).

---

## 9. Human task semantics

What an engine task **is**: a human gate anchored to a token.

- **Assignment:** `candidateGroups[]` is a flat string array with no
  user/group distinction; the demo mixes user ids (`"john.doe"`) and role-ish
  names (`"credit-officers"`).
- **Claim:** `claimTask` allows the assignee, any candidate, or **anyone when
  candidates is empty**.
- **Release:** assignee-only; `reserved`/`in_progress` → `ready`.
- **Completion:** allowed from `ready`/`reserved`/`in_progress`; if `assignee`
  is null (unclaimed) the assignee check passes for **any** userId — a user can
  complete an unclaimed task without claiming it.
- **Input/form data:** `form_data` jsonb, merged on completion; `form_key`
  resolves to an rjsf form **only in the demo UI**.
- **Output data:** merged into instance variables plus a
  `task_${name}_result` convention key.
- **Cancellation:** **no API**; `exited`/`obsolete`/`failed` statuses are never
  set.
- **Ownership:** assignee is a free-form string; there is no `app_user` FK
  (the engine has no user/identity table).

Versus the CulebraLuxe operational task: the engine task is **not** the
user-facing task. The Ogden [`TaskCorrelation`](../lib/workflow/operational-contracts.ts:10)
explicitly keeps CulebraLuxe `task` canonical and correlates. The engine task
currently lacks: due-date enforcement, swimlane semantics (column exists,
never set), and authority/role gating (only free-string candidates).

---

## 10. Timer / job semantics

- **Scheduled time:** `due_at` (timestamptz), claimed only when `due_at <= now()`.
- **Retries:** `attempts`/`max_attempts`; claim increments `attempts`; `failJob`
  schedules retry with **exponential backoff** `1min * 2^attempts`.
- **Backoff:** yes (exponential, capped by max_attempts).
- **Claiming:** `claimJobs` — lease 5 minutes, `FOR UPDATE SKIP LOCKED`.
- **Cancellation:** **no API** (status `cancelled` declared, never set).
- **Reschedule:** **no API** to move `due_at` on an existing job.
- **Exhausted retries:** `failJob` → `failed` once `attempts >= max_attempts`
  (emits `job.failed`).
- **Process correlation:** `process_instance_id` + `token_id` columns exist.
- **Critical:** nothing in the engine **creates** jobs from a process
  definition (no `timer` node type), and `completeJob` only marks the job done
  + writes an event — it **does not advance any token**. A deadline timer
  cannot, by itself, move a process forward; an external worker would have to
  `completeJob` then `signalToken`.

For brokerage deadlines/escalations this is a **PARTIAL**: the reliable queue
is real, but definition-level timer binding and auto-advance are missing.

---

## 11. Fork / join semantics

- **Fork:** unconditional fan-out over every outgoing transition; children share
  `parent_token_id`.
- **Join:** AND-join by counting remaining `active` siblings; the last arrival
  spawns the continuation token.

The `under_contract → inspection|appraisal|financing|title → ready_to_close`
shape:

| Need | Fit |
|---|---|
| AND fork (spawn all) | ✅ SUPPORTED |
| AND join (wait all complete) | ✅ SUPPORTED (positive branches only) |
| Partial completion (some done, some pending) | ✅ SUPPORTED (waiting is the natural state) |
| Cancelled branch (should not block) | ❌ MISSING — no "cancelled" token state |
| Failed branch (should abort or bypass) | ❌ MISSING — `_completeToken` always marks `completed` |
| Optional branch (may or may not spawn) | ❌ MISSING — fork fans out unconditionally |
| Join correlation (which join waits for which fork) | ⚠️ PARTIAL — correlation is purely `parent_token_id`; nested/overlapping fork-join pairs are fragile |

The `is_able_to_reactivate_parent` column is a jBPM carryover that is **never
read or written by the engine**.

---

## 12. Error / terminal semantics

- **Normal completion:** `_checkProcessCompletion` sets instance `completed`
  when zero active tokens remain.
- **Explicit error:** **missing.** There is no `error` transition, no
  `process.error` event, no way to mark an instance `error`. A thrown Error
  rolls back the transaction and leaves the instance silently `active`.
- **Failed job:** exists (`job.failed`) but does **not** propagate to the
  process instance (the instance stays `active`).
- **Invalid transition:** throws → rollback, no record.
- **Conflict:** **missing** as a concept. Conflicts are thrown `Error`s with
  string messages (no typed `ConflictReason` — that exists only in
  [`contracts.ts`](../lib/workflow/contracts.ts:95), application-side).
- **Cancelled process:** **missing.** `aborted`/`suspended`/`error` statuses
  are declared but never produced.
- **Terminal business outcome:** encoded only in the **name of an `end` node**
  (`"Approved"` vs `"Rejected"`) and in the `process.started`/`token.*` events —
  the instance status is always just `completed`. There is no END-SUCCESS vs
  END-ERROR vs END-CONFLICT distinction.

Deliberate END / ERROR / ALERT / CONFLICT concepts discussed earlier are
**not present** in the engine today.

---

## 13. Event history

- **Producers:** `startProcess` (`process.started`), `_moveToken`
  (`token.moved`), `_completeToken` (`token.completed`), `_checkProcessCompletion`
  (`process.completed`), `_createHumanTask` (`task.created`), `claimTask`
  (`task.claimed`), `releaseTask` (`task.released`), `completeTask`
  (`task.completed`), `completeJob` (`job.completed`), `failJob`
  (`job.retry_scheduled`/`job.failed`), `_handleFork` (`token.forked`).
- **Immutability:** append-only by convention; no UPDATE/DELETE path in code.
  (No DB-level `FOR UPDATE`/trigger prevents manual mutation, but nothing in the
  engine mutates events.)
- **Actor:** free-string `actor` column.
- **Ordering:** `created_at` + `bigserial id`; partitioned by `created_at`.
- **Correlation:** via `process_instance_id`/`token_id`/`task_id`/`job_id`
  columns. There is **no** `eventId`/`correlationId`/`causationId` matching the
  application [`DomainEvent`](../lib/workflow/contracts.ts:40).
- **Reconstruction:** sufficient to reconstruct *token movement and task
  lifecycle*, but `_handleJoin` emits **no join event** (a gap), and variable
  changes are not individually evented.
- **Nature:** these are **framework events**, not business-domain truth. They
  must not be confused with CulebraLuxe `DomainEvent`s.

---

## 14. Versioning

- [`_loadDefinition`](../workflow_engine/lib/workflow/engine.ts:875) resolves a
  specific `version` when supplied; `startProcess` stores `definition_id` (the
  row FK), and every later load (`signalToken`, `completeTask`) reads by
  `instance.definitionId`.
- **A running instance is pinned to the definition version it started with.**
  Changing the seed/definition for new instances does not affect running ones.
  **Classification: STRONG (pinned).**
- **Missing:** there is **no migration/upgrade path** — no way to move an
  active instance to a newer definition version (no instance version bump, no
  re-pointing of `definition_id`, no mid-flight migration policy).

---

## 15. Variable model

- **Storage:** single `variables` jsonb on `process_instances`.
- **Typing:** untyped (`Record<string, any>`); no schema for variables.
- **Mutation:** shallow merge in `signalToken` (params) and `completeTask`
  (formData + `task_${name}_result`).
- **Scope:** process-wide only; no per-token/per-task variable scope.
- **Merge behavior:** last-writer-wins shallow merge; no conflict detection.
- **Concurrency:** serialized by the transaction `FOR UPDATE` on the instance;
  safe but no merge strategy beyond overwrite.
- **Business truth usage:** the demo uses variables (`amount`, `customerId`) as
  decision inputs — acceptable for the demo, but per the boundary, CulebraLuxe
  business truth (deals/offers/people) must stay in CulebraLuxe. Engine
  variables should carry **orchestration facts only** (a compact projection
  like [`DealWorkflowFacts`](../lib/workflow/adapter.ts:57)), not shadow copies
  of domain records.

---

## 16. Loan demo decomposition

| Layer | Component |
|---|---|
| **ENGINE CAPABILITY** | tokens, transitions, decision evaluation, human task create/claim/complete, end nodes, events |
| **DEFINITION/MODEL** | JSONB graph in [`seed-loan-approval.sql`](../workflow_engine/scripts/seed-loan-approval.sql:7) — `start → credit_review → amount_decision → senior_review/funding → end` |
| **DEMO BUSINESS LOGIC** | `amount` variable drives the decision; form field `decision` is mapped to `transitionName` (`approve`/`reject`) in [`TaskForm.tsx`](../workflow_engine/components/workflow/TaskForm.tsx:45) |
| **UI/FORM** | [`StartProcessForm`](../workflow_engine/components/workflow/StartProcessForm.tsx) (hardcoded loan fields), [`TaskForm`](../workflow_engine/components/workflow/TaskForm.tsx) (rjsf), dashboard/task pages |

**Reusable patterns (domain-neutral):**
- `candidateGroups` as a role gate.
- `decision` node + expr conditions for routing.
- `preferredTransition` driven by form data (a human choosing the next edge).
- `end` node naming as a terminal-outcome label.

**Do not carry forward:** the loan-specific form fields, the
`amount`/`customerId` start variables, and the implicit assumption that a task's
`decision` enum equals a transition name.

---

## 17. Brokerage capability matrix

| Capability | Score | Evidence |
|---|---|---|
| Linear states | SUPPORTED | `state`/`task` nodes + transitions |
| Conditional transitions | SUPPORTED | `decision` node + expr-eval |
| Human approval | SUPPORTED | `task` node gate |
| Human task assignment | SUPPORTED (ENG-13) | `candidateGroups` free strings; claim/release/reassign/complete lifecycle with locking + version guards (`workflow_engine/docs/human-task-lifecycle.md`); no user FK (identity/authorization app-owned by design) |
| Deadlines | PARTIAL | `jobs.due_at` exists; no definition-level binding |
| Timers | PARTIAL | reliable queue; no auto-advance into a process |
| Retries | SUPPORTED | `failJob` exponential backoff |
| Parallel milestones | SUPPORTED | fork fan-out |
| Join / wait-all | SUPPORTED | AND-join (positive branches only) |
| Optional milestone | MISSING | fork fans out unconditionally |
| Cancellation | MISSING | no cancel op; `cancelled` status unused |
| Explicit terminal success | PARTIAL | `end` node + `completed` status; no outcome enum |
| Explicit terminal failure | MISSING | no `error`/`aborted` production |
| Blocker / wait state | PARTIAL | only via a human task node |
| External SME responsibility | PARTIAL | `candidateGroups` only |
| Actor attribution | PARTIAL | free-string `actor`/`assignee`; no `app_user` FK |
| Audit history | PARTIAL | framework events; no `DomainEvent` shape; no join event |
| Idempotent command execution | MISSING | no `commandId`/dedupe |
| Application command callback | MISSING | `completeTask` stores form data; never calls `executeCommand` |
| Application fact refresh | MISSING | no `readFacts`; variables static unless passed in |
| Process definition versioning | SUPPORTED | `(key, version)` unique + instance pinning |
| Process migration / version upgrade | MISSING | no re-pointing of active instances |

---

## 18. CRM-14 torture model (mapping only, no implementation)

`OFFER ACCEPTED → CONTRACT PREPARATION → CONTRACT EXECUTED → UNDER CONTRACT →`
`{inspection, appraisal, financing, title} → resolve blockers → ready to close →`
`closing → closed`.

- **Linear stages (offer→contract→under-contract→ready→closing→closed):**
  fits naturally as `state`/`task` nodes with named transitions.
- **Decision branches (offer dies before contract, etc.):** fits as `decision`
  nodes or `preferredTransition` from task completion.
- **Parallel under-contract work:** fits the fork/join **for the happy path**
  (all four complete). Inspection/financing/appraisal/title failures that must
  abort or bypass the join **do not fit** without engine extension (no
  cancelled/failed branch state, no join discrimination).
- **Deadline expires / escalation:** needs a definition-level timer node plus a
  timer→transition binding — **missing today** (jobs are a manual side queue).
- **Buyer/seller cancellation:** needs a `cancel`/`abort` operation — **missing**.
- **Manual override:** `signalToken` can force a named transition **only** if
  that token sits at a node whose transition set includes the name; there is no
  global jump/override — **partial**.
- **Terminal error/conflict:** needs END-ERROR / CONFLICT outcomes — **missing**
  (only a generic `completed` + end-node name).

---

## 19. Ogden contract comparison

| Contract element | Classification |
|---|---|
| `DomainEvent` / `CommandEnvelope` / `CommandResult` ([`contracts.ts`](../lib/workflow/contracts.ts:40)) | **ENGINE EXTENSION REQUIRED** — engine has `process_events`, not commands/results |
| `ApplicationFacade` / `WorkflowAdapter` ([`adapter.ts`](../lib/workflow/adapter.ts:18)) | **ENGINE EXTENSION REQUIRED** — engine has no callback surface; `workflow_app` would wrap it |
| `DealWorkflowFacts` projection ([`adapter.ts`](../lib/workflow/adapter.ts:57)) | **APPLICATION RESPONSIBILITY** — engine just consumes variables |
| `TaskCorrelation` ([`operational-contracts.ts`](../lib/workflow/operational-contracts.ts:10)) | **THIN ADAPTER** — engine task needs an `application_task_id` correlation |
| `TimerSpec` / `TimerExpiration` ([`operational-contracts.ts`](../lib/workflow/operational-contracts.ts:23)) | **THIN ADAPTER + EXTENSION** — `due_at` maps to `deadlineAt`; reschedule/cancel missing |
| `AlertEvent` ([`operational-contracts.ts`](../lib/workflow/operational-contracts.ts:56)) | **ENGINE EXTENSION REQUIRED** (or application responsibility via events) |
| `SmeParticipant` ([`operational-contracts.ts`](../lib/workflow/operational-contracts.ts:78)) | **THIN ADAPTER** — map to `candidateGroups` |
| `publishApplicationEvent` | **ENGINE EXTENSION** — bridge framework events ↔ `DomainEvent` |

---

## 20. KEEP / EXTEND / REPLACE / DROP

| Subsystem | Classification | Rationale |
|---|---|---|
| Execution loop (`_executeNodeLeave`) | KEEP + HARDEN | Correct recursive dispatch; add affected-row checks on optimistic updates |
| Types / metamodel | KEEP AS-IS | Clean definition/runtime split; enough expressiveness |
| Expressions (expr-eval) | KEEP AS-IS | Safe, deterministic; note silent-false on error |
| Persistence schema | KEEP + HARDEN | Sound, partitioned events; add join event, consider version guard on `_completeToken` |
| Tokens | KEEP + HARDEN | Core value; drop/repurpose `is_able_to_reactivate_parent` (unused) |
| Tasks | KEEP + EXTEND | Add correlation + due-date handling; enforce claim-before-complete |
| Jobs / timers | KEEP + EXTEND | Reliable queue; add reschedule/cancel + timer→transition binding |
| Fork / join | KEEP + EXTEND | Add branch outcome (cancelled/failed) + join discrimination + join event |
| Events | KEEP + EXTEND | Add join event; bridge to `DomainEvent` at the adapter boundary |
| Variables | KEEP AS-IS | Unstructured orchestration facts are fine; keep business truth out |
| Forms (rjsf) | DEMO ONLY | Not an engine concern; form ownership belongs to `workflow_app`/Portal |
| Demo UI (app/, components/) | DEMO ONLY | Reference only; do not carry into CulebraLuxe |
| Loan example | DEMO ONLY | Extract the reusable patterns (§16), drop loan-domain specifics |
| DB access layer (`db.ts`) | KEEP AS-IS | `neon()` singleton is adequate; move to injection when bridged |

---

## 21. Exact engine gaps preventing CRM-14 V1

1. **No terminal outcome distinction** — only `completed`; no END-SUCCESS vs
   END-ERROR vs CONFLICT, no `aborted`/`error` production.
2. **No cancellation / abort operation** on instances or tokens.
3. **No join branch semantics** — cannot express "cancelled/failed branch must
   not block the join" or "optional branch."
4. **No definition-level timer node** and **no timer→transition binding** —
   deadlines cannot auto-advance a process.
5. **No application callback / fact refresh** — `completeTask` never invokes
   `executeCommand`/`readFacts` (per [`adapter.ts`](../lib/workflow/adapter.ts:28)).
6. **No idempotent command execution** — no `commandId` dedupe (assigned to the
   application side, but the engine must at least tolerate/replay safely).
7. **No typed conflict / precondition result** — conflicts are string-thrown
   `Error`s, not [`CommandResult`](../lib/workflow/contracts.ts:82)-shaped.
8. **Unchecked optimistic guards** — `_moveToken` ignores affected-row count
   (stale-token risk).
9. **No process migration** for running instances across definition versions.

---

## 22. Overall verdict

**B. ENGINE SUFFICIENT WITH SMALL EXTENSIONS.**

The token/transition/decision/human-task/fork-join/event core and the reliable
SKIP-LOCKED job queue are sound, reusable, and well-matched to jBPM-style
brokerage orchestration. The gaps that block CRM-14 V1 are bounded and mostly
fall into three buckets:

- **Engine extensions (small):** terminal outcome enum, cancel/abort, join
  branch outcomes, timer-node binding, checked optimistic guards, join event.
- **Bridge work (`workflow_app`, per Ogden contract):** `executeCommand` /
  `readFacts` adapter, task correlation, DomainEvent bridge, command idempotency.
- **Policy/model (definitions, not engine):** the actual under-contract graph,
  deadlines, SME responsibilities — which the architecture already assigns to
  `workflow_app`.

The engine is **not** fundamentally wrong for this job; it is a deliberately
small, preservable core that needs a focused, bounded set of extensions plus the
application-side adapter before CRM-14 V1 can be implemented.
