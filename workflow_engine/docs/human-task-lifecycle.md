# Generic Persisted Human-Task Lifecycle (ENG-13)

Formalization of the workflow engine's generic human-task runtime: the
persisted state machine, the operations that move between states, the
concurrency/race guarantees, and the boundary between engine mechanics and
application authority.

## Boundary

- **Engine owns task runtime/assignment mechanics**: state transitions,
  row locking, version compare-and-set, audit events, terminal cleanup.
- **Application owns identity and authorization**: the engine only records the
  actor/assignee strings it is handed (`claimTask(taskId, userId)`,
  `reassignTask(taskId, newAssignee, actor)`, `completeTask({ userId })`).
  "May this actor claim/reassign/complete this task?" is answered by the
  embedding application before it calls the engine. The engine validates only
  task-level mechanics: status, current assignment, candidate eligibility.
- The engine is domain-neutral (`workflow_engine/ARCHITECTURE_BOUNDARY.md`);
  it never resolves `candidateGroups` or `swimlane` to real participants.

## State machine

```
                 claim / reassign
   ready  ───────────────────────────────▶  reserved  ──▶  in_progress (future)
     ▲                                        │  │
     └────────────── release ◀────────────────┘  └──▶  complete ─▶  completed
                                                        (terminal)

   ready / reserved / in_progress ── process termination ──▶  obsolete (terminal)
```

- `ready` — unclaimed; any candidate (or anyone, when `candidates` is empty)
  may claim, or a manager may reassign (pre-assignment).
- `reserved` — claimed by `assignee` (`claimed_at` set). Only the assignee may
  release it (back to `ready`) or complete it; a reassign transfers it to a
  new assignee (still `reserved`, `claimed_at` refreshed).
- `completed` — terminal success; the linked token advances exactly once.
- `obsolete` — terminal cleanup: set by process termination
  (`cancelProcess`, terminal end-node outcomes, branch-skip at join release)
  for every task still open at the time the process becomes terminal. An
  obsolete task is never actionable.

## Operations and deterministic conflicts

Every operation runs in one transaction with the ENG-11 lock discipline:
lock the owning **instance row first**, then lock and re-read the **task row**,
then mutate with a **version compare-and-set whose affected row is verified**
(`RETURNING id`). A mutation that matches no row aborts with `STALE_TASK`
before any event is recorded — an operation never "succeeds" silently and
never emits an event for a mutation that did not take effect.

| Operation | Allowed when | Conflict (code) |
|---|---|---|
| `claimTask(taskId, userId)` | status `ready`/`reserved`, user is candidate (or open task), not already assigned to another user, instance active | `TASK_NOT_CLAIMABLE`, `TASK_CANDIDATE_ONLY`, `TASK_ALREADY_ASSIGNED`, `PROCESS_NOT_ACTIVE`, `STALE_TASK` |
| `releaseTask(taskId, userId)` | `userId` is the assignee, status `reserved`/`in_progress`, instance active | `TASK_ASSIGNEE_ONLY`, `TASK_NOT_RELEASABLE`, `PROCESS_NOT_ACTIVE`, `STALE_TASK` |
| `reassignTask(taskId, newAssignee, actor)` | status `ready`/`reserved`/`in_progress`, `newAssignee` is a candidate (or open task), instance active | `TASK_ALREADY_COMPLETED`, `TASK_NOT_REASSIGNABLE`, `TASK_CANDIDATE_ONLY`, `PROCESS_NOT_ACTIVE`, `STALE_TASK` |
| `completeTask({ taskId, userId, ... })` | status `ready`/`reserved`/`in_progress`, `userId` is the assignee when one is set, instance active | `TASK_ALREADY_COMPLETED`, `TASK_NOT_ACTIONABLE`, `TASK_ASSIGNEE_ONLY`, `PROCESS_NOT_ACTIVE`, `STALE_TASK` |

Events: `task.claimed`, `task.released`, `task.reassigned` (data
`{ from, to }`, actor = who performed the reassignment), `task.completed`,
`task.created`, `task.obsoleted`.

## Guarantees (proven on real PostgreSQL)

`workflow_engine/tests/persistence/human-task-lifecycle.test.ts` proves, with
genuinely overlapping transactions against the DEV database:

1. **Claim/release/reassign/complete** behave deterministically and persist
   exact state (status, assignee, timestamps, version, events).
2. **Duplicate completion is a deterministic conflict** (`TASK_ALREADY_COMPLETED`
   before any mutation): sequential and concurrent duplicate completions never
   double-complete, never double-record, and the linked token advances
   **exactly once** (exactly one `task.completed`, one task-node
   `token.moved`, one completed process, one completed end token).
3. **Claim-vs-complete race**: either the claim wins (completion is rejected
   `TASK_ASSIGNEE_ONLY` and nothing advances) or the completion wins (claim is
   rejected) — never both effects, never a double advance.
4. **Completion-vs-cancellation race**: cancellation wins (completion rejected
   on the obsoleted task, no resurrection, no `task.completed`, no token
   move) or completion wins (cancellation rejected `PROCESS_NOT_ACTIVE`) —
   exactly one terminal event, no actionable residue.
5. **No actionable tasks after terminal process state**: after cancellation or
   completion, every task is `obsolete`/`completed`; claim/release/reassign/
   complete on them are deterministic conflicts; the active-task view never
   surfaces them.

## Expiry — deliberately NOT engine-owned

`tasks.due_date` is a stored deadline fact; the engine does **not** auto-expire
tasks. Auto-expiry is deadline policy (reminder/escalation/auto-cancel) and is
application-owned: deadlines and responsibility resolve through the embedding
application (`workflow_app/responsibility.ts`; "no alert delivery" is an
explicit integration-contract non-goal). Auto-expiry in the generic engine
would require a policy-carrying task sweeper the engine must not own. The
application can enforce due-date gates before claim/complete, or cancel the
process. ENG-13 therefore formalizes claim/release/reassign/complete and
terminal cleanup, and records expiry as an explicit, justified deferral.
