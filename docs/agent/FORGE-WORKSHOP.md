# CulebraLuxe Engineering Workshop — HELM Manifest

This is the **boot sheet for a fresh engineering session**. It is not a handoff and it is not the architecture source of truth.

- **ARCH-01 in Neon** = architecture authority / what the system is.
- **SOP-01 in Neon** = operating authority / how the system is run.
- **This file** = what is available in the workshop and how to choose among it.
- **Git + Neon live state** = what is actually true right now.

Before substantial work, inspect current Git and relevant Neon Story / Story Run / Forge evidence. Do not assume the live status snapshot below is newer than the repository or database.

---

## HELM

When the user explicitly says **"You have the HELM"**, treat that as permission to choose the engineering operating mode, not merely permission to write code.

With the HELM you may, as appropriate:

- inspect and modify repository code;
- use Git branches and isolated worktrees;
- read and write **Neon DEV**;
- create disposable DEV evaluation/test state;
- invoke the Forge workflow and its roles;
- use OpenCode / configured models through Forge;
- run targeted tests and deterministic engineering tools;
- investigate first, then change operating mode if the work is larger or riskier than it first appeared;
- stop, HOLD, recut, or escalate rather than continuing to spend against a bad shape.

HELM does **not** grant blanket production authority. Without separate explicit approval, do not:

- mutate Neon PROD;
- deploy or promote to PROD;
- delete or destructively rewrite production data;
- merge/promote risky experimental candidates;
- run a broad/full regression when targeted proof is sufficient.

**Decision rule:** use the cheapest operating mode that still controls correctness, risk, integration cost, and widget spend.

---

## Choose the operating mode

### DIRECT / SOLO

Use when the work is tiny, coherent, low-risk, and mechanically provable. Orchestration should not cost more than the change.

Typical shape:

`inspect -> edit -> targeted proof -> candidate/review as required`

### FORGE

Use when the work has meaningful uncertainty, multiple semantic surfaces, dependencies, substantial proof burden, recovery risk, or benefits from independent verification.

Typical shape:

`Scout -> Architect -> Lead -> SOLO or resident Smith -> Lead POST -> fresh QA/Inspector -> Assay -> DEV_OPS when authorized`

Starting DIRECT and later handing the work to Forge because scope grew is **good judgment**, not failure.

### HOLD / RECUT

Use when the story is oversized, architecturally unresolved, unverifiable, missing required context, or would require a fourth Smith chunk. Do not spend through uncertainty merely because a model can keep generating.

---

## Forge crew

### Scout

- read-only reconnaissance;
- Ripwire structural repository context / owning symbols / callers / blast radius;
- identifies relevant tests, uncertainties, and next investigation;
- produces durable `SCOUT_RESEARCH` evidence for downstream roles.

### Architect

- defines **what must be true**;
- freezes scope, invariants, acceptance, pre/postconditions and relevant context;
- does not spend implementation authority.

### Lead

- owns the checkbook and executable shape;
- validates the Architect contract against repo reality;
- chooses `SOLO`, `SMITH`, `HOLD`, or exceptional `SPLIT`;
- chooses model/grade appropriate to risk and proof;
- shapes Smith work into the **fewest safe proof boundaries**;
- defines merge/post gate before implementation where practical.

### Smith

- implements bounded work;
- may refine engineering inside a chunk but may not enlarge story scope;
- resident construction context is intended to persist across serial chunks/repair;
- a fourth chunk is STOP/HOLD, not permission to keep working.

### QA / Inspector

- fresh context after candidate freeze;
- independent semantic review of the exact candidate;
- worker self-report is not proof.

### Assay

- deterministic / model-free verification;
- runs the frozen command plan against the exact candidate;
- failure is evidence, not permission to silently declare success.

### DEV_OPS

- promotion, migrations, deploy and smoke;
- production actions remain separately governed.

---

## Core workshop tools

### Ripwire

Structural repository intelligence used primarily by Scout. It maps relevant files/symbols, callers, blast radius and likely tests. Treat it as repo context, not as an execution authority.

### RTK

Context compression for noisy command/tool output. Preserve useful evidence while avoiding raw-output context pollution. Compression must not substitute for semantic proof.

### Resident OpenCode construction session

The construction-side goal is:

`Scout -> Architect -> Lead -> Smith/repair`

with accumulated understanding preserved where supported.

**Invariant:** `CONTEXT PERSISTS; AUTHORITY DOES NOT.`

Every lane transition reapplies role/model/profile/permissions. Candidate freeze is a hard context wall before independent QA.

### Forge Phase Agent abstraction

Each role has a governed phase contract and required durable deliverable. Useful work must not disappear because an event/marker/parser missed it. Logging failure must never recursively destroy the run.

