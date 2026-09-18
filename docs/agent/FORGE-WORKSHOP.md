# CulebraLuxe Engineering Workshop — HELM Manifest

This is the **boot sheet for a fresh engineering session**. It is not a handoff and it is not the architecture source of truth.

- **`ARCH-HANDOFF` in Neon** (title "ARCH1 — Architecture Handoff — READ FIRST") = architecture authority / what the system is.
- **`SOP1` in Neon** (title "SOP1 — Pippin Forge Watch SOP") = operating authority / how the system is run.
- **`DEEP1` in Neon** (title "DEEP1 — Data Pipeline Doctrine … READ SECOND") = data-pipeline authority / how data travels from intake to screen. Read it second, after ARCH-HANDOFF.
- **This file** = what is on the bench, who may touch it, and how to choose a mode.
- **Git + Neon live state** = what is actually true right now.
- **Tool authority** = `workflow_app/forge/forge-tool-catalog.ts` (`resolveForgeToolPermissions`). Recomputed every lane transition. A session cannot keep a stale grant.

> **Story ids, so nobody hunts again:** the board ids are `ARCH-HANDOFF`, `SOP1` and `DEEP1`.
> Earlier revisions of this file called the first two "ARCH-01" and "SOP-01", which match
> no row — searching for them returns nothing. All three are reference stories:
> `workstream = ARCH`, `rollup = false`, so they never create an agent work item.
> ARCH-HANDOFF and SOP1 were last written before a lot changed; treat their content as
> authoritative only where it does not contradict Git + live Neon state.

Before substantial work, inspect current Git and relevant Neon Story / Story Run / Forge evidence. Do not assume any snapshot in this file is newer than the repository or database.

---

## Two laws

1. **CONTEXT PERSISTS; AUTHORITY DOES NOT.**
   The OpenCode session (the desk) may carry understanding across Scout → Architect → Lead → Smith. Every hat change reapplies role, model, profile, and **tool grants**. Candidate freeze is a hard wall before independent QA. A long session is a dirty desk — cut it and reload the cabinet, do not coach the ghost.

2. **A tool not granted to this hat does not exist.**
   Connected MCP is not permission. Cline / OpenCode seeing Serena does not mean Serena may drive a browser, walk the whole repo, or write. Grants come from the catalog for **this** position. Everything else is off the bench.

The desk must be the right shade of messy:

- Too clean → the model rediscovers the repo for 13 minutes and invents seams.
- Too dirty → 20-hour transcript, RTK exhaust, failed JSON; 2+2 breaks.
- Right mess → baseRef, declared surfaces, frozen proofs, last accepted handoff **row**, candidate SHA if any, last good machine line. Not the novel.

**Cabinet (Neon rows) survives the chair. The chair does not survive a dirty window.**

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
- run a broad/full regression when targeted proof is sufficient;
- let Serena, a browser, or any MCP tool act outside its catalog grant.

**Decision rule:** use the cheapest operating mode that still controls correctness, risk, integration cost, and widget spend.

---

## Choose the operating mode

### DIRECT / SOLO

Use when the work is tiny, coherent, low-risk, and mechanically provable. Orchestration should not cost more than the change. One surface + one frozen proof is SOLO until proven otherwise.

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

## The desk and the cabinet

| Piece | What it is | Sleep cost | Wake cost |
| --- | --- | --- | --- |
| Harness | OpenCode / worker process + worktree | RAM if kept | Spawn |
| Session (desk) | OpenCode SQLite transcript, session id on the story-run | Disk | Prefill / prefix-cache |
| Cabinet | Neon rows: `forge_role_contract`, plan chunks, findings, SHA, receipts | Almost free | Read |
| KV | Model working memory on GPU | GPU rent | Full re-seed if evicted |

Passing a session id across workers shares the **conversation**, not a free warm brain. Prefill of a *clean* session beats a full ramp (spawn + RTK/git walk + invented scope). Prefill of a junk drawer does not.

Construction-side goal:

`Scout -> Architect -> Lead -> Smith/repair`

on **one session id per story-run** where the runtime supports it. Hats are new markdown + recomputed grants on that session, not new lives.

- Architect then Lead on the same live process is the warm path that pays.
- Smith may take a clean desk loaded from the cabinet (assignment, surfaces, proofs) so implementation dirt does not live in Architect's window.
- Assay does not need a session. It is a job.

