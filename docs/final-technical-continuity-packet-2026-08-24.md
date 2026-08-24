# CULEBRALUXE FINAL TECHNICAL CONTINUITY PACKET

**Date:** 2026-08-24  
**Purpose:** Preserve technical implementation knowledge from the prior architecture/build session.  
**Scope:** Do not reinterpret the business model here. This packet focuses on the command bus, mini-MQ, CRM-27/26 wiring, Git/Neon topology, code seams, defects, testing, and next implementation order.

Where something was verified from the current repository, it is marked **VERIFIED**. Where connector state prevents confirmation, it is marked **UNVERIFIED / MEMORY**.

---

## 1. CANONICAL COMMAND PATTERN + MINI-MQ — WHAT ACTUALLY EXISTS

### 1.1 Command architecture — VERIFIED

The canonical application command runtime is already implemented under:

`lib/commands/`

Important files:

- `lib/commands/contracts.ts`
- `lib/commands/dispatcher.ts`
- `lib/commands/registry.ts`
- `lib/commands/register.ts`
- `lib/commands/domain-events.ts`
- `lib/commands/command-types.ts`
- handlers under:
  - `lib/commands/deal/`
  - `lib/commands/offer/`
  - `lib/commands/task/`
  - `lib/commands/document/`
  - `lib/commands/signature/`
  - `lib/commands/interaction/`

The central runtime is `CommandDispatcherImpl` in `lib/commands/dispatcher.ts`.

Its actual transaction model is:

```text
CommandEnvelope
    ↓
resolve handler from registry
    ↓
BEGIN application transaction
    ↓
read command receipt by commandId
    ↓
terminal receipt exists?
    ├─ yes → replay result, DO NOT rerun mutation
    └─ no
         ↓
       handler
         ↓
       canonical domain service
         ↓
       canonical mutation
         ↓
       claim/finalize command receipt
         ↓
       collect DomainEvents
         ↓
       append events to outbox_message
       IN THE SAME TRANSACTION
         ↓
       COMMIT ONCE
```

Infrastructure failure throws and rolls the whole application transaction back. Retrying the same `commandId` is the recovery mechanism. A committed receipt is what makes replay truthful.

Key rule: **No external side effect belongs inside the application mutation transaction. External effects happen from outbox subscribers after commit.**

### 1.2 Command idempotency — VERIFIED

Existing Deal/application services generally use the claim-first receipt pattern backed by `workflow_command_receipt`.

The dispatcher first checks a receipt for replay, while legacy/current domain services still own claim/finalize inside the transaction. Do **not** add a second receipt mechanism.

### 1.3 Transactional outbox / mini-MQ — VERIFIED

A real Postgres broker already exists under:

`lib/mq/`

Files:

- `lib/mq/types.ts`
- `lib/mq/broker.ts`
- `lib/mq/outbox-repository.ts`
- `lib/mq/proof-consumer.ts`
- `lib/mq/index.ts`

Migration:

`db/migrations/049_mq_broker.sql`

The durable tables are:

```text
outbox_message
mq_subscription
mq_delivery
mq_proof_effect   -- proof/diagnostic only, NOT business truth
```

The MQ tables are transport truth only. They are not canonical CRM/workflow state.

### 1.4 `outbox_message` — VERIFIED

One row per committed domain event:

```text
id UUID PK
event_type
aggregate_type
aggregate_id
correlation_id
causation_id
actor_app_user_id
occurred_at
payload JSONB
created_at
```

There is one canonical payload copy. Subscriber deliveries reference it rather than duplicating payloads.

### 1.5 `mq_subscription` — VERIFIED

One row per registered consumer:

```text
id TEXT PK
routing_key
description
max_attempts default 5
retry_backoff_seconds default 30
enabled
created_at
```

Routing in V1 is exact routing-key match. The routing key is the domain event's `event_type`.

There is no JMS-style topic hierarchy or wildcard grammar. Do not invent one.

### 1.6 `mq_delivery` — VERIFIED

