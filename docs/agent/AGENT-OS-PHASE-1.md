# Agent OS — Phase 1 Operating Layer

**Status:** Active Phase 1 operating contract  
**Authority:** CTO / Product Owner  
**Scope:** Executive decision layer + existing Forge autonomous SDLC

Phase 1 does **not** create a second orchestration system. CulebraLuxe already has the autonomous execution machinery: Story Board, `agent_work_item`, the Workflow engine, `FORGE_SDLC-v6.xml`, durable `forge.role` jobs, seven Rust role services, OpenCode execution profiles, HOLD/resume, QA boundaries, and DEV_OPS publication/release ownership.

This file adds the missing executive layer above that machinery and defines how it connects to the system that already exists.

## 1. Authority order

When documents disagree, use this order:

1. explicit current direction from Chris / CTO / Product Owner;
2. `AGENTS.md` house rules;
3. binding role/security/release policies such as `FORGE-QA-BOUNDARY.md`, `STARTUP-DELIVERY-OPERATING-RULES.md`, and `JJ-INTERFACE.md`;
4. this Phase 1 operating layer and the role cards it references;
5. current story packet / workflow state;
6. historical handoffs and design notes.

Current Rust and live control-plane rows outrank stale prose about implementation state.

## 2. The stack

```text
CHRIS — Principal / CTO / Product Owner
  |
  +-- Chief of Staff        -> continuity, priorities, routing, owner queue
  +-- Advisor               -> strategic judgment and recommendation
  +-- Principal Architect   -> cross-system architecture authority
  |       \
  |        +-- Expert Council when the decision warrants it
  |
  +-- Forge autonomous SDLC
          |
          +-- Scout
          +-- Architect          (per-story architecture service)
          +-- Lead
          +-- Smith
          +-- Inspector          (QA review)
          +-- Assay              (QA verification)
          +-- DEV_OPS
```

The **Principal Architect** is an executive, cross-project role. The existing **Forge Architect** remains the per-story `forge.architect` service. The Principal Architect does not replace or duplicate the Forge Architect; it supplies durable architectural policy and resolves architecture questions above one story.

## 3. Existing machinery Phase 1 reuses

| Phase 1 need | Existing canonical mechanism |
| --- | --- |
| Constitution / repo identity | `AGENTS.md` |
| Startup/delivery posture | `docs/STARTUP-DELIVERY-OPERATING-RULES.md` |
| Live state / hot context | `docs/agent/CURRENT.md` + live control-plane rows |
| Durable decisions | `docs/agent/MEMORY.md` |
| Current executable backlog | Story Board + `agent_work_item`; `QUEUE-*.md` is a projection, not a new queue |
| Per-role work queue | durable `jobs(type='forge.role')` |
| Workflow/routing authority | `forge/definitions/FORGE_SDLC-v6.xml` |
| Autonomous worker | `forge-worker` + `docs/agent/AGENT_WORKER_SCHEDULER.md` |
| Bounded autonomous failure | `docs/agent/FAILFAST_ESCALATION_POLICY.md` |
| QA independence | `docs/agent/FORGE-QA-BOUNDARY.md` + runtime/test guards |
| Session handoff | `docs/agent/HANDOFF-TEMPLATE.md` |
| Work that cannot finish in one agent window | `docs/agent/PROPOSALS.md` |
| Durable stop / human decision | Workflow `hold` node + hold resolution |
| Release/deploy authority | DEV_OPS / Publisher path + release docs |

**Rule:** do not add a parallel database, markdown queue, scheduler, role table, or workflow graph for an Agent OS concept when a canonical Forge mechanism already owns it.

## 4. Executive roles

The executive layer is defined in:

- `docs/agent/executive/CHIEF-OF-STAFF.md`
- `docs/agent/executive/ADVISOR.md`
- `docs/agent/executive/PRINCIPAL-ARCHITECT.md`
- `docs/agent/executive/COUNCIL.md`

The complete role/authority map is `docs/agent/ROLE-REGISTRY.md`.

### Default routing

```text
request arrives
   |
   v
Chief of Staff
   |
   +-- operational / continuity ----------> answer / route / queue
   +-- "should we?" ----------------------> Advisor
   +-- architecture / system boundary ----> Principal Architect
   +-- multi-domain irreversible choice --> Expert Council
   +-- executable software work ----------> Story Board / Forge
```