**Done means a row the database accepts.** Reply JSON / marker lines are scrap when the row exists. A marker without a row is not done.

Rehab: one pass, feed the tail of the last reply, name the miss. Second identical miss hash → new session, cabinet only. Third → HOLD.

---

## Forge crew

### Scout

- read-only reconnaissance;
- Ripwire structural repository context / owning symbols / callers / blast radius;
- identifies relevant tests, uncertainties, and next investigation;
- produces durable `SCOUT_RESEARCH` / `context_refs` for downstream roles;
- does **not** get Serena. Discovery is this job; later hats read the packet.

### Architect

- defines **what must be true**;
- freezes scope, invariants, acceptance, pre/postconditions and relevant context;
- seams must exist on the pinned `baseRef` (a new file → declare the parent **directory**, which does exist);
- does not spend implementation authority;
- writes findings / handoff into the cabinet. Invented seams HOLD.

### Lead

- owns the checkbook and executable shape;
- validates the Architect contract against repo reality;
- chooses `SOLO`, `SMITH`, `HOLD`, or exceptional `SPLIT`;
- records the decision in **fields** (`forge_role_contract`, assignments, chunks) via `scripts/forge-handoff.mjs`;
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
- PASS / FAIL / INCOMPLETE — an empty plan is a gap, never a pass;
- failure is evidence, not permission to silently declare success.

### DEV_OPS

- promotion, migrations, deploy and smoke;
- production actions remain separately governed.

---

## Core workshop tools

Authority is the catalog. Skill packs live in `docs/agent/skills/`. Interrogate live state with:

```sh
pnpm forge:tools                 # status for every tool
pnpm forge:tools --role smith    # what is in force for one position
pnpm forge:tools --run           # run the deterministic instruments here
```

Verify a tool by running it. A code path is not a running tool.

### Classes

- **model-facing** — the model may choose it from its catalog (Ripwire, Serena).
- **transparent** — runtime applies it *under* the model. The model never picks it and never sees a schema for it (RTK).
- **deterministic** — Assay / Inspector run it. Must never appear in a model catalog (cruiser, semgrep, knip).

### Ripwire

Structural repository intelligence. Maps files/symbols, callers, blast radius and likely tests. Treat it as repo context, not execution authority.

- Class: model-facing
- Positions: Scout · Architect · Lead PRE · Smith · Inspector
- Writes: none
- Fallback: `workspace.fs.read` + `git.diff`
- Skill: `docs/agent/skills/ripwire.md`

### Serena

Symbol-level navigation, references, diagnostics, and (where granted) bounded semantic edits. **Not a browser. Not a repo walker. Not the helm.**

- Class: model-facing
- Wired: **yes** — registered headless 2026-09-16 with `--context oaicompat-agent` and `--open-web-dashboard false`. The 2026-09-13 removal is history: the registration had no `--context`, so it ran in Serena's default `desktop-app` context and opened a browser page once per lane. Verified by starting the server: 52 tools, project loaded, no browser line. `pnpm forge:tools` reports `serena` wired.
- Positions: Architect · Lead PRE / SOLO / POST · Smith
- Writes: Lead SOLO / POST · Smith only (`symbol.rename`, `symbol.replace-body`, `symbol.insert`)
- Architect and Lead PRE: read operations only (`overview`, `lookup`, `references`, `implementations`, `diagnostics`)
- Excluded entirely: Scout · Inspector · Assay · DEV_OPS (exclusion, not a smaller grant)
- Fallback: Ripwire + generic read/search
- Skill: `docs/agent/skills/serena.md`

Connected is not permission: the grant comes from the catalog for THIS position, and Lead SOLO / POST · Smith hold the bounded writes (`symbol.rename`, `symbol.replace-body`, `symbol.insert`). If Serena preempts a human browser or starts an unbounded project walk, that is an authority leak. Stop the tool, do not negotiate with it.

### RTK

Transparent compaction of noisy command output (`git` / `ls` / `tree` / `gh` shims on the harness PATH). The model keeps typing `git status` and gets compacted output. Exit codes are preserved. Compression is not proof.

- Class: transparent shim — **never a model tool choice**
- Positions: Architect · Lead · Smith
- Writes: none
- Fallback: raw command path (uncompacted)
- Skill: `docs/agent/skills/rtk.md`
- Hard rule: RTK must not fork-bomb. One index job per run; everyone else reads the file.

### Cruiser (dependency-cruiser)