One independent row per `message × subscription`.

States:

```text
pending
claimed
delivered
failed
dead
```

Important fields:

```text
attempt_count
claimed_at
claimed_by
lease_until
available_at
acknowledged_at
last_error
```

Unique constraint:

```text
(message_id, subscription_id)
```

So materialization is idempotent.

### 1.7 Claim/recovery semantics — VERIFIED

`PostgresMessageBroker` in `lib/mq/broker.ts` implements the runtime.

Defaults:

```text
MQ_LEASE_SECONDS = 300
MQ_DEFAULT_LIMIT = 20
```

A dispatch pass performs:

```text
1. materialize missing deliveries
2. claim due rows
3. execute independent consumers
4. ACK / retry / dead-letter
```

Claim query includes:

```text
pending and available_at <= now()
OR
failed and available_at <= now()
OR
claimed and lease_until <= now()
```

with `FOR UPDATE SKIP LOCKED`.

Overlapping workers cannot double-claim the same delivery, and a crashed worker's stale lease becomes reclaimable.

### 1.8 Retry behavior — VERIFIED

Consumer contract:

```ts
interface MqConsumer {
  subscriptionId: string
  routingKey: string
  maxAttempts: number
  retryBackoffSeconds: number
  handle(message, ctx): Promise<void>
}
```

A thrown consumer error means failed delivery. If attempts remain, state becomes `failed` and `available_at` moves forward by the configured backoff. When attempts are exhausted, state becomes `dead`.

Delivery is **at least once**, not exactly once. Business effects therefore must be idempotent at the consumer side using existing command receipts/correlation.

### 1.9 Outbox repository — VERIFIED

`PostgresOutboxEventRepository` implements the durable `OutboxEventRepository`.

`append(events, tx)` writes to `outbox_message` using the same `tx` passed from `CommandDispatcher`.

It uses `ON CONFLICT(id) DO NOTHING`, so a duplicate event ID cannot create a second canonical message.

**This is the bus. Do not build another one.**

---

## 2. INTENDED CRM-27 → CRM-26 END-TO-END WIRING

### 2.1 What CRM-27 should ultimately produce

The important committed business fact is:

```text
AGREEMENT_FULLY_EXECUTED
```

for one immutable issued agreement version.

The event must ultimately enter:

```text
CommandDispatcher
    ↓
DomainEvent collector
    ↓
PostgresOutboxEventRepository.append(...)
    ↓
outbox_message
```

in the same transaction that records the canonical execution marker.

### 2.2 Correct producer path

```text
provider callback / poll / manual evidence
    ↓
neutral signature/document reconciliation
    ↓
canonical agreement-execution command
    ↓
CommandDispatcherImpl
    ↓
agreement execution domain service
    ↓
evaluate required evidence
    ↓
if first fully-executed transition:
    insert agreement_execution marker
    emit AGREEMENT_FULLY_EXECUTED
    ↓
CommandDispatcher eventSink
    ↓
outbox_message
    ↓ COMMIT
```

The exact new command name was not finalized. Do not pretend one currently exists unless code review finds it.

Architectural invariant: **marker + event + command receipt must participate in one durable application transaction.**

### 2.3 MQ dispatch

A CRM-26 consumer should register an exact subscription for `AGREEMENT_FULLY_EXECUTED`.

Broker path:

```text
outbox_message(event_type = AGREEMENT_FULLY_EXECUTED)
    ↓
mq_subscription exact routing match
    ↓
mq_delivery
    ↓
PostgresMessageBroker.dispatchOnce()
    ↓
CRM-26 consumer.handle()
```

### 2.4 CRM-26 consumer responsibilities

The consumer should:

1. load the immutable P&S/version referenced by the event;
2. verify it is the intended P&S document type and linked Deal;
3. load its structured issued snapshot;
4. project only approved fields;
5. invoke existing canonical Deal commands;
6. complete the existing `pns_executed` workflow task;
7. return success only after the whole business reaction is complete.