### Execution shaping

Lead emits a bounded `SmithExecutionPlan`:

- 1..3 serial chunks only;
- one outcome per chunk;
- explicit owning surface;
- invariant established;
- targeted proof;
- serial dependency order;
- same Smith brain/session/worktree unless the architecture explicitly chooses otherwise.

**MULTI-CHUNK DOES NOT MEAN SPLIT.**

`SPLIT` means genuinely separate execution topology/workers/worktrees and is exceptional, not the default response to decomposition.

### Dispatchability toolkit / Kraken

The policy library combines:

1. hard structural execution-shaping rules;
2. qualitative dispatchability judgment;
3. plan-derived difficulty / P(success).

`assessSmithDispatch(...)` returns `GO | FLAG | HOLD` with reasons.

Use it as an evidence-producing policy seam. Do not claim it is the authoritative pre-Smith hot-path breaker unless the current code proves that wiring is live.

### Anti-token-fire inner fuse

The running role gate currently treats a Smith self-plan that is `OVERSIZED` or greater than 3 chunks as a real HOLD under deliverable enforcement. A fourth chunk is not "keep working."

This is an **inner fuse**. It does not by itself prove that a bad Lead plan is blocked before the first Smith token is spent.

### Failure triage

Classify before escalating:

- **SPEC** — ambiguous/wrong/missing contract context -> repair the contract, same capability tier first;
- **ENVIRONMENT** — flaky test, wrong branch, stale state, dependency/permission/timeout -> repair environment, same tier first;
- **CAPABILITY** — clean spec + clean environment but worker cannot do it -> bounded model escalation and/or recut;
- **VERIFIABILITY** — repo cannot prove the required behavior -> HOLD or repair proof infrastructure; a stronger model does not fix missing verification;
- **SCOPE EXPANSION** — work escaped the frozen story -> HOLD / new story.

Retry budgets are bounded. Do not "try harder" indefinitely.

### Two retry shapes

- **Attempt failure with no verified progress** -> reset code/workspace to baseline and retry cleanly while preserving resident understanding where safe.
- **Verifier-found gap in otherwise passing work** -> incremental repair on the same candidate lineage; reverify only the open items rather than destroying already-proven work.

### Context lesson loop

When a SPEC failure is caused by missing context, encode that context so a later role touching the same area can receive it. The same important context should not be missing twice.

### Deterministic engineering tools

Use the current repo configuration rather than assuming availability. Relevant workshop surfaces may include targeted tests, TypeScript checks, dependency-cruiser, Semgrep, Knip, and other deterministic gates. Prefer targeted proof unless the story specifically requires broader regression.

### Reference ships for piracy, not adoption

Local reference clones may exist for:

- `adiatmaja/praxis` — capability-aware sizing, dispatchability, bounded triage;
- `midego1/claude-orchestrate` — failure taxonomy, retry/evidence discipline;
- `open-gsd/gsd-core` — dependency-first planning, tracer-first slicing, plan checking.

Read them for algorithms, prompts and policy ideas. **Do not import their runtimes or let them become a second orchestrator unless explicitly directed.** Forge remains authoritative.

---

## Lead / spend rules

The Lead is not a router. The Lead controls cognitive load, integration risk and spend.

A good Lead asks:

1. What is actually hard or uncertain?
2. What depends on what?
3. What can one engineer safely hold in context?
4. Where should we demand proof before spending more?
5. Is coordination/parallelism actually worth its cost?
6. Is the cheapest correct grade sufficient?
7. At what point must we HOLD instead of continuing?

Default Smith shape:

`one story -> one resident Smith -> 1..3 serial chunks -> proof at each boundary`

Do not size primarily by LOC/file count. Consider semantic surface, coupling, uncertainty, dependency depth, context burden, proof burden, change novelty and worker fit.

---

## Stop conditions

Stop or HOLD rather than silently expanding when any of these occurs:

- a fourth Smith chunk appears;
- the Architect contract materially changes;
- an independent second business outcome appears;
- no runnable/credible proof exists for important behavior;
- the environment is not trustworthy enough to classify the failure;
- the execution budget/wall-clock cap is exceeded;
- production mutation would be required without explicit approval;
- candidate identity or exact-SHA verification is uncertain.

---

## Live capability snapshot — 2026-09-10

This is a **snapshot, not authority**. Verify newer truth in Git + Neon before acting.

**Routing / architecture**
- `FORGE_ROUTING_BRAIN` **defaults to `engine`** — one canonical brain (`FORGE_SDLC`). The legacy reducer is opt-in (`FORGE_ROUTING_BRAIN=reducer`) for tests/migration only, and `pickLane` is labeled reducer-era hydrate-only (a total function). Never dual-write.
- Engine named as the one brain; `workflow_app/forge` and DEV_OPS grew *around* `engine.ts` rather than splitting it.

