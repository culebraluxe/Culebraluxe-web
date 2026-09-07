# CULEBRALUXE WORK ORDER — ENG-FORGE-CONVERGENCE-01

**Title:** Execution Convergence — One Workspace, Candidate Iterations, Real FAST Lane  
**Repository:** `culebraluxe/Culebraluxe-web`  
**Branch:** `main` only  
**Environment:** DEV proof only  
**Work type:** Forge hardening / execution model  
**Test policy:** Targeted only. No broad/full regression unless separately authorized.

---

## 1. Why this work exists

The first real OpenCode/DeepSeek Forge dogfood exposed an execution-model problem that unit/subpath tests did not make obvious:

- one serial story created many `agent_work_item` records;
- the invoker keyed worker workspaces to `workItem.id`;
- Architect, Lead, Smith, QA, repair, and other serial role transitions therefore created separate Git branches/worktrees;
- the operator could see many valid Git states but could not easily tell which state was the active implementation lineage;
- normal serial work began to look like an intentional multi-Smith split even when there was only one Smith;
- Lead/QA inherited unnecessary Git integration complexity;
- the TECH UI still reflects the older V1 run model and does not expose execution lineage, candidate iterations, or convergence.

Commit `78bec0e3` stopped the immediate per-work-item fanout by making serial roles reuse a workspace key. That is an interim hardening step, not the final identity model: the current key is derived from the Forge worker id. The durable execution identity already exists in Forge as the workflow `processInstanceId` and must become the canonical workspace/execution key.

This work order turns the dogfood finding into a simple operating model:

> **A story has a target. One Forge execution works toward that target in one serial workspace. Smith/QA repair cycles create candidate iterations in the same Git lineage. A specification failure starts a new execution generation. Extra writable worktrees exist only for an explicit SPLIT.**

The goal is working code with less orchestration debt, not additional ceremony.

---

## 2. Product / execution terminology

Use these terms in code and UI. Do **not** introduce ML vocabulary such as epoch/batch into the product.

- **Story** — the invariant target / frozen packet intent.
- **Execution** — one Forge attempt to deliver that target.
- **Execution generation** — the current specification generation inside a process. Repairs stay in the generation; a REPLAN advances it.
- **Workspace** — the isolated Git worktree owned by one execution generation.
- **Candidate** — an exact immutable Git SHA produced by a writable implementation role.
- **Iteration** — one distinct candidate SHA and the deterministic QA verdict against that SHA.
- **Repair** — implementation correction against the same target, execution generation, and workspace.
- **Replan** — target/architecture correction; advances execution generation and receives a clean workspace lineage.
- **Converged** — the current exact candidate SHA passed deterministic QA. Converged does not by itself mean deployed or released.
- **SPLIT** — explicit bounded parallel Smith implementation. This is the only normal reason to create additional writable worktrees.

Core distinction:

> **Implementation failure advances the candidate. Specification failure advances the execution generation.**

---

## 3. Required end-state behavior

### 3.1 Normal serial FEATURE path

```text
Story
  ↓
Execution generation E0
  ↓
ONE serial workspace
  ↓
Architect (read/judgment)
  ↓
Lead PRE (read/judgment)
  ↓
Smith → candidate A
  ↓
Lead POST → inspect/integrate same lineage
  ↓
QA exact candidate A
  ├─ PASS → release boundary / completion path
  └─ FAIL implementation defect
          ↓
       Smith repair in SAME workspace
          ↓
       candidate B
          ↓
       QA exact candidate B
```

No normal serial role transition may create a new writable worktree merely because it has a new `agent_work_item` UUID.

### 3.2 Replan path

```text
Execution generation E0
  ↓
QA / Lead detects contract or architecture defect
  ↓
REPLAN
  ↓
replanAttempts increments
  ↓
Execution generation E1
  ↓
NEW serial workspace
  ↓
Architect / Lead / Smith continue against revised target
```

The prior generation and candidate history remain visible and immutable.

### 3.3 Explicit SPLIT path

```text
Execution generation E0 workspace
  ↓
Lead explicitly chooses SPLIT:n
  ├─ Smith child 0 → child workspace
  ├─ Smith child 1 → child workspace
  └─ Smith child n → child workspace
          ↓
      explicit join / Lead POST integration
```