Approved structured mappings from the architecture packet:

```text
closingDate
    → deal.set_closing_date

inspectionDeadline
    → deal.set_inspection_deadline

financingDeadline
    → deal.set_financing_deadline

financing
    → deal.set_financing_type

appraisalWaived
    → inverse of deal.set_appraisal_required
```

Do NOT include `purchasePrice` or `surveyDeadline` until their explicit business decisions are applied.

### 2.5 Idempotent consumer command IDs

Because MQ is at-least-once, CRM-26 should generate deterministic command IDs from the incoming message/event identity plus operation, for example conceptually:

```text
<messageId>:closing-date
<messageId>:inspection-deadline
<messageId>:financing-deadline
<messageId>:financing-type
<messageId>:appraisal-required
```

Invariant: re-delivering the same MQ message must replay existing command receipts rather than repeat business effects.

### 2.6 `pns_executed` task completion

Reuse the existing workflow application task machinery.

Known seam:

`workflow_app/task-completion.ts`

Exports:

```text
completeWorkflowTaskCore(...)
completeWorkflowTask(...)
```

It takes the canonical application task ID, looks up the correlated engine task in `workflow_task_correlation`, then calls the engine's normal `WorkflowEngine.completeTask(...)`.

Do not bypass this with SQL against engine task tables.

The consumer needs a narrow deterministic lookup for the active P&S workflow task for the Deal if one does not already exist.

Expected path:

```text
CRM-26 consumer
    ↓
find canonical task correlated to active pns_executed engine task
    ↓
completeWorkflowTask(...)
    ↓
WorkflowEngine.completeTask(...)
    ↓
XML transition
    ↓
mark_under_contract command-node
    ↓
deal.set_stage_under_contract
```

The existing XML remains authoritative.

### 2.7 MQ ACK boundary

The CRM-26 MQ delivery should be marked delivered only after:

```text
all approved Deal projections succeed
AND
pns_executed workflow task advances successfully
```

If any step throws, the consumer throws and `mq_delivery` moves to failed/retry. Existing Deal command receipts make sub-command replay safe.

---

## 3. DEFECTS / INCOMPLETE SEAMS IN `6f43e5a`

Commit:

`6f43e5aa6b850be2adacf07cbd9067dd6f3ff896`

Message:

`CRM-27: Agreement Execution Predicate (provider-neutral, exactly-once)`

The commit is useful, but CRM-27 is not architecturally complete despite the commit summary sounding close to complete.

### Defect 1 — It creates `shouldEmit`, but does not durably publish the event

`lib/agreements/completion.ts` computes `shouldEmit: true` after successfully claiming `agreement_execution`, but that is only a returned boolean.

There is no demonstrated path in this commit that writes `AGREEMENT_FULLY_EXECUTED` into `outbox_message`.

Adding the event type to a TypeScript union is not event publication.

### Defect 2 — Marker and future event are not atomic

`claimAgreementExecution(...)` defaults to its own `neonTx`.

If future code does:

```text
claim marker
COMMIT
then
publish event
```

a crash between those steps produces:

```text
marker exists
event absent
```

On retry, the unique marker can cause `shouldEmit = false`, permanently losing the event.

Therefore the marker and outbox append must move into the same command transaction.

### Defect 3 — Event type appears on the wrong conceptual contract seam

The commit summary says `AGREEMENT_FULLY_EXECUTED` was added to `lib/workflow/contracts.ts`, while canonical DomainEvents/outbox are owned by `lib/commands/contracts.ts` and consumed by the CommandDispatcher / MQ stack.

Avoid duplicated event vocabularies.

### Defect 4 — Manual execution exists in the pure predicate but not the real completion wiring

The pure evaluator supports `manuallyExecuted`, but `evaluateAgreementCompletion(...)` calls it with `manuallyExecuted: false`.

So a real durable/manual evidence repository and application path do not yet exist.

### Defect 5 — Role presence is not participant cardinality

