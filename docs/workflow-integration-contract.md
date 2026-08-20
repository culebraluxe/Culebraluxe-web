# CulebraLuxe ↔ Workflow Engine Integration Contract

Status: Pre-CRM-14 plumbing. No engine imported, no workflow DSL, no runtime.

## Independence

The CulebraLuxe application and the workflow engine are independent components
that meet through a deliberately small integration seam.

**Application owns (canonical):**
- domain truth: `person`, `property`, `deal`, `offer`, `showing`, `interaction`, `task`, `app_user`
- domain commands and domain validation
- the security authority boundary (`ActingUser` → roles → authorities)
- the user-facing operational task
- business data and business dates/facts

**Engine owns (generic orchestration runtime):**
- process definition / process instance
- execution token and transition mechanics
- timers / jobs / retries
- generic human-gate runtime
- engine event history
- engine runtime task state (separate from application task)

**Definition/model owns (brokerage policy):**
- brokerage orchestration policy and state/transition model
- deadlines, responsibility, process-specific conditions

## The "Ogden" integration seam

The seam is the [`lib/workflow`](lib/workflow) contract surface:

| Application provides | Type |
|----------------------|------|
| Command envelope + result | [`contracts.ts`](lib/workflow/contracts.ts) |
| Domain event contract + catalog | [`contracts.ts`](lib/workflow/contracts.ts) |
| Command inventory + idempotency + preconditions | [`command-inventory.ts`](lib/workflow/command-inventory.ts) |
| Application facade + workflow adapter interface + fact projection | [`adapter.ts`](lib/workflow/adapter.ts) |
| Task / timer / alert / SME / audit seams | [`operational-contracts.ts`](lib/workflow/operational-contracts.ts) |

The engine (or a future adapter) calls the application only through these
contracts. The application never imports the engine.

## Authority vs domain legality (re-affirmed)

- Authority answers: *"may this actor attempt this command class?"* (`portal.read`, `crm.write`, `listing.write`, `deal.write`, `settings.manage`).
- Domain preconditions answer: *"is this transition legal in the current business state?"* (e.g., offer may only counter a submitted offer).
- The engine may REQUEST a command, but the application remains authoritative for both. No workflow rule may bypass an application precondition or authority check.

## Subject model

`WorkflowSubject = { subjectType, subjectId }`. For CRM-14 V1 the primary
workflow subject is **`deal`** (property and person are referenced facts).

## Correlation / causation

- `workflow instance id → CommandEnvelope.correlationId`
- `CommandEnvelope.commandId → CommandResult.emittedEvents[].causationId`
- `DomainEvent.eventId → .correlationId / .causationId`

Existing `interaction(source_system, source_external_id)` remains the
application-side idempotency key for interactions; it is not repurposed for
engine events. No new columns are added for the initial seam.

## Task boundary

CulebraLuxe `task` remains the canonical user-facing operational work item.
The engine may keep its own runtime task state and correlate via
`TaskCorrelation`. No dual truth without an explicit correlation key.

## Persistence / audit posture

- Existing immutable domain records are the business audit trail.
- `security_audit_event` covers auth/break-glass events only.
- Domain events are initially transient (returned on `CommandResult`); the
  engine keeps its own event log. No `application_event` table is created now.

## Explicit non-goals for this batch

No workflow DSL, no visual modeler, no engine runtime, no timers worker, no
alert delivery, no SME portal, no application/engine schema collapse, no event
sourcing, no RPC framework.