Only `smith_split_work` (or its explicit future equivalent) may fan out writable workspaces. Normal Smith, repair Smith, Architect, Lead, QA, and DEV_OPS transitions must not implicitly fork Git state.

---

## 4. Existing infrastructure to reuse

Do not build a second orchestration system.

Reuse the existing Forge control plane:

- `process_instances` / workflow `processInstanceId` — durable process execution identity.
- `forge_engine_task_execution.process_instance_id` — durable per-node execution lineage.
- `agent_work_item` — durable role work assignment.
- `storyboard_story_run` — frozen run/evidence record, including exact `commit_hash`.
- existing exact-candidate handoff (`smithCandidateSha(...)`).
- existing deterministic QA / Assay adapter.
- existing QA repair policy in `workflow_app/forge/qa-repair-policy.ts`.
- existing durable repair/replan counters and dispositions in `db/forge-repair-ledger.ts`.
- existing Forge TECH cockpit and Story Board surfaces.
- existing worker-workspace provision/recovery safety rules.

### Schema rule

**Prefer no database migration.** The required execution/iteration lineage should be derived from existing durable records first.

A schema change is out of scope unless implementation proves the lineage cannot be represented truthfully with the current `processInstanceId`, engine executions, Story Runs, and repair/replan ledger. If that happens, STOP and return the exact missing invariant rather than adding schema opportunistically.

---

## 5. Scope A — Make execution identity canonical for Git workspaces

### Goal

Replace the interim worker-id workspace ownership with execution-generation ownership.

### Required behavior

1. `workerId` remains the claim/executor identity only. It must not define Git lineage.
2. The serial workspace key must derive from the current Forge workflow `processInstanceId` plus execution generation.
3. Execution generation should derive from durable replan state without introducing a second state machine. Preferred form:

```text
<processInstanceId>-e<replanAttempts>
```

Example:

```text
process 7a2... / replanAttempts 0 → 7a2...-e0
process 7a2... / replanAttempts 1 → 7a2...-e1
```

4. All serial roles in the same execution generation reuse that exact workspace id.
5. Repair Smith reuses the same generation/workspace.
6. REPLAN increments the generation and subsequent writable work receives a new workspace.
7. Explicit split children derive from the execution generation plus a stable child id/index:

```text
7a2...-e0-split-0
7a2...-e0-split-1
```

8. Worktree recovery must preserve existing safety behavior: never reset, clean, rebase, or steal another workspace.
9. Existing historical worktrees are not part of this change. Do not bulk-delete or rewrite them.

### Expected implementation seam

Primary files:

- `agent-runtime/invoker.ts`
- `agent-runtime/invoker-workspace.test.ts`
- `workflow_app/forge/agent-runtime-role-runner.ts`
- focused role-runner test beside it if needed
- worker-workspace helpers only if required to accept the new deterministic execution id

The role runner currently knows the Forge task, including `task.processInstanceId`; pass that durable identity into workspace resolution at execution time rather than freezing workspace identity from `options.workerId` before task context exists.

---

## 6. Scope B — Make candidate iteration and convergence first-class

### Goal

Expose the actual delivery lineage instead of forcing the operator to infer it from raw agent IDs and worktrees.

### Candidate iteration rules

1. A **new persisted candidate SHA** creates the next iteration number.
2. A role run that produces no new SHA does not create a candidate iteration.
3. Retrying QA against the same SHA does not create another candidate iteration.
4. QA verdict always binds to the exact candidate SHA tested.
5. Repair after QA FAIL remains in the same execution generation and creates the next candidate SHA in the same Git lineage.
6. Replan begins a new execution generation; iteration numbering may restart per generation but the UI must retain the full prior history.
7. Lead POST may create a replacement/integration candidate and therefore may advance the candidate iteration if its persisted `commitHash` changes.
8. The read model must never guess candidate ownership or QA verdict from timestamps alone when exact persisted evidence exists.

### Convergence states

Expose a small derived state, not a new workflow engine:

- `ITERATING` — candidate/repair loop is active.
- `CONVERGED` — current exact candidate passed deterministic QA.
- `REPLAN_REQUIRED` — typed QA/Lead evidence says the target/contract must change.
- `HOLD` — operator/dependency/policy/no-progress stop.