The evidence reader effectively asks whether `BUYER`, `SELLER`, `SELLER_BROKER` roles are present among completed signature requests.

If a P&S has two buyers or two sellers, one completed role token may incorrectly satisfy the entire role.

Execution evidence must be participant/signature-slot aware, or the policy must explicitly define role-level satisfaction.

### Defect 6 — Required-role default is explicitly not authoritative

The current policy defaults to all declared signature groups required as a non-authoritative fixture.

CRM-27 is not business-complete until the required execution policy is resolved/configurable.

### Defect 7 — Single active signature request constraint creates sequential evidence

Current `signature_request` semantics appear to assume sequential role requests. That may be acceptable operationally, but it is not the same as native multi-signer envelope completion.

Do not redesign this provider seam casually.

### Defect 8 — Signature reconciliation evaluates completion non-fatally

If agreement-completion evaluation fails transiently, there must be a durable way to reevaluate eligible completed signature evidence later. Do not depend on another webhook eventually arriving.

### Defect 9 — No CRM-26 consumer

No implementation in `6f43e5a` performs:

```text
AGREEMENT_FULLY_EXECUTED
→ Deal projection
→ pns_executed completion
```

### Defect 10 — Migrations 067/068 are branch-only and not proven applied

The commit adds:

```text
067_agreement_execution_role.sql
068_agreement_execution.sql
```

They are not on current `main` and are not confirmed applied in Neon.

### Defect 11 — `agreement_execution` delete cascade deserves scrutiny

The new marker FK uses `transaction_document(id) ON DELETE CASCADE`.

For an immutable/auditable execution fact, disappearing because a referenced document is deleted is architecturally questionable. Review rather than blindly preserving.

---

## 4. CRM-25 VS EXISTING MINI-MQ

### What already exists — VERIFIED

MQ-01 already provides the generic reliable internal transport:

```text
DomainEvent
→ transactional outbox
→ outbox_message
→ subscription
→ mq_delivery
→ lease
→ retry
→ dead
→ independent consumer
```

Therefore CRM-25 is **not** “build the message queue.”

### What CRM-25 still adds

CRM-25 is the higher-level outbound external action / correlation loop:

```text
Business intent
    ↓
durable outbound request
    ↓
MQ transport
    ↓
provider adapter
    ↓
external provider ID/reference
    ↓
later external observation
    ↓
Integration Inbox
    ↓
correlation back to original CRM intent/person/deal/command
```

Distinction:

```text
MQ
= reliable internal transport

CRM-25
= semantic outbound-action lifecycle + external correlation
```

CRM-25 must preserve correlation ID, causation ID, originating command/business intent, provider/external reference, retry/idempotency strategy, and observed external response mapping.

It should use `outbox_message / mq_delivery` for delivery reliability and the existing Integration Inbox for inbound observed facts.

### OPS-11C relationship

```text
OPS-11C
= WHO should receive attention, WHY, WHEN, severity/escalation

CRM-25
= send external action reliably, adapter/provider execution, external reference, retry, correlation

MQ
= durable transport, claim, lease, retry, dead-letter
```

Correct chain:

```text
OPS-11C
→ CRM-25 outbound notification intent
→ existing MQ
→ email/provider adapter
```

Do not create `notification_queue`, `alert_queue`, or `email_queue`.

---

## 5. CURRENT GIT TOPOLOGY

### VERIFIED

Architecture document:

`docs/deal-workflow-architecture-specs-2026-08-24.md`

Commit:

`92d271d051e3fb8ec121f3eeabc9f7de5f2c0c6e`

CRM-27 Cline commit:

`6f43e5aa6b850be2adacf07cbd9067dd6f3ff896`

The commits are siblings, not parent/child.

Merge base:

`901642fe2dde935784bfecb0192fc81c6c7dea96`

Topology:

```text
                    92d271d
                   /        architecture document / main
901642f ----------
                   \
                    6f43e5a
                         CRM-27 Cline work
```

