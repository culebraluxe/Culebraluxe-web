# Deal Workflow Architecture Specifications

**Date:** 2026-08-24  
**Scope:** CRM-26, CRM-27, CRM-28, OPS-11A, OPS-11B, OPS-11C  
**Status:** Architecture-ready; implementation sequencing defined below.

## 0. Architectural Context

The residential transaction architecture is already structurally complete enough that the remaining work should be treated as reconciliation and wiring, not as a redesign of the workflow engine.

The canonical lifecycle is:

```text
Accepted Offer
    -> RE_supermodel starts
    -> P&S preparation
    -> P&S execution
    -> canonical Deal facts are enriched from the executed agreement
    -> mark Deal under contract
    -> parallel transaction obligations/gates/timers
    -> closing readiness
    -> closing
    -> post-closing/recording
```

The system boundary is intentional:

- **Forms XML** describes the agreement and captures structured operational terms plus legal prose.
- **The executed PDF / signed document lineage** is the immutable agreement artifact.
- **Canonical Deal/application state** stores only the operational facts the application must reason about after execution.
- **Workflow XML** owns process orchestration: obligations, gates, decisions, timers, fork/join and terminal outcomes.
- **workflow_app fact projection** translates canonical application truth into the compact fact vocabulary the engine consumes.
- **Canonical commands/domain services** own legal application mutations and invariants.
- **Human tasks** are first-class because many external domains are not controlled by CulebraLuxe.
- **Attention/alerting** is an operational projection over canonical truth; it must never become a second transaction-state model.

The central product rule is:

> Turn the P&S into a living set of owners, gates and clocks, then watch the deal until it closes.

The engine does not need to reproduce appraisal, lender, title, CRIM, legal, survey, HOA or insurer systems. It needs to know what the deal is waiting for, who owns the obligation, what canonical time gate matters, what evidence resolves it, and when the brokerage needs to intervene.

---

# CRM-27 — Agreement Execution Evidence + Role-Complete Signature Gate

## Objective

Define one provider-neutral, application-owned meaning of **“this agreement is fully executed”** so a multi-party Purchase & Sale agreement cannot advance the transaction after only one signer completes a signature request.

This story is deliberately ordered before CRM-26 because CRM-26 must consume a trustworthy execution signal.

## Problem

`PR-PNS.xml` declares multiple required signature roles, including Buyer, Seller and Seller's Broker, while the current Forms send path can issue a signature request to a single recipient. A neutral `SIGNATURE_REQUEST_COMPLETED` event therefore proves that **a signature request completed**, not necessarily that **the agreement as a whole is fully executed**.

Provider completion must not be conflated with business completion.

The system must also support agreements executed outside the configured signature provider. Human-confirmed or imported signed evidence is valid business input and must not require BoldSign-specific semantics.

## Architectural Decision

Introduce an **Agreement Execution Predicate** owned by the application/document domain.

The predicate answers one question:

```text
Is this issued agreement version fully executed according to its required signature roles/evidence?
```

The predicate must be provider-neutral.

It may be satisfied by:

1. all required signature roles being evidenced by neutral signature request/signer completion state; or
2. an authorized human recording externally executed evidence tied to the immutable issued agreement version / signed artifact lineage.

BoldSign is an evidence source, never the definition of execution.

## Required Semantics

### Required role set

The required signing roles come from the form/template definition or an explicit application-level execution policy derived from that definition. Do not hardcode `BUYER`, `SELLER`, etc. into provider adapters.

The P&S template may contain multiple participants for a role. The role-completion model must tolerate role collections rather than buyer1/buyer2 style columns.

### Evidence

Execution evidence should be traceable to:

- issued transaction document/version,
- signer/role where applicable,
- neutral signature request/provider artifact where applicable,
- evidence source,
- actor/time,
- immutable signed document lineage.

### Business event

Once the predicate transitions from false to true, the application may emit or expose a neutral business fact/event equivalent to:

`AGREEMENT_FULLY_EXECUTED`

The exact persistence mechanism should reuse existing command/event conventions and should not introduce event sourcing.

### Idempotency

Replaying provider callbacks, poll results, reconciliation passes, or manual confirmation must not cause duplicate Deal mutations or duplicate workflow advancement.

The execution predicate must be deterministic for a given evidence set.

## Invariants