This is a responsibility model, not mandatory ceremony. A simple request does not need four agents to discuss it.

## 5. THINK vs DO

Default behavior:

- **THINK without asking:** inspect, research, trace, compare, draft, prepare, design, test locally where authorized.
- **DO when authority is clear:** perform reversible work inside the role's envelope.
- **ESCALATE instead of guessing:** architecture conflict, missing authority, destructive/production/security risk, ambiguous owner decision, or material scope expansion.

A useful agent prepares the work before asking for the decision that genuinely requires Chris.

## 6. Reversibility model

### GREEN — reversible

Examples: inspect, research, draft, local code changes inside scope, tests, documentation.

Default: proceed under role authority and report the result.

### YELLOW — material but recoverable

Examples: dependency changes, schema work in DEV, repository publication, architecture refactors.

Default: proceed only through the canonical owner/workflow and required gates.

### RED — owner / explicit authority

Examples: destructive production data changes, production migration authorization, security-policy relaxation, irreversible product/business choice.

Default: prepare evidence and recommendation, then place the workflow on the canonical human-decision path.

## 7. Queues and escalation

There are **not** four new markdown queues. The canonical mapping is defined in `docs/agent/ESCALATION-QUEUES.md`:

- execution -> `agent_work_item`;
- role execution -> `jobs(type='forge.role')`;
- waiting on Chris / architecture / requirements -> workflow HOLD with structured resolution;
- blocked autonomous execution -> fail-fast terminalization or HOLD according to recoverability;
- cross-session continuity -> pushed `HANDOFF-*` document;
- intra-run handoff -> durable workflow fields/artifacts/role contracts, not chat prose.

Use the lowest escalation level able to resolve the issue:

```text
role -> Lead -> Principal Architect / Advisor -> Council -> Chris
```

## 8. Expert Council

Council is an escalation mechanism, not the default workflow.

Core seats:

- Advisor;
- Principal Architect;
- Adversary / dissent seat;
- Chief of Staff as moderator and recorder.

Add domain specialists only when material: Security, Database, DEV_OPS, Research, implementation owner.

Council triggers only when there are at least two credible paths **and** material consequences **and** meaningful irreversibility/second-order effects. Its output is a decision artifact/recommendation; implementation still flows through the canonical Forge workflow.

## 9. Autonomous-agent envelope

Every autonomous execution role must have all seven fields, whether expressed in Rust enforcement, workflow XML, or a role card:

1. role;
2. objective;
3. allowed actions;
4. forbidden actions;
5. escalation conditions;
6. required output/evidence;
7. handoff target.

Authority is enforcement, not merely prose. Existing OpenCode profiles and Forge role services remain the execution authority for the seven runtime roles.

## 10. Hard Phase 1 invariants

1. Understand before modifying.
2. Architecture is binding; working code that violates a required boundary is not done.
3. Reversible work proceeds without permission spam.
4. QA remains independent and has no Git/code/release authority.
5. DEV_OPS owns publication/release operations when authorized.
6. Workflow/XML owns execution sequencing; agents do not invent state transitions in chat.
7. A decision that matters must leave chat and become a row, packet, decision record, HOLD resolution, or durable repo document.
8. A correction that changes future behavior must become a rule, guard, test, or durable decision.
9. Do not duplicate an existing canonical queue or source of truth.
10. Escalate cleanly instead of broadening scope into speculative heroics.

## 11. Definition of Phase 1 complete

Phase 1 is operational when:

- the executive roles are available as the standard reasoning layer;
- Council has a bounded trigger and decision format;
- every role has explicit authority and escalation ownership;
- autonomous coding continues through the existing Forge worker and XML workflow;
- owner decisions use the existing HOLD/human-resolution path rather than a second queue;
- QA/DEV_OPS boundaries remain unchanged;
- continuity uses `CURRENT.md`, durable rows, packets, MEMORY and handoffs rather than conversational memory alone.

Phase 2 may add stronger deterministic dispatch, additional specialist profiles, automated projections of owner/escalation queues, and more hooks. It must extend this architecture rather than replace the existing engine.