Keep normal Story status (`Ready`, `Hold`, `Complete`, etc.) separate from convergence state.

### No-progress guard

Add a deterministic stop against token-burning loops:

> If the same candidate SHA receives the same machine-classified QA failure again without a new candidate in between, do not launch another model repair cycle automatically. Route to `HOLD` with a machine-readable `NO_PROGRESS` reason/evidence.

Do not invent fuzzy semantic similarity. Use exact SHA + existing typed/machine QA evidence/disposition.

### Preferred read-model seam

Create a narrow derived read model, for example:

- `db/forge-convergence.ts`
- `db/forge-convergence.test.ts`

Possible shape:

```ts
type ForgeConvergenceExecution = {
  executionId: string
  storyId: string
  generation: number
  processStatus: string
  currentNode: string | null
  convergenceState: 'ITERATING' | 'CONVERGED' | 'REPLAN_REQUIRED' | 'HOLD'
  repairAttempts: number
  replanAttempts: number
  iterations: ForgeCandidateIteration[]
}

type ForgeCandidateIteration = {
  iteration: number
  candidateSha: string
  producerNodeId: string
  producerRunId: string | null
  qaRunId: string | null
  qaVerdict: 'PASS' | 'FAIL' | 'PENDING'
  qaFailureClass: string | null
  qaDisposition: string | null
  startedAt: string | null
  endedAt: string | null
}
```

Exact names may vary. Keep it a read model over existing durable evidence.

---

## 7. Scope C — Make FAST_LANE actually fast

### Problem

The active `FORGE_SDLC-v3.xml` FAST path still sends bounded work through Lead and then a human `fast_confirmation` gate. That defeats the purpose of a fast lane and makes small, well-specified fixes pay the orchestration cost of the full roster.

### Versioning rule

**Do not edit active historical workflow XML in place.**

Create:

- `workflow_app/definitions/FORGE_SDLC-v4.xml`

Then update:

- `workflow_app/definitions/forge-sdlc.ts`

to make v4 the active definition after targeted tests pass. Preserve v3 unchanged.

### FAST eligibility

FAST may be selected only when all are true:

- packet already contains a usable frozen Architect brief / execution contract;
- acceptance criteria are explicit;
- deterministic QA commands are explicit;
- implementation is a single coherent Smith assignment;
- no unresolved architecture/decomposition question remains;
- `migrationRequired = false`;
- `derivedRefreshRequired = false`;
- `deploymentRequired = false`;
- no schema redesign, provider redesign, or domain-contract redesign is required;
- no intentional SPLIT is required.

If any of those become false at runtime, FAST must fail closed to HOLD/reclassification. It must not silently grow into a release/deployment workflow.

### FAST target path

```text
Ready FAST story
  ↓
Smith
  ↓
exact candidate SHA
  ↓
deterministic QA
  ├─ FAIL implementation defect
  │      ↓
  │   repair Smith — same execution/workspace
  │      ↓
  │   new candidate SHA
  │      ↓
  │   deterministic QA
  │
  └─ PASS
         ↓
     deterministic candidate publish
         ↓
      Complete
```

FAST skips Architect, Lead PRE, Lead POST, and DEV_OPS **model runs** when eligibility is already satisfied.

QA is never skipped.

Publishing is allowed only after exact-SHA deterministic QA PASS and only through the existing candidate publication boundary. No direct Smith push to `main`.

If publication requires deployment, migration, derived refresh, or other release work, FAST is no longer eligible and must HOLD/reclassify rather than performing hidden DEV_OPS work.

---

## 8. Scope D — Update the TECH Forge UI to show execution lineage

### Goal

The operator should not need shell archaeology to understand what Forge is doing.

Update the existing TECH cockpit incrementally. Do not rewrite the portal.

Primary surface:

- `app/portal/tech/page.tsx`
- `components/portal/tech/engineering-cockpit.tsx`

Feed it from the convergence read model.

### Required story execution view

For an active/recent Forge story show, at minimum:

```text
ENG-FORGE-...
Execution E0                         RUNNING

Workspace      <execution workspace id>
Current SHA    b15506b
Current role   QA
Current node   qa_verify

Iterations
1   c8f2b6f   QA FAIL
2   717c3fc   QA FAIL
3   b15506b   QA RUNNING

Convergence
2 repairs · 3 candidates · <elapsed>
```