1. `SIGNATURE_REQUEST_COMPLETED` != `AGREEMENT_FULLY_EXECUTED` unless the completed request satisfies the final missing required evidence.
2. Provider-specific status strings never leak into workflow XML.
3. A manual/external execution path is allowed and auditable.
4. Agreement execution is tied to a specific immutable issued agreement version.
5. A later superseding agreement/amendment does not mutate the historical execution state of the prior version.
6. Duplicate completion evidence is idempotent.
7. The workflow engine does not decide whether an agreement is legally executed; it consumes the application-owned result.

## Acceptance Criteria

- A single signer completing a multi-role P&S does not mark the agreement fully executed.
- All required roles completed causes the agreement execution predicate to become true exactly once.
- An authorized external/manual execution path can satisfy the predicate without provider-specific logic.
- The execution state is auditable back to immutable document/signature evidence.
- Replayed provider events do not duplicate execution effects.
- Tests cover partial-signature, full-signature, replay and manual-execution paths.

## Dependencies / Reuse

- Forms template/signature role model
- immutable transaction_document lineage
- neutral signature contracts/application seam
- signed document reconciliation
- command idempotency/receipt semantics

## Non-Goals

- Do not redesign BoldSign.
- Do not build a provider-specific agreement state model.
- Do not interpret legal prose.
- Do not require every future agreement to use an external signature provider.

---

# CRM-26 — Executed P&S -> Canonical Deal Projection + Workflow Advance

## Objective

When a Purchase & Sale agreement becomes fully executed, promote only its operational structured terms into canonical Deal state and advance the existing `pns_executed` workflow task so the existing XML naturally executes `deal.set_stage_under_contract` and begins the transaction tracks.

## Current Architecture to Preserve

The workflow already starts from an accepted offer. The XML already models:

```text
offer_accepted
 -> pns_preparation
 -> pns_executed
 -> mark_under_contract
 -> under_contract
 -> fork_tracks
```

Do **not** move workflow start to P&S signing.

The missing seam is the application synchronization between a fully executed agreement and canonical Deal facts.

## Structured Term Projection

The initial projection should be explicit and bounded.

Candidate mappings already supported by canonical command/domain seams:

| P&S structured field | Canonical application meaning | Expected canonical seam |
|---|---|---|
| `closingDate` | target contractual closing date | `deal.set_closing_date` |
| `inspectionDeadline` | contractual inspection-period deadline | `deal.set_inspection_deadline` |
| `financingDeadline` | contractual financing commitment deadline | `deal.set_financing_deadline` |
| `financing` | cash vs financed applicability | `deal.set_financing_type` |
| `appraisalWaived` | appraisal applicability | `deal.set_appraisal_required` (inverse mapping) |

`surveyDeadline` remains an architecture decision and must not be promoted merely because the form contains the field.

`purchasePrice` requires an explicit source-of-truth invariant before automatic mutation; see Open Decisions below.

## Projection Rule

> Promote a P&S field into canonical Deal state only when the application needs to reason about that field after signing.

Legal prose, furniture/art terms, boilerplate, arbitrary negotiated clauses and other non-recurring agreement language remain in the immutable executed document.

A unique negotiated clause that requires operational follow-up should become a human task/obligation, not a permanent Deal column.

## Execution Flow

```text
Agreement execution predicate becomes true
    -> load immutable issued agreement version + structured snapshot
    -> validate Deal linkage
    -> derive bounded operational projection
    -> execute canonical commands/services idempotently
    -> complete/advance correlated workflow `pns_executed` task
    -> XML command-node marks Deal under_contract
    -> parallel tracks begin
```

## Ordering / Atomicity

The canonical application DB and engine runtime may not share a single transaction boundary for all steps. Therefore the handoff must be recoverable and idempotent.

A safe pattern is:

1. project canonical Deal facts using command idempotency;
2. record synchronization completion/evidence in canonical state or a durable receipt;
3. complete the workflow task through the existing task-correlation seam;
4. reconciliation may retry either side safely after interruption.

Do not attempt distributed transaction semantics.

## Open Business Invariant — Purchase Price

The executed P&S may be the legally operative agreement while the accepted Offer is the earlier negotiation artifact.

Before implementation, choose one of the following and encode it deliberately:

1. **P&S authoritative:** executed P&S price updates canonical Deal economics;
2. **Mismatch requires reconciliation:** P&S price must match canonical/accepted Offer unless an authorized user resolves the difference;
3. **Offer authoritative precondition:** Offer must be amended before P&S issuance, so mismatch is rejected.

No implementation agent may silently choose among these.

## Invariants

1. Workflow starts from accepted Offer, not P&S execution.
2. P&S execution enriches Deal state; it does not create a second Deal aggregate.
3. Only structured operational facts are promoted.
4. All writes use canonical command/domain services where they already exist.
5. No workflow-engine direct writes to Deal/application tables.
6. Replaying execution synchronization is idempotent.
7. Workflow advancement occurs only after the agreement execution predicate is true.
8. Legal prose is not decomposed into relational columns merely because it exists in XML.

## Acceptance Criteria

- A fully executed P&S linked to a Deal projects the approved operational terms into canonical Deal state.
- The existing `pns_executed` workflow task advances without restarting the workflow.
- The existing XML `mark_under_contract` command executes normally.
- The same execution signal replayed twice does not duplicate mutations or workflow advancement.
- Partial signature completion cannot trigger the projection.
- Cash/financed and appraisal applicability facts are refreshed correctly after projection.
- Tests prove accepted offer -> P&S execution -> under_contract -> transaction fork.

## Dependencies

- CRM-27
- canonical Deal command layer
- workflow task correlation/completion seam
- RE_supermodel-v1 XML
- Forms/transaction_document immutable issuance path

## Non-Goals

- No new workflow engine.
- No P&S legal interpretation engine.
- No CRIM/lender/title integration.
- No duplicate Deal-stage state machine.

---

# CRM-28 — P&S Amendment -> Canonical Term Delta + Timer Reschedule

## Objective

Apply a fully executed P&S Amendment as an explicit delta against the existing agreement/Deal, route changed operational terms through canonical commands, and allow the existing workflow instance and timer jobs to continue/reschedule without restart.

## Architectural Context

`PR-PNS-AMD.xml` already exists and captures amendment metadata plus structured candidate fields such as amended closing date and price.

The workflow timer subsystem already supports canonical date rescheduling without replacing the process instance.

Therefore this story is primarily an agreement-delta projection problem.

## Amendment Model

An amendment must reference the agreement/deal lineage it modifies.

The amendment artifact is immutable. It does not overwrite the original P&S PDF or historical structured snapshot.

The canonical application state reflects the **current operative value** after amendment while preserving the document lineage that explains why it changed.

## Projection Rules

Only explicit structured amendments should mutate canonical fields automatically.

Examples:

- amended closing date -> `deal.set_closing_date`
- amended inspection deadline -> `deal.set_inspection_deadline` if/when represented structurally
- amended financing deadline -> `deal.set_financing_deadline` if/when represented structurally
- amended financing/appraisal applicability -> corresponding canonical command where explicitly represented

Free-form amendment prose must not be parsed into hidden business mutations without an explicit structured field or authorized human resolution.

## Timer Semantics

Changing a contractual date must:

- preserve the same workflow instance;
- preserve the same logical deadline monitor;
- reschedule/cancel/re-arm the existing generic timer job using existing deadline semantics;
- avoid duplicate timer jobs;
- retain auditability of old and new canonical values.

## Invariants

1. Amendment never overwrites original agreement bytes.
2. Amendment references prior agreement lineage.
3. Structured deltas are explicit; free prose is not silently converted into Deal mutations.
4. Canonical commands own application writes.
5. Date amendment does not restart the workflow.
6. Existing timer/job seam performs reschedule; no second SLA scheduler is created.
7. Replaying the same executed amendment is idempotent.

## Acceptance Criteria

- Executed amendment can update at least closing date through the canonical command path.
- Existing workflow instance remains the same.
- Existing closing timer is rescheduled rather than duplicated.
- Replayed amendment synchronization is a no-op/replay-safe.
- Original P&S and Amendment remain independently retrievable immutable artifacts.
- Tests cover date extension, unchanged fields and duplicate replay.

## Dependencies

- CRM-27 execution predicate semantics
- CRM-26 projection pattern
- PR-PNS-AMD template
- canonical date commands
- generic deadline-timer reschedule seam

## Non-Goals

- No general contract-diff engine.
- No NLP parsing of arbitrary amendment prose.
- No workflow restart on changed dates.