GitHub reports them as diverged.

### Safest integration sequence

```text
1. preserve 6f43e5a
2. work from current main containing 92d271d
3. cherry-pick or rebase the CRM-27 changes onto main
4. repair CRM-27 defects there
5. keep repair commits separate
6. only then proceed to CRM-26
```

### Branch name — UNVERIFIED

The exact branch Cline used is not safely known from this packet.

### Local uncommitted work — UNVERIFIED

GitHub cannot see the local working tree. Run local `git status` before any integration operation.

---

## 6. CURRENT NEON STATE

### Last known working connection

Project ID used successfully:

```text
billowing-snowflake-76768657
```

Database:

```text
neondb
```

Calls were made without explicit branchId, so they targeted the connector's default branch. Exact Neon branch name was not queried.

### Story Board — previously verified

Canonical table:

```text
storyboard_story
```

Execution history:

```text
storyboard_story_run
```

Known `storyboard_story` columns include:

```text
id
workstream
title
priority
status
notes
batch
goal
scope
acceptance_criteria
dependencies
created_at
updated_at
completion
rollup
planned_start_at
actual_start_at
completed_at
preconditions
architect_brief
context_refs
postconditions
architect_brief_updated_at
operating_surface
```

OPS-11 was changed from stale `Failed` to `Planned`, `rollup = true`, with notes decomposing the work into OPS-11A/B/C.

### Six new story rows — VERIFY

We attempted to create/fill:

```text
CRM-26
CRM-27
CRM-28
OPS-11A
OPS-11B
OPS-11C
```

Connector instability means their exact current state must be reread from Neon.

### Architecture table — UNKNOWN

The exact architecture table name/columns were not safely established before the connector failed. Do not invent them.

The durable replacement is the Git architecture document at `92d271d`.

### Connector failure

Current Neon connector problem was an HTTP 404 / internal authorization failure. Permission policy itself appeared correct, so this should not be interpreted as proof of a Neon database outage.

### Migration state

Main repo currently contains migrations through `066_forms_engine_context.sql` plus `049_mq_broker.sql` and all relevant existing application migrations.

CRM-27 adds:

```text
067_agreement_execution_role.sql
068_agreement_execution.sql
```

only on commit `6f43e5a`.

067/068 are **not confirmed applied to Neon**.

Committed migration files do not prove a migration was applied to a particular Neon branch.

---

## 7. EXACT EXISTING CODE SEAMS TO REUSE

### Workflow application boundary

`workflow_app/application-port.ts`

Export:

```text
createApplicationPort()
```

Behavior:

```text
engine command
→ toCommandEnvelope
→ routeCommand
→ canonical CommandDispatcher
```

and:

```text
readFacts(deal)
→ getDealWorkflowFacts
```

### `workflow_app/engine-bridge.ts`

Exports:

```text
toCommandEnvelope(...)
toApplicationCommandResult(...)
CulebraLuxeApplicationPort
```

Translation seam only; no brokerage rules here.

### `workflow_app/command-router.ts`

Existing workflow → canonical command dispatch seam.

Do not make workflow engine import domain services.

### Workflow start

`workflow_app/runtime.ts`

Exports:

```text
findActiveInstance(...)
startResidentialTransactionWorkflow(...)
reconcileResidentialTransactionWorkflows(...)
```

Important rule: accepted offer starts workflow, not signed P&S.

`workflow_app/start-core.ts`

Exports:

```text
startWorkflowCore(...)
isUniqueViolation(...)
```

Handles concurrent start race.

`workflow_app/reconcile.ts`

Exports:

```text
reconcileWorkflowsCore(...)
reconcileWorkflows(...)
```

Current pass starts missing residential workflows and materializes missing tasks. This is a useful future hook for durable reconciliation.

### Workflow facts

`workflow_app/facts.ts`

Primary export:

```text
getDealWorkflowFacts(...)
```

### Workflow task materialization

`workflow_app/task-materialization.ts`

Exports:

```text
materializeEngineTaskCore(...)
materializeEngineTask(...)
```

Current behavior:

```text
engine task
→ canonical task
→ workflow_task_correlation
```

It explicitly does not own notification scheduling.

### Workflow task completion

`workflow_app/task-completion.ts`

Exports:

```text
completeWorkflowTaskCore(...)
completeWorkflowTask(...)
```

Current flow:

```text
applicationTaskId
→ correlated workflowTaskId
→ WorkflowEngine.completeTask(...)
```

CRM-26 must reuse this seam.

### Responsibility

`workflow_app/responsibility.ts`

Exports:

```text
resolveResponsibility(...)
resolveParticipantTarget(...)
```

Current vocabulary already handles brokerage, buyer, seller, lender, inspector, appraiser, notario, title_company, other_sme.

OPS-11C should reuse it.

### Workflow read model

`workflow_app/read-service.ts`

Exports:

```text
getWorkflowSummaries(...)
getWorkflowDetail(...)
```

Already exposes current nodes, milestones, open task count, pending timer count, blocker count, responsible party, next expected action, display order, node labels/descriptions, and events.

OPS-11A should compose this rather than rebuilding workflow state.

### Canonical command runtime

`lib/commands/dispatcher.ts` → `CommandDispatcherImpl`

`lib/commands/register.ts` → canonical registrations

`lib/commands/registry.ts` → handler registry

`lib/commands/contracts.ts` → canonical command/result/domain-event contracts

### MQ

`lib/mq/broker.ts` → `PostgresMessageBroker`

`lib/mq/outbox-repository.ts` → `PostgresOutboxEventRepository`

`lib/mq/types.ts` → `MqMessage`, `MqDeliveryContext`, `MqConsumer`, `MqDeliveryState`, `MqDispatchSummary`

### Existing Deal commands

Known canonical seams already exist for:

```text
deal.set_stage_under_contract
deal.set_stage_closed
deal.set_closing_date
deal.set_financing_type
deal.set_appraisal_required
deal.set_lender_clear_to_close
deal.set_inspection_deadline
deal.set_financing_deadline
```

Do not write these columns directly from CRM-26.

### Offer acceptance

`db/offer-acceptance.ts`

Accepting an Offer changes the Offer to `accepted`. It does **not** itself set Deal.stage under contract. Workflow does that later after P&S.

---

## 8. TEST STRATEGY ACTUALLY AGREED

Correct any claim that full regression is required after every story. It is not.

Agreed strategy: targeted tests during development, broader regression only at meaningful architecture checkpoints.

### CRM-27 targeted

```text
partial evidence
final required evidence
duplicate evidence replay
wrong document/version
manual/external evidence
required vs optional role policy
participant multiplicity/cardinality once repaired
signature reconciliation integration
outbox atomicity
```

### CRM-26 targeted

```text
AGREEMENT_FULLY_EXECUTED consumer
P&S snapshot loading
each approved Deal projection command
duplicate MQ delivery
partial consumer failure/retry
workflow task correlation
pns_executed completion
existing mark_under_contract transition
```

### Meaningful broad checkpoint #1

After:

```text
CRM-27 repaired
+
CRM-26 implemented
+
E2E accepted Offer → executed P&S → under contract proof
```

Then run the appropriate broader workflow/forms/signature regression.

### Meaningful broad checkpoint #2

After the entire Deal/attention batch is complete.

There is **no architectural requirement for a full-site regression after each commit**.

---

## 9. KNOWN TRAPS / NON-OBVIOUS LESSONS

