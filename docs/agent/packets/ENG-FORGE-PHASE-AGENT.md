# ENG-FORGE-PHASE-AGENT — Role/Phase Agent abstraction

**Status:** DESIGN (parked until the current runtime pass stabilizes). Captured
2026-09-08 after three evidence-marshaling defects traced to a single scatter:
scout forgetting its packet, architect forgetting its disposition, findings
parsed only for architect. The common root was that per-role marshal logic lives
as conditionals inside one big `createAgentRuntimeForgeRoleRunner`, so each role's
"deliverable" was not a declared, enforced invariant.

## Why

Today a role node executes through `forge-executor` → the role-runner → a harness
adapter. Deliverables are bolted on as `if`-branches:
- scout must write a packet to `story.context_refs` (else Architect's gate blocks)
- architect must emit `research_disposition` (else the engine can't route)
- findings must be parsed for architect **and** scout (else GIGO)

These were each added reactively. A parent abstraction makes the deliverable a
**declared invariant every role inherits** instead of a remembered conditional.

## Layering (do NOT collapse existing seams)

```
forge-executor (engine driver)
  └─ ForgePhaseAgent (NEW abstract — owns phase lifecycle + deliverable gate)
       ├─ AgentRuntimeAdapter  (EXISTING — execution: begin/heartbeat/terminal,
       │    evidence normalization, cost/model recording, escalation)
       └─ role subclasses (ScoutAgent, ArchitectAgent, LeadAgent, SmithAgent,
            QAAgent, DevOpsAgent)
```

`AgentRuntimeAdapter` already IS the execution base (harness run, run row, cost,
heartbeat). The new parent is the **phase contract layer on top of it** — it does
NOT duplicate execution. It owns role plan + context + prompt + marshal + gate +
persist + finish.

## Parent lifecycle (abstract methods in {braces})

```
ForgePhaseAgent.execute(node, task)
  buildPlan()                      → role, phase, permissions, evidenceInstruction
  gatherPriorContext()             → prior run/packet (the Scout→Arch handoff),
                                     V5-21 resident session when it lands
  promptAndExecute()               → via harness AgentRuntimeAdapter
  marshalEvidence()                → normalize + parse structured markers (JSON/legacy)
  assertDeliverable()  [abstract]  → THE GATE: throw/HOLD if deliverable absent
  persistRunAndDeliverable()       → run row + cost + the phase's durable output
  finish()
```

`assertDeliverable` is the enforcement that fixes today's flakiness: a role cannot
report Complete without its required output (else the engine HOLDs / retries). It
is the generalization of "scout must produce a packet" / "architect must produce a
disposition" / "smith must produce a candidate SHA".

## Subclasses declare their contract

| Agent | deliverable (assertDeliverable) | persisted to |
|---|---|---|
| ScoutAgent | non-empty research packet | `story.context_refs` (+ run notes) |
| ArchitectAgent | valid `research_disposition`/plan | `forge_workflow_evidence` |
| LeadAgent | decision + split count | `forge_workflow_evidence` |
| SmithAgent | candidate SHA | run `commit_hash` / `candidate_sha` |
| QAAgent | exact-SHA PASS/FAIL | assay evidence / `qa_passed` |
| DevOpsAgent | release/prod receipt | `forge_workflow_evidence` |

Each subclass keeps its specific permissions + model grade (flash/pro), but the
lifecycle + marshal + persist + capture are shared and correct-by-construction.

## What it buys

1. **Enforced deliverables** — the three marshaling bugs become impossible to
   reintroduce (a subclass that forgets to declare its deliverable fails loudly).
2. **One canonical evidence/run/capture path** — every phase routes through the
   error-capture framework (AGENTS "Error Capture Obligation"), no bespoke
   try/catch per role.
3. **A home for V5-21** — `gatherPriorContext()` is where the story-resident
   OpenCode session (context persists; authority does not) plugs in.
4. **Cleaner role-runner** — the ~conditional-soup collapses into ~6 small
   subclass files + one shared parent.

## Suggested surface (inspect before assuming every file changes)
`workflow_app/forge/agent-runtime-role-runner.ts` (becomes thin factory over
`ForgePhaseAgent` subclasses), `forge-role-mapping.ts` (plan stays), new
`workflow_app/forge/agents/*.ts`. Do not invent a second router.

## Acceptance (targeted)
- Scout without a packet ⇒ phase HOLDs (never silently proceeds).
- Architect without a disposition ⇒ phase HOLDs.
- Each phase still records run row + cost + prior-phase handoff.
- Existing single-role drive (`--until role`), scout-findings test, and the
  exact-candidate Assay all still pass.