---

# OPS-11A — Deal Attention Projection / Red Lights

## Objective

Produce one deterministic business-facing projection that answers:

> What can hurt a deal today?

This is the operational “red lights” surface. It composes canonical application/workflow truth into attention items without inventing shadow transaction state.

## Existing Inputs to Reuse

The current workflow read model already exposes much of the required data:

- active/current workflow nodes,
- active/completed milestones,
- blocker count,
- open workflow/canonical task count,
- pending timer count,
- responsible-party hints,
- next expected action,
- XML-driven node labels/descriptions/display order,
- process events.

Canonical application state also provides:

- task due dates/status,
- closing/inspection/financing deadlines,
- signature/document state,
- lender clearance,
- closing document readiness,
- intake/identity failures where included by OPS-11 parent scope.

## Projection Model

An `AttentionItem` should be a read-model concept, not necessarily a persisted business aggregate.

Suggested shape:

```text
id / deterministic key
subject type + subject id
deal id
category
severity
headline
detail
business gate / source state
owner/responsible party
assigned app user where known
due date / contractual date where known
age / overdue duration
acknowledgement state if OPS-11B exists
source references
drill-down target
```

Exact TypeScript names may differ; semantics must remain deterministic and provider-neutral.

## Initial Deterministic Rules

Examples include:

- canonical deadline overdue while related obligation remains unresolved;
- canonical deadline due soon and related obligation unresolved;
- workflow blocker node active;
- required closing document packet incomplete;
- financed Deal lacks lender clear-to-close near closing;
- signature request failed/declined or required agreement execution evidence incomplete;
- workflow human task overdue;
- external integration action failed where the failure blocks business progress.

## Deadline Rule

Do not create T-7/T-3/T-1 workflow timers merely to power the dashboard.

Instead:

```text
canonical deadline + unresolved obligation + current clock
    -> healthy / due-soon / urgent / overdue
```

Warning thresholds are presentation/attention policy, not additional contractual dates.

For title, CRIM, funds and closing documents, where there may be no independent contract deadline, attention may be derived from closing-date proximity, explicit canonical task due date, active blocker state or age.

## Severity

Severity should be deterministic in V1, for example:

- informational
- warning
- urgent/critical

Do not use AI prioritization as the source of severity.

## Invariants

1. Attention items are derived from canonical truth.
2. Attention state must not become a duplicate Deal/workflow state model.
3. No fake contractual dates are invented.
4. Read projection can be recomputed safely.
5. Provider-specific errors are normalized before reaching the projection where possible.
6. The projection distinguishes business blockers from engine/runtime anomalies.
7. Forge/Pippin engineering-health concerns stay outside this business projection.

## Acceptance Criteria

- One query/service returns deterministic actionable attention items across active deals.
- Overdue inspection/financing/closing examples classify correctly.
- Active blocker nodes appear with clear business language and drill-down.
- Closing-document/lender/signature blockers can surface from canonical facts.
- No shadow status column is introduced solely for dashboard convenience.
- Unit tests cover rule ordering/severity and false-positive suppression.

## Dependencies

- existing workflow read-service/query projections
- CRM-21 closing document readiness
- CRM-22 canonical deadline facts
- canonical task model
- signature/document status seams

## Non-Goals

- No AI ranking in V1.
- No generic observability dashboard.
- No workflow-engine repair UI.
- No duplicate timer subsystem.

---

# OPS-11B — Attention ACK / NACK / RESOLVED + Escalation Watchdog

## Objective

Add a lightweight operational acknowledgement protocol around attention items so humans can signal ownership/awareness without falsely satisfying the underlying business gate, while a watchdog continues to escalate unresolved risk.

## Core Distinction

Two orthogonal dimensions must exist:

### Business Gate State

Examples:

- pending
- satisfied
- failed
- waived
- unresolved

### Attention State

Examples:

- unacknowledged
- acknowledged
- escalated
- resolved

Critical invariant:

> ACK does not satisfy the business gate.

If Lisa acknowledges “I am chasing the appraiser,” the appraisal remains unresolved until actual evidence/business state changes.

## Operational Protocol

V1 user actions:

### ACK

Meaning: “I have seen this / I own this / it is being handled.”

Effect:

- record actor/time;
- suppress immediate duplicate nagging according to policy;
- keep underlying business rule active;
- allow later escalation if risk grows or remains unresolved.

### NACK

Meaning: “This remains blocked / I cannot resolve it / escalation or reassignment is required.”

Effect may increase severity, notify another subscriber or require explicit reassignment.

### RESOLVED

This action must be carefully defined.

Where the system can observe canonical business completion, `RESOLVED` should mean the attention item is closed because its source condition is no longer true.

Where resolution itself is a human-confirmed business fact, the action must route through the relevant canonical command/task completion seam first; it must not merely hide the alert.

## Watchdog

A bounded periodic reconciliation/watchdog recomputes attention state from canonical truth.

It may:

- create/materialize newly actionable attention items or acknowledgement records if persistence is chosen;
- escalate an acknowledged item when thresholds are crossed;
- close/suppress an item when its underlying condition resolves;
- re-open/create a new attention episode if a condition later recurs.

The watchdog is not a second workflow scheduler. Contract timers remain in the workflow engine.

## Persistence Decision

Persistence is justified only for human interaction/history that cannot be recomputed, especially:

- ACK actor/time,
- NACK actor/time/reason,
- escalation delivery/episode metadata,
- optional snooze/next-reminder state if introduced.

The source business condition remains derived.

## Invariants

1. ACK != business completion.
2. Alert disappearance must not mutate the underlying gate.
3. Acknowledgement history is auditable.
4. Re-running watchdog is idempotent.
5. Escalation is based on canonical dates/state plus policy, not fake workflow dates.
6. Human RESOLVED cannot bypass canonical domain rules.
7. Attention persistence, if any, stores interaction state, not shadow Deal truth.

## Acceptance Criteria

- User can ACK an attention item without completing the related workflow obligation.
- ACK records actor/time and the watchdog can later escalate it.
- NACK path is represented and auditable.
- When underlying canonical state resolves, the attention item ceases to be active deterministically.
- Duplicate watchdog runs do not emit duplicate escalation episodes/deliveries.
- Tests prove ACK-vs-business-state separation.

## Dependencies

- OPS-11A deterministic attention projection
- canonical task/command seams
- CRM-22 deadline facts
- future/activated outbound delivery seam where notifications are sent

## Non-Goals

- No new transaction-state machine.
- No replacing workflow human tasks with alerts.
- No AI decision about whether an obligation is legally satisfied.

---

# OPS-11C — Attention Ownership + Subscriber Delivery

## Objective

Resolve business-attention ownership/subscribers from existing Deal/workflow responsibility metadata and deliver notifications through the existing provider-neutral outbound action/correlation architecture rather than creating a second messaging subsystem.

## Existing Ownership Infrastructure

The application already contains two useful concepts:

1. canonical `task.assigned_user_id` for actual application-user ownership;
2. `workflow_app/responsibility.ts` for resolving XML responsibility hints such as brokerage, buyer, seller, lender, inspector, appraiser, notario and title company to operational responsibility classes and Deal participants.

The missing work is binding these concepts into attention ownership and subscriber policy.

## Ownership Rules

Distinguish:

- **Responsible external party** — who owes the work product (e.g. appraiser/lender/title professional);
- **Accountable CulebraLuxe user** — who must watch/chase/communicate internally;
- **Subscribers** — who should receive a given warning/escalation.

These are not the same identity.

An external appraiser should not be modeled as a CulebraLuxe user merely so the dashboard can display ownership.

## Task Assignment

Workflow-derived canonical tasks should populate `assigned_user_id` when a deterministic owning CulebraLuxe user exists.

Responsibility hints should continue to resolve through the application seam; the engine remains identity-agnostic.

## Subscriber Policy

V1 subscriber policy should be deterministic and conservative.

Examples:

- brokerage-owned attention -> owning/assigned CulebraLuxe user;
- external-SME overdue obligation -> accountable CulebraLuxe user, while displaying the external SME as responsible party;
- severe/critical escalation -> optionally include designated business owner/admin subscriber;
- client notification is a separate explicit business action, not an automatic side effect of every red light.

## Delivery Architecture

Reuse CRM-25's outbound external action/correlation pattern.

Desired boundary:

```text
OPS-11 attention policy
    -> recipient/subscriber resolution
    -> outbound notification intent
    -> CRM-25 durable outbound request
    -> email/provider adapter
    -> provider/external reference
    -> observed result/correlation
```