1. **P&S does not start the workflow.** Accepted Offer starts it. P&S execution advances an already-running workflow.
2. **`deal.stage` is not workflow state.** XML/token state is the rich transaction process.
3. **The command is the envelope, not the business rule.** Handlers should remain thin; domain services own legality and canonical mutations.
4. **Do not build CRIM/title/appraisal/lender systems.** Workflow observes their outcomes/work products and watches clocks.
5. **Do not create another MQ.** The broker is already real and durable.
6. **Do not publish events after commit when the event corresponds to a state mutation.** The outbox exists precisely to avoid state committed/event lost.
7. **`shouldEmit` is not publication.** This is the central defect in current CRM-27.
8. **Provider completion is not agreement completion.** `SIGNATURE_REQUEST_COMPLETED` is evidence, not the P&S business rule.
9. **Multiple people can share a role.** BUYER is not necessarily one human.
10. **Human-confirmed state is a first-class source of truth for external processes.**
11. **ACK is not gate completion.** Attention state and transaction state are orthogonal.
12. **No fake warning timers.** Contract dates belong to workflow timers; T-7/T-3 warnings belong to attention policy.
13. **Do not attempt distributed ACID between engine/application worlds.** Use recoverable/idempotent handoffs.
14. **Signature/email providers are operationally ugly.** Keep provider details behind adapters.
15. **Migrations committed != migrations applied.** Especially 067/068.
16. **Architecture doc and CRM-27 are sibling commits.** Integrate deliberately before continuing.

---

## 10. EXACT NEXT CLINE WORK ORDER AFTER CRM-27 REVIEW

Do **not** simply continue CRM-26 from `6f43e5a` unchanged.

First repair the CRM-27 durability seam, then implement CRM-26 as a separate commit.

### PHASE 1 — CRM-27 DURABILITY REPAIR

**Goal:** Turn the CRM-27 agreement execution predicate from a useful local predicate + marker into a correctly durable canonical event producer using the existing CommandDispatcher + transactional outbox + Postgres MQ architecture.

Reuse:

```text
lib/commands/dispatcher.ts
lib/commands/contracts.ts
lib/commands/register.ts
lib/commands/registry.ts
lib/commands/domain-events.ts
lib/events/outbox-contracts.ts
lib/mq/outbox-repository.ts
lib/mq/broker.ts
lib/mq/types.ts
db/workflow-command-receipt.ts
existing signature reconciliation/application seam
```

Required architecture:

```text
provider/manual execution evidence
    →
canonical agreement execution command
    →
CommandDispatcher transaction
    →
evaluate immutable document/version evidence
    →
claim agreement_execution
    →
if newly fully executed:
    emit canonical AGREEMENT_FULLY_EXECUTED DomainEvent
    →
CommandDispatcher eventSink
    →
PostgresOutboxEventRepository.append(event, SAME tx)
    →
commit once
```

The command receipt, agreement_execution marker and outbox event must share the same application transaction.

Manual evidence must have a real application-owned seam or remain explicitly blocked. Do not claim it complete solely because a pure unit test accepts a boolean.

Inspect the Forms participant/signature model. Do not assume one `execution_role='BUYER'` proves all buyer participants signed. If the current lineage cannot distinguish participant evidence, stop and report the smallest missing evidence key/schema seam.

Do not decide whether SELLER_BROKER is legally required. Keep required/optional policy explicit/configurable.

A transient failure while evaluating agreement completion must be recoverable through an idempotent reconciliation path.

Phase 1 acceptance:

```text
1. First fully executed agreement version atomically produces:
   - canonical agreement_execution marker
   - command receipt
   - exactly one outbox_message AGREEMENT_FULLY_EXECUTED

2. Transaction rollback leaves neither marker nor outbox event.

3. Duplicate evidence/callback/reconciliation produces no second business event.

4. PostgresMessageBroker can materialize the event for a test consumer.

5. Required-role policy remains explicit, not silently decided.

6. Participant multiplicity is either correctly supported or reported as a hard stop with exact missing evidence.

7. No second queue/event store is introduced.
```

Commit Phase 1 separately.

Suggested commit message:

`CRM-27: make agreement execution event transactional and MQ-ready`

### PHASE 2 — CRM-26 AGREEMENT_FULLY_EXECUTED CONSUMER

Begin only after Phase 1 passes.

Implement an `MqConsumer` with exact routing key `AGREEMENT_FULLY_EXECUTED`.