Each iteration row should be expandable to existing evidence where available:

- exact candidate SHA;
- producer role/node;
- Story Run id;
- QA run id;
- QA verdict;
- failure class/disposition;
- tests summary;
- runtime/model evidence;
- timestamps.

For replans, show separate execution generations under the same Story rather than flattening them into unrelated agent runs.

For explicit SPLIT, visually label child workspaces as an intentional split. Multiple worktrees must never be presented as an unexplained normal condition.

### UI principle

Git evidence is first-class Forge telemetry. The operator should see current candidate SHA and execution lineage beside Neon/control-plane status.

---

## 9. Explicit non-goals

Do **not** add any of the following as part of this work:

- a second orchestrator or state machine;
- a new queue;
- an ML/epoch-themed domain model;
- a new model provider or harness;
- an automatic fuzzy retry/learning algorithm;
- a cost ledger redesign;
- broad Story Board redesign;
- real-estate workflow changes;
- production database writes or migrations;
- Vercel/PROD deployment changes;
- bulk cleanup/deletion of old agent worktrees;
- automatic branch merging outside the existing accepted-candidate publication seam;
- broad/full regression runs.

Do not weaken the existing exact-SHA QA rule, frozen run contract, commit ownership rules, or publication safety boundaries.

---

## 10. In-scope files

Expected scope. Builder may add a small adjacent helper/test when clearly necessary, but must not broaden domains.

### Execution / workspace

- `agent-runtime/invoker.ts`
- `agent-runtime/invoker-workspace.test.ts`
- `workflow_app/forge/agent-runtime-role-runner.ts`
- focused role-runner test adjacent to the runner if one does not already cover this seam
- `lib/worker-workspace/**` only if required for deterministic execution-generation reuse

### Convergence / repair

- `db/forge-convergence.ts` — new preferred read model
- `db/forge-convergence.test.ts` — new
- `workflow_app/forge/qa-repair-policy.ts`
- `workflow_app/forge/qa-repair-policy.test.ts`
- `db/forge-repair-ledger.ts` only for minimal reuse/query support; no schema expansion by default
- `db/forge-engine-task-execution.ts` only for minimal read/query support

### Workflow / FAST

- `workflow_app/definitions/FORGE_SDLC-v4.xml` — new
- `workflow_app/definitions/forge-sdlc.ts`
- focused Forge workflow tests, preferably a new `workflow_app/tests/forge-convergence.test.ts` if that keeps scope clearer

### TECH UI

- `app/portal/tech/page.tsx`
- `components/portal/tech/engineering-cockpit.tsx`
- small adjacent presentation component only if it materially simplifies the cockpit

---

## 11. Acceptance criteria

### Workspace / execution

1. A normal serial FEATURE traversal (`Architect → Lead PRE → Smith → Lead POST → QA`) uses exactly **one serial writable worktree** for one execution generation.
2. New role/work-item UUIDs do not create new serial worktrees.
3. Repair Smith after QA failure reuses the same execution-generation workspace.
4. Workspace ownership derives from `processInstanceId` + durable execution generation, not `workerId` and not `workItem.id`.
5. REPLAN advances execution generation and the next writable implementation receives a new clean workspace lineage.
6. Prior execution-generation workspaces/history remain intact; no reset/clean/rebase is performed implicitly.
7. Only explicit `smith_split_work` may create multiple writable Smith worktrees. SPLIT child ids are deterministic and traceable back to the parent execution generation.

### Candidate iteration / convergence

8. A new exact persisted candidate SHA increments the candidate iteration.
9. A retry against the same candidate SHA does not increment the iteration.
10. Deterministic QA verdict is bound to the exact candidate SHA tested.
11. QA FAIL classified as implementation repair returns to Smith in the same execution/workspace.
12. QA/Lead evidence classified as architecture/contract REPLAN advances execution generation instead of pretending it is another implementation repair.
13. Same SHA + same machine QA failure repeated with no intervening candidate produces `NO_PROGRESS` HOLD and does not launch another model repair cycle.
14. `CONVERGED` is derived only from deterministic QA PASS against the current exact candidate.

### FAST

