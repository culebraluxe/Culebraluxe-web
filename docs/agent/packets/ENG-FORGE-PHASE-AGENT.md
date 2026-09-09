# ENG-FORGE-PHASE-AGENT — Role/Phase Agent abstraction

**Status:** IMPLEMENTED + live-validated (2026-09-08/09). The role/phase lifecycle,
deliverable gate, routing-decision validation, and bounded self-heal now live in
the abstract `ForgePhaseAgent` instead of scattered conditionals in one big
role-runner. Built in four commits:
`704e177` write-on-exit persistence · `8892b6c` routing decisions ·
`ac31d11` self-heal · `847aa21` enforcement ON by default.

## Why

A role node executes through `forge-executor` → the role-runner → a harness
adapter. Deliverables used to be bolted on as `if`-branches (scout packet,
architect disposition, findings parse), each added reactively after a defect.
The parent abstraction makes a role's durable deliverable a **declared invariant
every role inherits** instead of a remembered conditional.

## Files (current truth)

- `workflow_app/forge/agents/forge-phase-agent.ts` — abstract parent +
  pure policy/directive helpers.
- `workflow_app/forge/agents/role-agents.ts` — concrete subclasses
  (Scout/Architect/Lead/Smith/QA/DevOps) + `forgeAgentFor(nodeId)` factory.
- `workflow_app/forge/agent-runtime-role-runner.ts` — the real runner routes the
  marshal/deliverable/self-heal tail through the agent.
- Tests: `workflow_app/tests/forge-phase-agent.test.ts`,
  `workflow_app/tests/forge-evidence-roundtrip.test.ts` (DB-backed).

## The contract the parent owns

| Concern | Member | Behavior |
|---|---|---|
| Raw output | `rawRoleOutput(notes, testsSummary)` | notes + tests summary = what a deliverable is built from |
| Deliverable kind | `deliverableKind()` | scout-packet / architect-plan / lead-decision / smith-candidate / qa-verdict / devops-receipt |
| Findings parse | `marshalFindings()` | `FORGE_FINDINGS_JSON` → evidence.findings (Scout + Architect) |
| Lead shape override | `applyLeadShape()` | validateLeadShapeChoice / findings-driven lead shape |
| **Write-on-exit** | `storyDeliverable(raw)` | centralizes the durable Story field a role writes on exit — Scout → `context_refs`, Architect → `architect_brief`. The runner persists it generically; **never gated on a model self-formatting a marker**. New roles that hand off free text add one field here. |
| Deliverable gate | `missingDeliverables(evidence, raw, scoutSet, architectSet)` | real presence check per flavor |
| **Routing decision** | `routingDecisionMissing(evidence)` | research_architect must emit valid `research_disposition` (IMPLEMENT\|ARCHIVE\|HOLD); lead must emit valid `lead_decision` (SMITH\|SPLIT\|HOLD\|SOLO, `splitCount>1` when SPLIT). A persisted plan is NOT enough to route — this is the decision gate. |
| Self-heal | `buildSelfHealDirective` / `parseDeliverableRepromptBudget` | corrective re-prompt text + attempt budget |
| Enforcement policy | `deliverableEnforcementEnabled(env)` | **ON by default**; disable with `FORGE_ENFORCE_DELIVERABLES=0` |

## Execution flow in the runner

```
enforceDeliverables = deliverableEnforcementEnabled(env)   // default ON
totalAttempts = enforce ? 1 + FORGE_DELIVERABLE_RETRIES : 1  // default 1 retry
for attempt in 0..totalAttempts-1:
  build plan/prompt (+ correctiveNote if self-heal)  →  enqueue+claim+execute
  marshal evidence, agent.marshalFindings + applyLeadShape
  successful?  → write-on-exit storyDeliverable → context_refs / architect_brief
  miss = agent.missingDeliverables(...) + routingDecisionMissing(...)
  miss empty                    → finish task + Complete
  attempt < totalAttempts - 1   → correctiveNote = selfHeal; continue (reprompt)
  else                          → throw real HOLD (engine releases)
```

Hard runtime failures / interruptions **throw immediately** (never reprompted —
not a fixable miss). Only an otherwise-successful run that missed a deliverable or
decision self-heals.

## Enforcement policy

- **Default ON.** A successful role that reports without its deliverable or a
  valid routing decision is re-run up to `FORGE_DELIVERABLE_RETRIES` (default 1)
  times with a corrective directive, then HOLDed.
- Disable leniently: `FORGE_ENFORCE_DELIVERABLES=0` (or `false`/`off`).
- Budget: `FORGE_DELIVERABLE_RETRIES` (default 1 → max 2 model runs per phase).

## Deliverables vs. persistence

| Agent | deliverable | persisted to |
|---|---|---|
| ScoutAgent | non-empty research packet | `story.context_refs` (write-on-exit) |
| ArchitectAgent | valid `research_disposition` + plan | `story.architect_brief` (write-on-exit) + disposition in `forge_workflow_evidence` |
| LeadAgent | valid decision + split count | `forge_workflow_evidence` |
| SmithAgent | candidate SHA | run `commit_hash` / `candidate_sha` |
| QAAgent | exact-SHA PASS/FAIL | assay evidence / `qa_passed` |
| DevOpsAgent | release/prod receipt | `forge_workflow_evidence` |

Subclasses keep their specific permissions + model grade (flash/pro); lifecycle,
marshal, persist, and self-heal are shared and correct-by-construction.

## Live proof (SIMPLE-RUN-03, cleaned up, ~6.4 widgets)

Scout → Architect → Lead with enforcement ON + self-heal:
Scout packet persisted (context_refs 5115). **Architect #1** persisted the plan
(architect_brief 5118) but omitted `research_disposition` → self-heal reprompted
**Architect #2**, which delivered `IMPLEMENT` → gate passed → advanced. Lead made a
valid `HOLD` → correctly parked at the human gate. No operator needed for the
Architect miss.

## Future home

`ForgePhaseAgent.gatherPriorContext()` is where V5-21 story-resident OpenCode
session continuity ("context persists; authority does not") plugs in — still
parked (see `ENG-FORGE-V5-21.md`).