Architecture boundaries and cycles. **Hard gate** on the exact candidate.

- Class: deterministic
- Positions: Assay · Inspector
- Writes: none
- Fallback: gate skipped and the omission recorded — never a silent PASS
- Skill: `docs/agent/skills/cruiser.md`

### Semgrep

Static and dataflow checks on the exact candidate. Informational. Must never recall Smith.

- Class: deterministic
- Positions: Assay · Inspector
- Skill: `docs/agent/skills/semgrep.md`

### Knip

Unused files, exports, dependencies. Hygiene. Informational. Must never recall Smith.

- Class: deterministic
- Positions: Inspector
- Skill: `docs/agent/skills/knip.md`

### Other deterministic surfaces

Use whatever the repo actually has: targeted tests, `tsc --noEmit`, `pnpm forge:tools --run`. Prefer targeted proof unless the story requires broader regression. A skipped arch gate is INCOMPLETE, not clean.

### Perimeter instruments (installed, not engine tools yet)

Three instruments run at the repository boundary. They are **deliberately NOT in the engine tool
catalog**: `wired` in `workflow_app/forge/forge-tool-catalog.ts` means "the engine can actually run
it", a fence refuses to catalogue anything less (`wired` was corrected on 2026-09-11 for exactly this
reason — cruiser and knip read as wired while the gate silently skipped), and the engine's static gate
does not invoke these yet. Cataloguing them now would be a claim rather than a fact. Each one enters
the catalog with the story that builds its seam.

| Instrument | Runs via | Found on first run, 2026-09-18 | Enters the catalog with |
| --- | --- | --- | --- |
| squawk 2.65.0 | `pnpm scan:migrations` (changed files) and the gates workflow | 745 findings across 192 migrations — 145 indexes without `CONCURRENTLY`, 50 constraints without `NOT VALID` | `ENG-FORGE-MIGRATION-LINT-01` |
| gitleaks 8.30.1 | `pnpm scan:secrets` and the gates workflow | a plaintext env backup captured in 9 `refs/cline/checkpoints` refs — never pushed, purged 2026-09-18 | `ENG-FORGE-LANE-SECRET-GATE-01` |
| osv-scanner 2.6.0 | `pnpm scan:deps` and the gates workflow | 17 packages affected by 45 advisories (2 critical, 22 high) | `ENG-FORGE-DEPENDENCY-AUDIT-01` |

- Class: deterministic. **Never model-facing** — a model that chooses whether to be scanned is not
  being scanned.
- Positions: none yet. No position may claim these until its seam exists.
- Record: `docs/agent/PERIMETER.md` — the numbers, the root causes, and the invocations that silently
  do not cover (`osv-scanner` without `--all-packages` reports 17 of 899 packages; `gitleaks dir`
  scans your own `.env.local` and build output, so history is the perimeter).

CI (`.github/workflows/gates.yml`) runs all three on every push, which is why they are worth having
before they are engine tools.

### Resident OpenCode construction session

See **The desk and the cabinet**. Session id belongs on the story-run. Hats change; the cabinet is the personnel file.

### Forge Phase Agent

Each role has a governed phase contract and a required durable deliverable (`collect` fills, parent gates decide). Useful work must not disappear because a marker/parser missed it. Logging failure must never recursively destroy the run.

Deliverable lives in **columns** where the story has landed that seam (`forge_role_contract`, `forge_role_assignment`, `forge_role_plan_chunk`, written by `scripts/forge-handoff.mjs`). JSON marker lines (`LEAD_ROUTING:`, `LEAD_PLAN:`, `FORGE_ARCHITECT_HANDOFF:`) are fallback for older runs only. The row wins when present. A role instructed to record fields that still "succeeds" by emitting only chat JSON is on the wrong channel.

### Execution shaping

Lead records a bounded plan (rows, not a novel):

- 1..3 serial chunks only;
- one outcome per chunk;
- explicit owning surface (at least one path);
- invariant established;
- targeted proof (non-blank, preferably a frozen story command);
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

Use it as an evidence-producing policy seam. Do not claim it is the authoritative pre-Smith hot-path breaker unless the current code proves that wiring is live. The live Lead reviewer is `reviewLeadProposal` in `forge-lead-routing.ts`. `assessLeadHandoff` is retired from the hot path.

### Anti-token-fire inner fuse

