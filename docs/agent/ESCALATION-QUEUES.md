# Agent OS Queues & Escalation Map

**Status:** Active Phase 1 contract  
**Purpose:** Define where work, blocks, human decisions, and handoffs actually live without creating parallel queues.

## Principle

CulebraLuxe already has durable queues and workflow state. Agent OS Phase 1 names how to use them; it does not add markdown task queues beside them.

## Queue map

### 1. Executable story queue

**Canonical:** `agent_work_item`

Created from Story Board workflow/status transitions through the existing database trigger path.

Use for work that is ready to be executed autonomously.

Do not duplicate this in `waiting.md`, `tasks.md`, or an Agent OS queue file.

### 2. Role execution queue

**Canonical:** durable `jobs(type='forge.role')`

A READY workflow task becomes a role job through the existing Forge job bridge. JobService owns lease, heartbeat, attempts, backoff, recovery, cancellation, and terminalization.

This queue is role execution reliability, not product/story scheduling. Do not merge it with `agent_work_item`.

### 3. Waiting on Chris / human decision

**Canonical:** the Workflow `hold` node and its structured resolution.

The HOLD contract already preserves:

- reason;
- evidence;
- originating node;
- requested resolution;
- resume target.

Use HOLD when executable work cannot safely continue until Chris (or another explicit human owner) makes a decision.

A HOLD waiting on Chris should present:

```text
DECISION REQUIRED
WHY OWNER AUTHORITY IS REQUIRED
EVIDENCE
RECOMMENDATION
CREDIBLE ALTERNATIVES
WHAT EACH ANSWER UNBLOCKS
RESUME TARGET
```

Do not create a second `waiting_on_chris.md` queue as an alternative source of truth.

### 4. Architecture escalation

**Canonical path:** current role -> Lead -> Principal Architect -> resolved story contract or HOLD.

Use when:

- service/data/state ownership is ambiguous;
- existing architecture appears incompatible with the requirement;
- implementing the obvious fix would violate a binding boundary;
- the story requires a cross-project architecture decision.

If Principal Architect resolves the question technically, return the decision to the normal Forge Architect/Lead path.

If product/business choice remains, HOLD for Chris.

### 5. Strategic escalation

**Canonical path:** Chief of Staff / role -> Advisor -> Council if warranted -> Chris if owner authority remains.

Use for:

- build-vs-buy;
- platform/vendor direction;
- effort/value sequencing;
- material opportunity-cost decisions.

A strategic recommendation does not itself change workflow state or grant implementation authority.

### 6. Blocked autonomous worker

**Canonical behavior:** `FAILFAST_ESCALATION_POLICY.md` + workflow/job terminal state or HOLD according to recoverability.

An autonomous worker performs bounded diagnosis and only low-risk reversible recovery inside scope.

Escalate rather than broaden scope when:

- the same failure repeats after reasonable retry;
- root cause remains ambiguous;
- required access/credential/provider interaction is missing;
- the fix materially expands story scope;
- architecture judgment is required beyond captured guidance;
- data integrity/security/production/destructive risk appears;
- execution is hung or makes no meaningful progress within the configured limit.

A failed story should cost one story, not the rest of the queue.

### 7. Cross-session handoff

**Canonical:** `docs/agent/HANDOFF-<topic>-<date>.md` in the shape of `HANDOFF-TEMPLATE.md`, pushed as required by house rules.

Use when a human/model session stops before the work is fully complete.

This is continuity, not workflow state. It does not replace Story Board/Neon status.

### 8. Intra-workflow agent handoff

**Canonical:** durable workflow fields, role contracts, plan rows, tool artifacts, packet/evidence mechanisms owned by Forge.

A role-to-role handoff that matters to execution must not exist only in a model chat message.

The receiving role starts from the durable contract/evidence the workflow supplies.

### 9. Short-window proposal lane

**Canonical:** `docs/agent/PROPOSALS.md` contract.

Use when a productive worker cannot complete the required integration/gate inside its execution window. A proposal is a bounded handoff with explicit verification state, not a hidden branch or pseudo-queue.

## Escalation ladder

Default ladder:

```text
LEVEL 0 — role resolves inside its authority and scope
LEVEL 1 — Lead resolves execution/ownership/sequencing
LEVEL 2 — Principal Architect or Advisor resolves architecture/strategy
LEVEL 3 — Expert Council resolves consequential multi-domain choice
LEVEL 4 — Chris resolves product intent or owner-only authority
```

Always use the lowest level that actually owns the decision.

## Direct escalation conditions

The ladder is not bureaucracy. Jump directly to the required owner when policy says so.

Examples:

- explicit production authorization -> Chris / canonical production owner;
- destructive canonical data operation -> Chris;
- QA finding -> implementation owner, never QA self-repair;
- release action -> DEV_OPS/publisher path;
- security boundary conflict -> security/architecture authority, then owner if policy change is required.

## HOLD vs failure

Use **HOLD** when the workflow can continue after an explicit decision or missing prerequisite is supplied.

Use **failure/error terminalization** when execution has failed under the existing failure policy and there is no valid automatic continuation in the current attempt.

Do not mark Complete to clear a blocked queue.

## Queue projection for humans

A future UI/report may present convenient projections such as:

- WAITING ON CHRIS;
- BLOCKED;
- ESCALATED;
- READY;
- ACTIVE.

Those are views over canonical Story Board/workflow/job/HOLD state. They must not become another state store.

## Freshness rule

For current execution status, read live control-plane rows. Documentation may explain semantics, but a markdown file does not outrank the database about what is currently queued, running, blocked, held, or complete.