**Pre-Smith fuse (Lead → Smith) — authoritative and machine-enforced**
- Lead **must** emit a machine-readable `LEAD_PLAN:` for a SMITH/SPLIT dispatch (`8f842b8`), it is persisted (`a01e7db`), and rendered to Smith as explicit per-chunk WORK ORDERS it executes verbatim (no re-planning, no scope enlargement).
- The **running** gate is `assessLeadHandoff` (`forge-lead-plan.ts`), called on the `lead_pre` node in `agent-runtime-role-runner`. A **missing or malformed** plan on SMITH/SPLIT is itself a HOLD → the task is released and **no Smith adapter starts**. Gated only at the Lead handoff (never applied on a `smith` node).
- Difficulty scores are **FLAG-only** (`9fe43fda`): uncalibrated logistics can no longer spend a HOLD.
- The full KRAKEN gate (`assessSmithDispatch`) therefore **runs in the live pre-Smith path**; the post-Smith envelope guard (`assessSmithWork` in `forge-dispatch-seam.ts`) still holds Smith's self-reported `SMITH_PLAN` when it is OVERSIZED or claims >3 chunks.

**QA / Assay**
- Ready-gate: a **zero-command assay recipe = `missing-assay-plan`** (`b8df15`) — FINAL-02's lesson is now a gate, not a postmortem. A FEATURE/RESEARCH story packet still needs a `## Assay commands:` section.
- QA **always terminates**: the assay child runs in its own process group, the timeout SIGTERMs the tree then SIGKILLs after a 5s grace (worst case `timedOut` → HOLD), so QA can no longer hang.
- Optional arch tooling missing in a bare worktree no longer fails QA.
- `failure_classifier` routes on the **`failureClass` the model emitted**, not on `leadDecision` (`46ffa33`).

**DEV_OPS / release**
- **MINIMAL DEV_OPS** (`e83f6ed`): a `deploy_required=false` story **completes at publish**; CI owns production smoke. A headless run is no longer blocked on `production_smoke`.
- **Schema gate (new, 2026-09-10):** `db/migrations/144` added the `schema_migration` ledger; every apply records filename + sha256 + target, re-apply is skipped/refused on checksum mismatch. `pnpm db:parity` (tables, columns, **indexes, FKs**) and `pnpm db:migrations` are release gates, and `release-operations.verifyMigrations` fails closed unless the ledger records the migration **and** DEV/PROD parity is clean. See `docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md`.
- `db/migrations/145` added `tasks.node_id` and the engine now writes it (fixes a live defect where Contract execution could never complete `pns_executed`).

**Proven end-to-end**
- **V13** reached publish. **V14** ran to `"status":"completed"` and shipped `1a83b3b` — the first clean full WRITE → freeze → QA → publish → complete with a real `lead_post` integration.
- Still **not** established: one quiet completion with no human in the loop on publish/classifier. V13/V14 hit `PUBLISH_CONFLICT` / classifier confusion; a HOLD after QA PASS is a control-plane bug, not a Smith bug.

**Known gaps (honest)**
- **Usage is still zero.** 204 PROD runs, `tokens_*` unwritten — so difficulty cannot be calibrated and Flash-vs-Pro cannot be graded. Packet `ENG-FORGE-RUN-USAGE` is the next story; **null is honest, invented numbers are poison**.
- **Session continuity is still env-gated** — proven, not the default. The amnesia problem is not closed.
- Dispatch/`LEAD_PLAN` parsing depends on the model emitting the machine line; the gate HOLDs rather than guessing when it is absent.

---

## Fresh-session startup

When handed a pile of work with the HELM:

1. Read this file.
2. Read/retrieve `ARCH-01` and `SOP-01` from Neon when relevant.
3. Inspect current Git HEAD/status/recent commits and the relevant source surfaces.
4. Inspect relevant Neon Story, Story Run, Forge task/evidence/artifact state when the work touches Forge or active workflows.
5. Decide DIRECT/SOLO vs FORGE vs HOLD before spending heavily.
6. State the operating choice briefly and why.
7. Execute using the smallest sufficient workshop surface.
8. Preserve evidence, candidate identity, targeted proof and stop conditions.

A useful HELM prompt is intentionally small:

> **You have the HELM. Read `docs/agent/FORGE-WORKSHOP.md`, consult ARCH-01/SOP-01 and live Git/Neon state as needed. Here is the work. Use any part of the workshop you want. You own the outcome, time, risk and widget spend. Choose the operating mode and execute.**

The desired behavior is not "always use Forge." The desired behavior is **use the cheapest operating mode that still controls the risk, and change modes when reality says you should.**