The running role gate treats a Smith self-plan that is `OVERSIZED` or greater than 3 chunks as a real HOLD under deliverable enforcement. A fourth chunk is not "keep working."

This is an **inner fuse**. It does not by itself prove that a bad Lead plan is blocked before the first Smith token is spent.

### Failure triage

Classify before escalating:

- **SPEC** — ambiguous/wrong/missing contract context → repair the contract, same capability tier first;
- **ENVIRONMENT** — flaky test, wrong branch, stale state, dependency/permission/timeout → repair environment, same tier first;
- **CAPABILITY** — clean spec + clean environment but worker cannot do it → bounded model escalation and/or recut;
- **VERIFIABILITY** — repo cannot prove the required behavior → HOLD or repair proof infrastructure; a stronger model does not fix missing verification;
- **SCOPE EXPANSION** — work escaped the frozen story → HOLD / new story.

Retry budgets are bounded. Do not "try harder" indefinitely.

### Two retry shapes

- **Attempt failure with no verified progress** → reset code/workspace to baseline and retry cleanly while preserving resident understanding where safe.
- **Verifier-found gap in otherwise passing work** → incremental repair on the same candidate lineage; reverify only the open items rather than destroying already-proven work.

### Context lesson loop

When a SPEC failure is caused by missing context, encode that context so a later role touching the same area can receive it. The same important context should not be missing twice.

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
- the execution budget / turn cap / wall-clock cap is exceeded;
- the session has gone rogue (repeating itself, fighting parsers, inventing tools);
- production mutation would be required without explicit approval;
- candidate identity or exact-SHA verification is uncertain;
- a tool is acting outside its catalog grant.

---

## What is live vs what you must re-verify

This section is **orientation, not authority**. Git HEAD + Neon win.

Landed and should be treated as the intended machine:

- Analyzer catalog of six tools, all wired: Ripwire, Serena, RTK, cruiser, semgrep, knip — see `pnpm forge:tools`.
- Decision-in-fields: migrations `170_forge_role_contract` and `171_forge_role_plan`; writer is `scripts/forge-handoff.mjs`.
- Architect seam existence check on pinned `baseRef`; reasons recorded on rejection (not discarded).
- Deterministic Assay adjudicator: PASS / FAIL / INCOMPLETE.
- Ready-gate: a zero-command assay recipe on a QA-applicable story is `missing-assay-plan`.
- Schema / release ledger and DEV/PROD parity gates (`docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md`).
- `CONTEXT PERSISTS; AUTHORITY DOES NOT` is the session law. Continuity is still easy to leave env-gated — check the current runner before assuming a second role shares a process.

Still easy to get wrong (do not paper over these in a snapshot):

- Lead collect may still parse reply JSON *before* preferring the row. If the instruction says "record fields" and the gate still scores a marker line, the model will emit the marker. Fix the gate, not the prompt.
- Usage / token columns may still be unwritten. Null is honest; invented numbers are poison.
- A HOLD after QA PASS is a control-plane bug, not a Smith bug.

---

## Fresh-session startup

When handed a pile of work with the HELM:

1. Read this file.
2. Retrieve the reference stories from Neon when relevant: `ARCH-HANDOFF` (architecture), `SOP1` (operations), `DEEP1` (data pipeline: ODS -> warehouse -> screen, and the traps in it).
3. Inspect current Git HEAD/status/recent commits and the relevant source surfaces.
4. Inspect relevant Neon Story, Story Run, Forge task/evidence/artifact state when the work touches Forge or active workflows.
5. `pnpm forge:tools` if the work will touch analyzer tools — believe the live grant, not memory.
6. Decide DIRECT/SOLO vs FORGE vs HOLD before spending heavily.
7. State the operating choice briefly and why.
8. Execute using the smallest sufficient workshop surface.
9. Preserve evidence, candidate identity, targeted proof and stop conditions.
10. When the shift ends, leave the cabinet complete even if you throw the chair away.

A useful HELM prompt is intentionally small:

> **You have the HELM. Read `docs/agent/FORGE-WORKSHOP.md`, consult ARCH-HANDOFF / SOP1 / DEEP1 and live Git/Neon state as needed. Here is the work. Use any part of the workshop you want. You own the outcome, time, risk and widget spend. Choose the operating mode and execute. A tool not granted to this hat does not exist.**

The desired behavior is not "always use Forge." The desired behavior is **use the cheapest operating mode that still controls the risk, and change modes when reality says you should.**