OPS-11C owns **who/why/when**.

CRM-25 owns **durable external delivery, retry and correlation**.

Do not build an `alert_queue` that duplicates CRM-25.

## Email First

Email is a sensible V1 delivery adapter because much of brokerage coordination already occurs through email.

The business model must remain channel-neutral so a later iMessage/WhatsApp/in-app delivery channel does not alter attention semantics.

## Delivery Idempotency

Consequential outbound communications must not be duplicated by retries.

An attention escalation episode should have a stable delivery/idempotency key sufficient for CRM-25/provider semantics to prevent duplicate sends where supported.

## Invariants

1. Engine responsibility hints remain abstract metadata.
2. External SME identity != internal assignee.
3. OPS-11C does not create a second outbound queue.
4. Email provider details do not leak into business-attention rules.
5. Retry/reconciliation does not send duplicate consequential alerts.
6. Client-facing notifications remain explicit policy decisions.
7. Delivery failure does not corrupt canonical Deal/workflow state.

## Acceptance Criteria

- Workflow/business attention can resolve an accountable internal user where one exists.
- External responsible party is displayed independently of internal owner.
- Workflow-derived canonical task assignment reuses `assigned_user_id` where appropriate.
- One alert can be converted into a provider-neutral outbound notification intent.
- Durable send/retry/correlation reuses CRM-25 rather than a new queue.
- Email delivery proof preserves correlation to attention item/deal.
- Tests cover subscriber resolution and duplicate-send suppression/idempotency boundary.

## Dependencies

- OPS-11A
- OPS-11B
- CRM-13 Deal Participants
- workflow responsibility mapping
- canonical task model
- CRM-25 outbound external action/correlation loop

## Non-Goals

- No provider-specific business policy.
- No automatic client blast for every warning.
- No replacement of CRM-25.

---

# Cross-Story Architecture Decisions Still Requiring Human Review

## 1. Survey Deadline

`PR-PNS.xml` contains `surveyDeadline`, while the existing workflow deadline architecture intentionally does not invent a canonical survey deadline.

Before implementation, determine whether survey timing is a recurring contractual clock in actual CulebraLuxe transactions.

If yes:

- add one canonical application-owned survey deadline fact;
- project it through workflow_app;
- use the generic timer seam;
- do not create a special survey scheduler.

If no:

- leave it as document/task-level information.

The mere existence of the field in the form is not sufficient reason to expand the canonical model.

## 2. Purchase Price Authority After P&S Execution

Define the invariant for divergence between accepted Offer amount and executed P&S purchase price before CRM-26 writes price automatically.

The implementation must not silently choose a source of truth.

## 3. Agreement Execution Role Policy

Confirm whether Seller's Broker signature is legally/business-required for `PR-PNS` execution or merely included for brokerage acknowledgement. The template currently declares it as a signature group; CRM-27 should distinguish required execution roles from optional/acknowledgement roles if needed.

## 4. Attention Threshold Policy

Choose initial deterministic warning thresholds for contractual dates and closing-date proximity. These are business-attention policy, not new workflow deadlines.

---

# Recommended Delivery Order

1. **CRM-27** — establish trustworthy agreement execution semantics.
2. **CRM-26** — project executed P&S operational facts and advance workflow.
3. Prove end-to-end: accepted Offer -> P&S -> under contract -> parallel gates.
4. **CRM-28** — amendment delta + timer reschedule.
5. **OPS-11A** — business red-lights projection.
6. **OPS-11B** — ACK/NACK/RESOLVED + watchdog.
7. Activate/complete **CRM-25** if durable notification transport is not already ready.
8. **OPS-11C** — owner/subscriber resolution + email delivery.

---

# Forge Boundary

Likely Forge-safe after architecture decisions are fixed:

- CRM-26 mapping/wiring
- CRM-28 amendment/date wiring
- OPS-11A deterministic projection
- OPS-11C task-assignment/owner-resolution plumbing

Keep human/architecture review on:

- CRM-27 execution predicate and required-role semantics
- P&S purchase-price authority
- survey deadline decision
- OPS-11B acknowledgement/escalation semantics
- CRM-25 consequential-message idempotency

The guiding implementation rule is:

> Do not invent another model where an existing canonical seam already expresses the business concept.