15. An eligible FAST story routes `Smith → deterministic QA → deterministic publish → Complete` without Architect, Lead, or DEV_OPS model runs.
16. FAST QA FAIL routes to repair Smith in the same execution/workspace, then deterministic QA again.
17. FAST never skips deterministic QA.
18. FAST never publishes a candidate that has not passed exact-SHA QA.
19. FAST with migration, derived refresh, deployment, unresolved architecture, or split requirement fails closed to HOLD/reclassification; no hidden PROD/release work occurs.
20. `FORGE_SDLC-v3.xml` remains unchanged; v4 is a new immutable workflow definition and becomes active only through `forge-sdlc.ts`.

### UI

21. TECH cockpit groups Forge activity by Story → execution generation → candidate iteration rather than only a flat list of agent runs.
22. The operator can see current execution, workspace id, current node/role, current candidate SHA, candidate count, repair count, and QA status at a glance.
23. Each candidate iteration shows its exact SHA and QA verdict.
24. Replans are visibly separate execution generations under the same Story.
25. Intentional SPLIT child workspaces are visibly marked as SPLIT; unexplained worktree fanout is not treated as normal.

### Safety

26. No PROD database mutation, deployment, migration, or derived refresh is performed by this work order or its tests.
27. Main remains the only operator development branch for this work. Do not create an implementation feature branch unless separately authorized.
28. No full regression is run unless separately authorized.

---

## 12. Targeted test plan

Run only tests directly covering changed seams.

Minimum expected commands after implementation:

```bash
pnpm exec tsx --test agent-runtime/invoker-workspace.test.ts
pnpm exec tsx --test workflow_app/forge/agent-runtime-role-runner.test.ts
pnpm exec tsx --test db/forge-convergence.test.ts
pnpm exec tsx --test workflow_app/forge/qa-repair-policy.test.ts
pnpm exec tsx --test workflow_app/tests/forge-convergence.test.ts
```

If an exact listed test file does not yet exist, create the focused test at the closest existing Forge test seam rather than running a broad suite.

Optional scoped typecheck:

```bash
pnpm exec tsc --noEmit
```

If known unrelated TypeScript errors appear, report them explicitly and do not broaden this story to repair unrelated code.

Do **not** run the full repository regression without separate approval.

---

## 13. Required DEV integration proof

After targeted tests pass, prove the behavior with **one bounded DEV FAST story** that intentionally exercises one repair cycle.

Expected proof:

```text
Ready FAST
  ↓
Smith
  ↓
candidate 1
  ↓
deterministic QA FAIL
  ↓
repair Smith — same workspace
  ↓
candidate 2
  ↓
deterministic QA PASS
  ↓
deterministic publish
  ↓
Complete
```

Verify durable evidence:

- one Forge process instance / execution generation;
- one serial worktree for that generation;
- exactly two distinct candidate iterations;
- candidate 2 is a descendant of candidate 1 in the same normal Git lineage;
- no Architect model run;
- no Lead model run;
- no DEV_OPS model run;
- deterministic QA records exact candidate SHA for both iterations;
- the first FAIL is retained visibly;
- the second PASS is retained visibly;
- accepted candidate publication is the only point at which `main` advances;
- TECH UI shows the complete convergence chain without requiring shell/Git archaeology.

DEV only. No PROD proof is authorized by this work order.

---

## 14. Commit expectations

Work directly on `main` in small surgical commits. Suggested sequence:

1. `fix(forge): key serial workspace to execution generation`
2. `feat(forge): expose candidate convergence lineage`
3. `feat(forge): make fast lane smith qa publish`
4. `feat(tech): show forge execution convergence`

The builder may combine commits when one seam cannot truthfully stand alone, but each commit should leave its touched contract coherent and testable.

Do not mix unrelated application, marketing, accounting, CRM, or production changes into these commits.

---

## 15. Definition of done

This work is done when a bounded DEV story can be understood and operated as:

> **one target → one execution workspace → one candidate lineage → deterministic QA/repair iterations → convergence → accepted candidate publication**

and when extra writable worktrees occur only because Forge explicitly chose SPLIT.

The success criterion is not a richer diagram or more agent activity. It is **faster, more legible delivery of working code with durable evidence and less Git/orchestration ambiguity.**