Do not create a queue table.

Event payload must contain or allow deterministic resolution of immutable `transaction_document` id, issued version, template/document type, Deal linkage, and correlation/causation identity.

Load the immutable issued structured form/document snapshot and project only:

```text
closingDate
  -> deal.set_closing_date

inspectionDeadline
  -> deal.set_inspection_deadline

financingDeadline
  -> deal.set_financing_deadline

financing
  -> deal.set_financing_type

appraisalWaived
  -> inverse of deal.set_appraisal_required
```

Stop signs:

```text
DO NOT write purchasePrice.
DO NOT create/use surveyDeadline canonical state.
```

For each subcommand use a deterministic commandId derived from the incoming event/message identity so MQ redelivery replays the existing command receipt.

After all approved projections succeed, locate the existing application task correlated to the active `RE_supermodel` `pns_executed` task for this Deal.

Reuse:

```text
workflow_app/task-completion.ts
workflow_task_correlation
WorkflowEngine.completeTask()
```

Completing `pns_executed` must allow existing XML to execute:

```text
mark_under_contract
    →
deal.set_stage_under_contract
```

Do not start a second workflow.

Delivery ACK rule: consumer returns success only after all approved canonical projections succeeded/replayed safely and `pns_executed` advanced or was already proven advanced idempotently. Any incomplete step throws so `mq_delivery` retries.

E2E acceptance:

```text
accepted Offer
    →
existing RE workflow starts
    →
pns_preparation
    →
pns_executed active
    →
fully executed P&S evidence
    →
canonical CRM-27 command
    →
AGREEMENT_FULLY_EXECUTED committed to outbox
    →
MQ delivery
    →
CRM-26 consumer
    →
canonical Deal facts projected
    →
pns_executed completed
    →
existing mark_under_contract command
    →
Deal under_contract
    →
expected transaction fork/gates active
```

Commit CRM-26 separately. If the E2E fixture/proof is substantial, make it a third separate commit.

### Stop conditions

Stop the affected sub-part and report rather than invent semantics if:

```text
1. Required signature role policy requires deciding whether Seller Broker is mandatory.
2. Purchase price mismatch handling is encountered.
3. Survey deadline requires new canonical state.
4. The immutable P&S issued artifact does not contain enough structured snapshot data to perform CRM-26 projection.
5. Participant signature evidence cannot distinguish multiple people in one role.
6. pns_executed cannot be located deterministically through existing task correlation.
```

For #4-#6 report the smallest missing seam and continue independent safe work where possible.

### Test strategy for this work order

Use targeted suites while implementing.

Do not run the entire site regression after every commit.

Required broad regression checkpoint is after:

```text
CRM-27 durability repair
+
CRM-26
+
accepted Offer → P&S → under_contract E2E proof
```

Run the broader suite again only at the later complete Deal batch checkpoint.

### Final report required from Cline

Report:

```text
- commits
- exact files changed
- schema changes
- whether migrations were only created or actually applied in DEV
- targeted test results
- MQ proof
- E2E proof
- stop-sign decisions left unresolved
- idempotency/recovery proof
- any deviation from the architecture document
```

Do not continue into CRM-28 or OPS-11 in the same run unless explicitly asked after review of CRM-26.

---

## FINAL STATE ASSESSMENT

The most important correction for the next architect is:

**The mini-MQ already exists and is substantially implemented.**

The right architecture is not:

```text
signature completion
→ call Deal code directly
```

and it is not:

```text
signature completion
→ make a new event mechanism
```

It is:

```text
canonical agreement-execution command
→ transactional marker + DomainEvent + command receipt
→ existing outbox
→ existing PostgresMessageBroker
→ CRM-26 consumer
→ existing canonical Deal commands
→ existing workflow task-correlation seam
→ existing XML transition
```

`6f43e5a` contains the front half of CRM-27, not the complete durable chain. Repair that boundary before allowing the batch to continue downstream.
