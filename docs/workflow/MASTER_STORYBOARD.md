# CulebraLuxe Master Storyboard

> **Durable, authoritative backlog.** Every unit of work is a numbered story with
> a goal, a scope, acceptance criteria, and dependencies. Execution state is
> tracked separately in [`STORYBOARD_STATUS.md`](STORYBOARD_STATUS.md) — this
> file never records status.
>
> Seed version 1, dated 2026-08-20, main @ `fddcd26`. Documentation only: this
> storyboard implements nothing, changes no application/engine/schema/data, and
> builds no UI.

> **Note (2026-08-21):** the persistent `/portal/storyboard` is now the
> authoritative CulebraLuxe master backlog, seeded from the human-authored
> 8/21 master board (74 stories) via
> `db/migrations/022_storyboard_authoritative_seed.sql`. This document and
> `STORYBOARD_STATUS.md` are retained as history of the earlier S-* storyboard.
>
> **Capability (2026-08-21, migration 024):** each stored story now carries a
> durable execution specification — goal, dependencies, preconditions,
> architect brief, context references, acceptance criteria, postconditions —
> and every execution run snapshots that specification at start (immutable).
> The global execution rules live in
> [`docs/agent/STORY_EXECUTION_CONTRACT.md`](../agent/STORY_EXECUTION_CONTRACT.md).
> The 74 stories' spec fields are intentionally empty; the architect/review
> model and human populate important stories deliberately.


---

## 1. Purpose

- One authoritative source for what CulebraLuxe builds next and why.
- Each story is small enough to bound, and each batch is at most **20 stories**.
- Completed work is represented as stories whose acceptance criteria were met;
  the status file records the evidence. Nothing here grants license to re-open
  or re-run completed work without a new story.
- Pending work is intentionally not started by this document.

## 2. Seeding and conventions

Seeded from the existing repository documentation and the current
workflow / TUNIT work:

- `docs/agent/CURRENT.md`, `docs/agent/RUNLOG.md`, `docs/agent/CRM-06-PRELIMINARY.md`
- `docs/workflow-integration-contract.md`
- `docs/workflow-engine-archaeology.md`
- `docs/workflow-xml-model.md`
- `docs/tunit-harvest-register.md`
- `docs/auth-bootstrap-order.md`, `docs/auth-command-map.md`,
  `docs/auth-security-model.md`, `docs/auth-test-matrix.md`, `docs/authjs-adapter.md`
- `docs/portal-ui-contract.md`, `docs/property-ui-contract.md`
- `workflow_engine/ARCHITECTURE_BOUNDARY.md`, `workflow_app/README.md`
- Git history on `main` through `fddcd26` (2026-08-20)
- `db/migrations/001..020` and `db/manual/2026-08-20_*.sql`

**Story IDs:** `S-001` … `S-041`, assigned in batch order. Earlier planning
documents used their own labels (`CRM-NN`, `M-N`, `AUTH-NN`, `WF-NN`, and
"Story 116…136"); the mapping to S-IDs is noted inside each story and in the
[legacy reference map](#legacy-reference-map).

**Seeding rule (honesty constraint):** a story is only recorded as complete
where the repository documentation and git history establish it. Facts that are
operational and unverifiable from the repository alone (for example, whether a
recorded migration was executed against Neon) are explicitly flagged as such in
`STORYBOARD_STATUS.md`; they are never asserted as done.

## 3. Non-negotiable architectural boundaries

Every story, present or future, must preserve:

1. **`workflow_engine` is a generic, domain-neutral runtime.** It must never
   import from `app/`, `components/`, `db/`, `lib/workflow/`, or `workflow_app/`
   (`workflow_engine/ARCHITECTURE_BOUNDARY.md`).
2. **`workflow_app` is the only place CulebraLuxe domain concepts meet the
   engine** (`workflow_app/README.md`).
3. **Portal/UI may observe and control workflows only through `workflow_app`.**
4. **`lib/workflow` is the "Ogden" application-side contract seam** (command
   envelope/result, domain events, command inventory, adapter, fact projection,
   operational contracts). The application never imports the engine
   (`docs/workflow-integration-contract.md`).
5. **Application owns canonical truth** (`person`, `property`, `deal`, `offer`,
   `showing`, `interaction`, `task`, `app_user`), domain commands/validation,
   the authority boundary, the operational task, and business dates/facts.
   **Definition/model owns brokerage policy.**
6. **Authority answers "may this actor attempt this command class?"; domain
   preconditions answer "is this transition legal in the current business
   state?".** No workflow rule may bypass either.
7. **Schema changes only via reviewed migrations in `db/migrations`.** Any
   manual live change must be recorded as an equivalent migration.
8. **`media` is the reusable asset abstraction**; `property_media` owns
   property-specific media roles and ordering. Provider URLs are never media.
9. **The XML node id IS the workflow state identity** — no second workflow-state
   enum/mapping layer. `deal.stage` stays a separate coarse canonical state
   changed only by explicit application commands (`docs/workflow-xml-model.md`).
10. **No listing-specific hardcoding**; `property.id` is the stable identity.
11. **`main` is production-sensitive**: no commit or push without explicit
    authorization; `docs/agent/BUILDER.md` / `REVIEWER.md` govern story execution.

## 4. Batch index

| Batch | Stories | Theme | State (see status file) |
|---|---|---|---|
| 1 | S-001 – S-009 | CRM Intake & Channel Foundations | Foundations complete; decisions/connector pending |
| 2 | S-010 – S-016 | Data, Auth & Portal Foundations | Migrations recorded; bootstrap pending |
| 3 | S-017 – S-028 | Workflow Engine Trust & TUNIT | Engine trust foundation complete |
| 4 | S-029 – S-041 | Workflow Road Ahead | All pending |

### Legacy reference map

| Legacy label | S-ID | Legacy label | S-ID |
|---|---|---|---|
| CRM-01 | S-001 | M-4 (showing) | S-013 |
| CRM-02 | S-002 | M-5 (offer) | S-014 |
| CRM-03 | S-003 | AUTH-01 | S-015 |
| CRM-04 | S-004 | AUTH-02 | S-016 |
| CRM-05 | S-005 | WF-01…WF-12 | S-017…S-028 |
| CRM-06 | S-006 | Story 116,117,119–129,135,136 | S-020 (and S-031/S-041) |
| CRM-07 | S-007 | M-1 (whatsapp channel) | S-010 |
| M-2 (general enquiry) | S-011 | M-3 (deal participant) | S-012 |

---

## Batch 1 — CRM Intake & Channel Foundations (S-001 – S-009)

### S-001 — CRM-01 Source-Idempotent Interaction Inputs

- **Goal:** Establish the canonical, source-idempotent interaction input
  foundation shared by every intake channel.
- **Scope:** Interaction/task foundation (`db/migrations/005`), source-idempotent
  inputs keyed by `(source_system, source_external_id)`, and the database
  uniqueness backstop. No provider connectors, no UI.
- **Acceptance criteria:**
  - Interaction inputs deduplicate by `(source_system, source_external_id)`.
  - The database enforces the uniqueness backstop.
  - The foundation is reusable by all later intake channels without per-channel
    special cases.
- **Dependencies:** None (first CRM foundation story).

### S-002 — CRM-02 Neutral Inbound Events, Identity Normalization & Context Resolution

- **Goal:** Provide provider-neutral inbound events with strict identity
  normalization and exact context resolution.
- **Scope:** Neutral event contract, strict E.164 phone and canonical email
  identity normalization, exact trusted property/deal/person context
  resolution, advisory intents only. No action inference, no persistence beyond
  the CRM-01/02 contracts.
- **Acceptance criteria:**
  - Provider-neutral event shape with bounded, case-preserving source metadata.
  - Strict E.164 / canonical identity normalization; national formats without
    an explicit country code are rejected.
  - Context resolves only from exact trusted adapter context; free text never
    selects person/property/deal context.
  - Advisory intents remain empty of requested actions.
- **Dependencies:** S-001.

### S-003 — CRM-03 Explicitly Authorized Person Creation

- **Goal:** Safe, explicitly authorized atomic person/identity creation with
  race recovery.
- **Scope:** Atomic person + identity creation, existing-person-wins race
  recovery, source-token validation. Fixture-only verification with zero Neon
  access.
- **Acceptance criteria:**
  - Unknown-person creation occurs only under explicit authorization.
  - Concurrent identical creations resolve to a single canonical person
    (existing-person-wins).
  - Repository boundaries and result semantics match CRM-02.
- **Dependencies:** S-001, S-002.

### S-004 — CRM-04 Website / Self-Service Intake

- **Goal:** Provider-neutral website intake through the canonical pipeline.
- **Scope:** Pure website adapter, CRM-02/03 coordinator, atomic canonical
  persistence seam, property-context server action, existing contact-path
  integration, `db/migrations/006_website_intake_submission.sql` recorded.
  Fixture-only verification; no Neon access during verification.
- **Acceptance criteria:**
  - A `processing_started_at` ownership claim gates every receipt transition;
    completion requires ownership.
  - Interaction/task presentation fields are deterministic.
  - Pre-receipt validation is separate from post-claim rejection.
  - Migration 006 is recorded; the runtime path depends on human migration
    execution.
- **Dependencies:** S-002, S-003.

### S-005 — CRM-05 Provider-Neutral Email Intake

- **Goal:** Provider-neutral email intake POC that translates email transport
  events into the CRM-02/03 intake contracts.
- **Scope:** Envelope parsing separated from identity assurance;
  `authenticated_pass` verdict required before unknown-person creation;
  bounded opaque message/thread/reply/reference IDs; strict attachment runtime
  validation and exact MIME matching. Fixture-only; zero provider/Neon access.
- **Acceptance criteria:**
  - Exact canonical email may resolve an existing person; unknown creation
    requires an explicit `authenticated_pass` verdict plus the unanimous
    mailbox-role policy (conservative default: no creation).
  - Provider identifiers are bounded, case-preserving, and opaque.
  - Attachment descriptors are bounded and validated (exact MIME, capped size,
    no URL-like identifiers).
  - Fixture suite passes with zero provider/Neon access.
- **Dependencies:** S-002, S-003.

### S-006 — CRM-06 Phone / SMS / iMessage Communications Intake

- **Goal:** Provider-neutral communications intake POC for call, SMS, and
  iMessage transport.
- **Scope:** Endpoint classification (owned/shared/system), strict E.164 phone
  identity, delivery-authenticity vs. endpoint-ownership assurance separation,
  duplicate-first source identity, exact context resolution, deterministic
  special-endpoint outcomes, duplicate owned-line rejection, bounded
  case-preserving identifiers and normalized message content. Fixture-only.
- **Acceptance criteria:**
  - One normalized phone number belongs to at most one canonical person.
  - Unknown-person creation requires an explicitly approved anti-spoof
    assurance plus line-role policy.
  - Every special endpoint category has a deterministic outcome; identifier and
    content grammars are complete.
  - No canonical persistence occurs until consent/retention policy approval.
- **Dependencies:** S-002, S-003.

### S-007 — CRM-07 WhatsApp Intake Architecture (Architecture Only)

- **Goal:** Design the smallest provider-neutral boundary through which a future
  WhatsApp Cloud API (or equivalent) connector can translate terminal message
  deliveries into canonical CRM intake.
- **Scope:** Architecture only — bounded `WhatsAppProviderEvent` contract,
  canonical `phone` resolution (WhatsApp is a transport channel, not an
  identity type), configuration-owned business-number classification,
  assurance separation, exact context, attachment-reference privacy. No Meta
  SDK/types, webhook route, signature verifier, credential, live request,
  schema write, media fetch, or UI.
- **Acceptance criteria:**
  - Architecture independently reviewed PASS (see `docs/agent/RUNLOG.md`).
  - Provider SDK objects, signature headers, access tokens, and phone-number
    IDs never cross the neutral boundary.
  - No schema change is authorized by this story; canonical WhatsApp transport
    representation is the tracked decision in S-008.
- **Dependencies:** S-002, S-003; implementation depends on S-008.
  Status note: implementation is blocked pending S-008 (see status file).

### S-008 — Canonical WhatsApp Interaction Channel Decision

- **Goal:** Approve or reject adding `whatsapp` to the neutral interaction
  channel contract and the existing interaction-channel constraint.
- **Scope:** Decision only — either a single narrow, separately reviewed
  migration adding `whatsapp` to the channel sets, or an approved alternative
  canonical transport model. No provider implementation.
- **Acceptance criteria:**
  - A decision is recorded with rationale.
  - If approved: a narrow migration adds `whatsapp` without corrupting the
    meaning of `sms`/`imessage`/`call`; no generic `message` channel is
    introduced.
  - The decision explicitly unblocks (or permanently blocks) S-009.
- **Dependencies:** S-007 (architecture). **Blocks** S-009.

### S-009 — WhatsApp Provider Connector Implementation

- **Goal:** Implement the approved WhatsApp intake connector through the
  neutral contract.
- **Scope:** Connector translating provider deliveries into
  `WhatsAppProviderEvent` then canonical CRM intake; durable webhook
  receipt/acknowledgement and retry boundary; signature verification;
  least-privilege credentials; explicit retention/consent policy before live
  traffic. **Repository reality note:** `lib/crm-whatsapp-*.ts`
  (provider-neutral types/normalization/intake scaffolding) was committed under
  `b9cc403` ("Add CRM participant, showing, offer, and WhatsApp read models");
  no connector, webhook, or runtime use of it exists, and docs describe
  implementation as blocked pending S-008.
- **Acceptance criteria:**
  - Events reach canonical intake only through the neutral contract.
  - Webhook ingestion is idempotent with a durable receipt and retry boundary;
    no raw provider envelope is persisted in CRM metadata.
  - Signature verification is a hard boundary; no secret or credential leaks.
  - Consent/opt-in, message-window/template, retention, deletion/export, and
    jurisdictional policies are defined before live traffic.
  - Shared business numbers, groups, replies, reactions, edits, delivery/read
    state, and outbound sending remain separate stories.
- **Dependencies:** S-008 (**blocked** until decision), S-007.

## Batch 2 — Data, Auth & Portal Foundations (S-010 – S-016)

### S-010 — V1 DB Unblock M-1: WhatsApp Interaction Channel (migration 010)

- **Goal:** Record the canonical `whatsapp` interaction channel in the schema.
- **Scope:** `db/migrations/010_whatsapp_channel.sql` — `whatsapp` is a canonical
  interaction channel, not a new identity type; WhatsApp actors resolve through
  `person_identity` phone (strict E.164); source idempotency reuses
  `(source_system, source_external_id)`. Provider integration deferred.
- **Acceptance criteria:**
  - Migration 010 recorded and committed (with the manual bundle in
    `db/manual/2026-08-20_v1_database_unblock.sql`).
  - No application write path assumes the channel before the S-008 decision.
- **Dependencies:** S-008 (decision) governs use; recording is independent.

### S-011 — V1 DB Unblock M-2: General Enquiry Website Intake (migration 011)

- **Goal:** Allow property-less website intake requests.
- **Scope:** `db/migrations/011_website_intake_general_enquiry.sql` —
  `website_intake_submission.property_id` becomes nullable with a CHECK that
  property-scoped requests require a property and `general_enquiry` forbids
  one. Generic `/contact` submits through the canonical pipeline;
  property-scoped `private_viewing`/`property_information` behavior unchanged;
  CRM-04 identity rules unchanged.
- **Acceptance criteria:**
  - Migration 011 recorded and committed.
  - CHECK constraints exactly as documented; no behavioral regression to
    property-scoped intake.
- **Dependencies:** S-004.

### S-012 — V1 DB Unblock M-3: Deal Participants (migration 012)

- **Goal:** Add additive, normalized deal participants.
- **Scope:** `db/migrations/012_deal_participant.sql` — role is a checked
  structural category (`client`/`owner`/`seller`/`other`) plus optional
  `role_label` for the SME long tail (application-curated, no migration per
  role). Legacy `deal.client_person_id`, `deal.owner_user_id`,
  `property.seller_person_id` remain source-of-truth for current reads;
  `deal_participant` is backfilled from them and stays additive until a later
  story migrates the FKs.
- **Acceptance criteria:**
  - Migration 012 recorded and committed.
  - Backfill matches legacy columns; FK migration is a later story.
- **Dependencies:** None structural; participates in later participant/role work.

### S-013 — V1 DB Unblock M-4: Showing Lifecycle (migration 013)

- **Goal:** Add the mutable showing lifecycle entity.
- **Scope:** `db/migrations/013_showing.sql` — `showing` statuses
  `requested`/`scheduled`/`completed`/`cancelled`; `interaction` remains the
  immutable timeline. **Documented only** — the showing→interaction write
  behavior belongs to a later bounded story.
- **Acceptance criteria:**
  - Migration 013 recorded and committed.
  - A completed showing must eventually emit exactly one idempotent
    `channel='showing'` interaction (`occurred_at = completed_at ?? scheduled_at`;
    idempotency from `showing.id`) — the write behavior is a separate story.
- **Dependencies:** S-002 (interaction contract).

### S-014 — V1 DB Unblock M-5: Offer Model (migration 014)

- **Goal:** Add the offer model with counter-offer semantics.
- **Scope:** `db/migrations/014_offer.sql` — `offer` rows carry `amount` and
  `status` (`submitted`/`accepted`/`rejected`/`withdrawn`). Original offers have
  `parent_offer_id = null`; counters are new rows with `status='submitted'` and
  `parent_offer_id` pointing at the countered offer. `status='countered'` is not
  used. `deal.offer_price` is untouched; no offer backfill.
- **Acceptance criteria:**
  - Migration 014 recorded and committed.
  - Counter-offer semantics exactly as documented; `deal.offer_price` never
    mutated by this migration.
- **Dependencies:** S-012 (optional participant context).

### S-015 — AUTH-01 Auth & Security Model Foundation

- **Goal:** Foundation for the application security model and auth bootstrap.
- **Scope:** Migrations `015_auth_security_model.sql`,
  `016_auth_identity.sql`, `017_security_audit_event.sql`; docs
  `auth-security-model.md`, `auth-command-map.md`, `authjs-adapter.md`,
  `auth-test-matrix.md`, `auth-bootstrap-order.md`; `app_user → role →
  authority` model with account-type enforcement; break-glass secret primitive
  (`lib/auth/break-glass-secret.ts`, `scripts/generate-break-glass-hash.mjs`,
  `scripts/verify-break-glass-secret.mjs`); portal login routes.
- **Acceptance criteria:**
  - Schema foundation and docs committed; model reviewable end-to-end.
  - Break-glass secret primitive verified by script.
  - Runtime enforcement is NOT active until the documented bootstrap order is
    followed (S-016).
  - Authorization answers "may this actor attempt this command class?"; domain
    legality stays in domain/workflow services.
- **Dependencies:** None. Runtime activation: S-016.

### S-016 — AUTH-02 Auth Runtime Enforcement Activation

- **Goal:** Activate Portal route enforcement and server-command authorization
  only after bootstrap prerequisites are proven.
- **Scope:** Install/configure the provider (Auth.js + Google/OIDC), set
  `AUTH_*` environment values, link the stable provider subject to an active
  internal `app_user`, assign the first owner role, configure break-glass, and
  only then activate route enforcement and server-command authorization.
  Operational/human sequence (`docs/auth-bootstrap-order.md`).
- **Acceptance criteria:**
  - Normal provider login resolves an `ActingUser` with owner authorities
    (auth test matrix D/G).
  - Break-glass login via `/login/recovery` resolves the same `ActingUser`
    model (matrix N).
  - Auth test matrix A-O passes before enforcement activation.
  - Route protection is never active before both normal and recovery login are
    proven.
- **Dependencies:** S-015. **Blocked** on the human bootstrap sequence.

## Batch 3 — Workflow Engine Trust & TUNIT (S-017 – S-028)

### S-017 — WF-01 Workflow Engine Preservation & Architecture Assessment

- **Goal:** Preserve the generic workflow engine and produce a read-only
  capability assessment against brokerage needs.
- **Scope:** `workflow_engine/` preserved as-is;
  `workflow_engine/ARCHITECTURE_BOUNDARY.md`;
  `docs/workflow-engine-archaeology.md` (architecture map, execution trace,
  metamodel, persistence map, transactions, concurrency, human task semantics,
  timer/job semantics, fork/join, error/terminal semantics, event history).
  No engine code changes.
- **Acceptance criteria:**
  - Assessment documents engine strengths and gaps (§7–§12) without changing
    engine code.
  - Engine remains domain-neutral and independent of application code.
  - Identified gaps map to future stories S-033…S-038.
- **Dependencies:** None.

### S-018 — WF-02 Ogden Integration Seam (Application-Side Contracts)

- **Goal:** Define the deliberately small application↔engine integration seam.
- **Scope:** `lib/workflow/` — `contracts.ts`, `command-inventory.ts`,
  `adapter.ts`, `operational-contracts.ts`;
  `docs/workflow-integration-contract.md`; correlation/causation model; task
  boundary; persistence/audit posture. No engine import; no DSL; no runtime.
- **Acceptance criteria:**
  - Application never imports the engine; engine never imports the application.
  - Command envelope/result, domain event contract/catalog, command inventory,
    adapter interface, and fact projection are defined.
  - Correlation: instance → `CommandEnvelope.correlationId`;
    `CommandEnvelope.commandId` → `emittedEvents[].causationId`.
  - Explicit non-goals recorded (no DSL, no visual modeler, no timers worker,
    no alert delivery, no SME portal, no schema collapse, no event sourcing,
    no RPC framework).
- **Dependencies:** None (pre-CRM-14 plumbing).

### S-019 — WF-03 CRM-14 Transaction Workflow Foundation

- **Goal:** Build the CRM-14 transaction workflow foundation: canonical
  application commands wired to claim-first idempotency receipts.
- **Scope:** `db/workflow-command-receipt.ts`,
  `db/migrations/018_workflow_command_receipt.sql`,
  `db/migrations/019_workflow_task_correlation.sql`,
  `db/migrations/020_deal_financing_type.sql`, `db/deal-stage.ts`,
  `db/offer-acceptance.ts`, `db/deal-closing-date.ts`,
  `workflow_app/command-router.ts`, `workflow_app/engine-bridge.ts`,
  `workflow_app/application-port.ts`, `workflow_app/facts.ts`,
  `workflow_app/responsibility.ts`, `workflow_app/financing.ts`.
- **Acceptance criteria:**
  - Claim-first receipt pattern: exactly one winner per `commandId`; losers
    replay the winner's committed outcome.
  - A `pending` receipt is never a terminal `CommandOutcome` (regression test).
  - Compare-and-set deal stage transitions (`offer → under_contract → closed`).
  - Canonical application commands remain authoritative over workflow requests.
- **Dependencies:** S-018.

### S-020 — WF-04 XML-Driven RE Supermodel (CRM-14E)

- **Goal:** Make XML the authoritative source format for workflow definitions
  and define the brokerage `RE_supermodel`.
- **Scope:** `workflow_app/xml/` (`mini-xml.ts`, `xml-parser.ts`,
  `graph-validator.ts`), `workflow_app/definitions/` (`RE_supermodel-v1.xml`,
  `re-supermodel.ts`, `version-policy.ts`),
  `workflow_app/scripts/deploy-process-definition.ts`,
  `db/manual/2026-08-20_v4_crm14_workflow_activation.sql`,
  `docs/workflow-xml-model.md`. Legacy story references: 116 (state identity +
  label), 117 (responsibility/SME), 119 (jurisdiction/config facts), 120 (simple
  cash path), 121 (complexity paths), 122 (P&S/closing-date), 123 (appraisal
  independence), 124/135/136 (closing readiness), 125 (post-closing), 127
  (deployment pipeline), 129 (visual modeler future contract).
- **Acceptance criteria:**
  - Generic XML grammar (no domain-specific tags) parses and validates into
    `ProcessGraph`; the XML node id IS the workflow state identity.
  - Validator rejects structures the engine cannot run plus unambiguous
    authoring errors; cycles are allowed (blocker loops).
  - Jurisdiction is expressed only as facts/capabilities supplied by
    `workflow_app`, never as engine behavior.
  - Simple cash path creates no optional-track tasks (scenario A).
  - Deploy command recorded but **not executed** in this story (no Neon
    deployment; see S-030).
- **Dependencies:** S-018, S-019.

### S-021 — WF-05 Neon Workflow Transaction Adapter

- **Goal:** Adapt Neon transactions to the engine's transactional runtime
  correctly.
- **Scope:** `db/tx.ts`, `workflow_app/engine-client.ts` and the workflow
  transaction adapter surface (commit `ffac351` "Fix Neon workflow transaction
  adapter").
- **Acceptance criteria:**
  - Engine operations run atomically in one transaction (start, signal,
    completeTask, claim/release, completeJob/failJob).
  - Adapter preserves Neon interactive transaction semantics.
- **Dependencies:** S-018, S-019.

### S-022 — WF-06 Workflow Task Completion Seam

- **Goal:** Close the gap between engine task completion and the canonical
  application task.
- **Scope:** `workflow_app/task-completion.ts` (`completeWorkflowTaskCore` +
  injected deps), `workflow_app/task-materialization.ts`,
  `workflow_app/task-reconciliation.ts`,
  `db/migrations/019_workflow_task_correlation.sql`,
  `workflow_app/tests/task-completion.test.ts`,
  `workflow_app/tests/materialization.test.ts`.
- **Acceptance criteria:**
  - Engine/canonical task 1:1 correlation with no duplicates
    (`duplicate_correlations: 0` live check).
  - Task materialization is idempotent (`materializedTasks: 0` on re-reconcile).
  - Dependency-injected core exists so the seam is unit-testable.
- **Dependencies:** S-021.

### S-023 — WF-07 Neon Interactive Transaction Handling

- **Goal:** Correct Neon interactive (WebSocket Pool + lazy thenable)
  transaction handling.
- **Scope:** `lib/neon-interactive.ts`; correction commit `3ac2a1` ("Fix Neon
  interactive transaction handling") following `ffac351`.
- **Acceptance criteria:**
  - Interactive transactions over the WebSocket Pool are atomic.
  - Live DEV runs (CRM-14H/J per TUNIT register) prove the seam.
- **Dependencies:** S-021.

### S-024 — WF-08 Workflow Command Receipts & Idempotent Replay

- **Goal:** Harden command receipt replay so an in-flight/poisoned receipt can
  never re-run a mutation or surface a terminal outcome.
- **Scope:** `db/workflow-command-receipt.ts` (`claimReceipt`,
  `finalizeReceipt`, `readFinalReceipt`, `replayOutcome`); hardening commit
  `1661937` ("Harden workflow command replay"); regression commit `7eb8690`
  (pending receipt → conflict); `workflow_app/tests/command-receipt.test.ts`.
- **Acceptance criteria:**
  - A null or `pending` receipt maps to a retryable `conflict`, never a
    terminal outcome.
  - A completed receipt replays success/terminal outcomes deterministically.
  - A pending receipt must not mutate the deal (regression).
  - A losing INSERT blocks until the winner commits or rolls back.
- **Dependencies:** S-019.

### S-025 — WF-09 Workflow Reset & IT Support Diagnostics

- **Goal:** Provide bounded operational reset and read-only IT support
  diagnostics for workflows.
- **Scope:** `workflow_app/reset.ts`, `workflow_app/diagnostics.ts` (anomaly
  detectors: `failed-process`, `pending-receipt`, `ready-task-uncorrelated`,
  `correlation-dangling-app-task`, `correlation-dangling-workflow-task`,
  `open-job-on-closed-token`, `multiple-active-instances`); commit `cc4c6da`
  ("Add workflow reset and IT support diagnostics").
- **Acceptance criteria:**
  - Read-only anomaly detection over engine/application state (GLOBAL
    INVARIANT sweep).
  - Reset path bounded and explicit; no production impact without
    authorization.
  - Terminal invariant sweep: no active tokens/tasks/jobs, no duplicate/orphan
    correlations, one active instance per subject (live sweep clean).
- **Dependencies:** S-021, S-022, S-024.

### S-026 — WF-10 Workflow End-to-End Trust Validation

- **Goal:** Complete end-to-end trust validation of the workflow application
  seam.
- **Scope:** `workflow_app/tests/acceptance.test.ts`,
  `deal-closing-date.test.ts`, `closing-timer.test.ts`, `re-supermodel.test.ts`
  (scenarios), `materialization.test.ts`, `task-completion.test.ts`;
  `workflow_app/reconcile.ts`; commit `ec3947b` ("Complete workflow end-to-end
  trust validation").
- **Acceptance criteria:**
  - Duplicate command replay does not double-mutate (`replayed: true`, date
    unchanged).
  - Closing-date reschedule reuses the same instance/job without duplicates.
  - Engine/canonical task correlation is clean.
  - Join releases exactly once; blockers gate closing readiness.
- **Dependencies:** S-020…S-025.

### S-027 — WF-11 TUNIT Harvest Register

- **Goal:** Harvest the trust mechanisms proven during CRM-14O (Engine Trust
  Push / first end-to-end transaction) as durable evidence for a future TUNIT
  suite.
- **Scope:** `docs/tunit-harvest-register.md` — 16 proven mechanisms classified
  `UNIT` / `APPLICATION INTEGRATION` / `LIVE DEV` / `GLOBAL INVARIANT`, each
  with a durable artifact/evidence link.
- **Acceptance criteria:**
  - Every mechanism has a classification and an artifact/evidence pointer.
  - UNIT coverage is dominated by the in-memory engine fake (`FakeSql` /
    `makeApp`); no database required.
  - Known remaining gap recorded (join release concurrency under two
    simultaneous branch completions — later addressed by S-028).
- **Dependencies:** S-020…S-026.

### S-028 — WF-12 Join Release Concurrency Regression Test

- **Goal:** Prove the join releases exactly once under two simultaneous branch
  completions.
- **Scope:** `workflow_app/tests/concurrency.test.ts` (commit `fddcd26`), which
  models the PostgreSQL claim-first serialization boundary (`UNIQUE(command_id)`
  blocking a losing concurrent INSERT).
- **Acceptance criteria:**
  - Two required fork branches completing concurrently against the same join
    release exactly once and produce exactly one downstream token.
  - The TUNIT register's known-remaining-gap note is superseded; the note in
    `docs/tunit-harvest-register.md` predates this test and is a doc-update
    candidate (a separate documentation story, not this one).
- **Dependencies:** S-024, S-027.

## Batch 4 — Workflow Road Ahead (S-029 – S-041)

> All stories in this batch are **PENDING** at seed time. None are authorized to
> start by this document; see `STORYBOARD_STATUS.md` for next-story guidance.

### S-029 — TUNIT Formal Suite

- **Goal:** Convert the TUNIT harvest register into a durable, runnable trust
  test suite.
- **Scope:** Formalized `UNIT` / `APPLICATION INTEGRATION` / `GLOBAL INVARIANT`
  tests across `workflow_app/` and `workflow_engine/`, keyed to the 16 harvest
  mechanisms; a documented, reproducible run command. No production writes.
- **Acceptance criteria:**
  - Every register mechanism has a runnable test or an explicit
    LIVE DEV/operational marker.
  - Run command documented and reproducible from a clean checkout.
  - UNIT tier green on in-memory fakes; no DB/Neon required.
  - Preserves the engine/app architectural boundary (no engine test imports
    application code).
- **Dependencies:** S-027, S-028.

### S-030 — RE Supermodel Deployment to Neon

- **Goal:** Deploy the RE_supermodel definition through the generic pipeline.
- **Scope:** Run `workflow_app/scripts/deploy-process-definition.ts` against a
  reviewed environment (`XML → parse → validate → ProcessGraph →
  upsertProcessDefinition`). Operational + review gate.
- **Acceptance criteria:**
  - Pipeline executes cleanly; deployed definition pinned by version and
    immutable in practice.
  - Version policy honored (`workflow_app/definitions/version-policy.ts`).
  - Production deployment requires explicit authorization and is not part of
    this story's default path.
- **Dependencies:** S-020.

### S-031 — Portal Workflows Experience

- **Goal:** Provide the Portal workflows experience on top of `workflow_app`.
- **Scope:** Read-only summaries already exist (`workflow_app/read-service.ts`,
  `app/portal/workflows`). Full experience — task work surfaces, deadlines and
  responsibility views, attention surfaces, and command actions through
  `workflow_app` only. **UI is not built yet; this is backlog only.**
- **Acceptance criteria:**
  - Portal/UI touches the engine only through `workflow_app` (never directly).
  - Node id/label/description/responsibility surface directly from the deployed
    definition graph (legacy Story 116 contract); no translation table.
  - Follows existing Portal geometry/touch targets and the
    `docs/portal-ui-contract.md` surface.
  - A future visual modeler (S-041) emits/reads the same XML grammar without
    engine changes.
- **Dependencies:** S-020, S-022, S-026. Held by the current "no UI yet"
  constraint.

### S-032 — CRM-14 Closing Orchestration

- **Goal:** Complete the closing orchestration story deferred from the V1
  unblock tranche.
- **Scope:** Closing flow through the RE_supermodel closing readiness/schedule
  path; `deal.set_stage_closed` command; post-close recording continuation;
  closing-date reschedule on the same instance/job.
- **Acceptance criteria:**
  - Closing readiness is join-gated; confirmation cannot bypass blockers.
  - `deal.stage` closing is a separate command; the workflow continues recording
    after `deal.stage = closed` (scenario L1).
  - Closing-date reschedule reuses the same instance and job (scenario I).
- **Dependencies:** S-019, S-020, S-024, S-026.

### S-033 — Engine Error / Terminal Semantics (END / ERROR / CONFLICT)

- **Goal:** Add explicit, typed error/terminal semantics to the engine.
- **Scope:** Archaeology §12 gaps — a `process.error` path, typed
  error/cancelled/conflict terminal outcomes, an END-SUCCESS vs END-ERROR vs
  END-CONFLICT distinction, and propagation of failed jobs to a typed process
  outcome. Preserve application authority for business outcomes.
- **Acceptance criteria:**
  - `job.failed` propagates to a typed process outcome.
  - Invalid transitions surface a typed conflict rather than an unrecorded
    thrown `Error`.
  - Behavior outside this story is preserved; engine stays domain-neutral.
- **Dependencies:** S-017 (assessment), S-019.

### S-034 — Engine Job Lease Requeue & Timer Auto-Advance

- **Goal:** Requeue expired job leases and bind definition-level timers to
  process advancement.
- **Scope:** Archaeology §8/§10 gaps — requeue of jobs whose `locked_until`
  expired; definition-level `timer` nodes that on fire complete the job and
  advance the token deterministically.
- **Acceptance criteria:**
  - Expired leases requeue; no job is lost.
  - A fired definition-level timer advances the process deterministically.
  - Exponential backoff and `max_attempts` behavior preserved.
- **Dependencies:** S-017, S-020 (the XML timer grammar already exists).

### S-035 — Engine Join Correlation & Optional Branch Hardening

- **Goal:** Strengthen fork/join correlation and optional-branch semantics.
- **Scope:** Archaeology §11 gaps — optional branches that may or may not spawn,
  cancelled/failed branch bypass, and join correlation beyond raw
  `parent_token_id` for nested/overlapping fork-join pairs.
- **Acceptance criteria:**
  - An optional branch can be skipped without blocking the join.
  - Nested/overlapping fork-join pairs correlate deterministically.
  - Positive required-branch behavior is unchanged.
- **Dependencies:** S-017, S-020, S-028.

### S-036 — Engine Optimistic Concurrency Guard Enforcement

- **Goal:** Enforce the engine's optimistic-concurrency guards.
- **Scope:** Archaeology §7/§8 gaps — `_moveToken` affected-row check, guarded
  `completeTask` instance version bump, a `_completeToken` version guard, and
  suppression of `token.moved` events on stale moves.
- **Acceptance criteria:**
  - A stale token move is rejected and writes no event.
  - Instance version bumps are guarded by optimistic compare-and-set.
  - Existing STRONG concurrency scenarios remain strong (tests added/updated).
- **Dependencies:** S-017, S-021, S-028.

### S-037 — Application Command Inventory Completion

- **Goal:** Complete the application command inventory and the
  authority/domain-legality separation.
- **Scope:** `lib/workflow/command-inventory.ts` — declared idempotency and
  preconditions for every command class (C/D), kept in sync with the actual
  command implementations; authority boundary enforced at the application
  facade.
- **Acceptance criteria:**
  - Every command class declares idempotency and preconditions.
  - No workflow rule bypasses an application precondition or authority check.
  - Inventory drift is caught by a check (or documented manual review).
- **Dependencies:** S-018, S-019, S-024.

### S-038 — Operational Seams: Alerts / Deadlines / SME / Audit

- **Goal:** Realize the operational contract seams with reviewed
  application-side implementations.
- **Scope:** `lib/workflow/operational-contracts.ts` — task, timer, alert, SME,
  and audit seams; deadlines/responsibility resolve to actual participants via
  `workflow_app/responsibility.ts`; the integration-contract non-goals
  (no alert delivery, no SME portal) are lifted only deliberately and with
  review.
- **Acceptance criteria:**
  - Each seam has a concrete application-side implementation or an explicit
    reviewed deferral.
  - Alerts and audit events write only through approved canonical channels.
  - No alert/SME behavior ships without review and policy.
- **Dependencies:** S-018, S-020, S-022.

### S-039 — Domain Event Persistence & Audit Trail

- **Goal:** Decide and implement durable application-side domain event
  persistence.
- **Scope:** Integration-contract §persistence — domain events are currently
  transient on `CommandResult`; the engine keeps its own event log; no
  `application_event` table exists today. Options: introduce a reviewed table,
  or record a decision to rely on immutable domain records + engine events.
- **Acceptance criteria:**
  - Decision recorded with rationale.
  - If implemented: additive migration plus an idempotent write path;
    `interaction(source_system, source_external_id)` is untouched.
  - Audit posture preserved (`security_audit_event` covers auth/break-glass
    only).
- **Dependencies:** S-018, S-024.

### S-040 — Media / Attachment & Retention Policy for Provider Ingestion

- **Goal:** Define and implement bounded attachment/media ingestion and
  retention for provider channels.
- **Scope:** `docs/agent/CURRENT.md` risks — attachment fetch, malware
  screening, media ownership/expiry/retention; bounded descriptors only until
  policy approval; reuse the `media`/`property_media` abstraction (provider URLs
  are never media).
- **Acceptance criteria:**
  - Policy is approved before any byte ingestion.
  - Ingested bytes create `media`/`property_media` only under the approved
    policy; descriptors are bounded and validated.
  - Provider-hosted media retention stays distinct from CulebraLuxe media
    ownership.
- **Dependencies:** S-005/S-006 attachment boundaries, S-009 (WhatsApp).

### S-041 — Workflow Visual Modeler (legacy Story 129 Future Contract)

- **Goal:** Build a visual editor that reads and writes the same XML grammar.
- **Scope:** Future capability; no UI is built now. The editor emits/reads the
  grammar in `docs/workflow-xml-model.md` §2; the parse→validate→deploy→engine
  pipeline is unchanged.
- **Acceptance criteria:**
  - Editor round-trips XML source without loss (ids, labels, descriptions,
    display-order).
  - No engine change is required; the deploy pipeline is reused.
  - XML remains source-controlled, diffable, and reviewable.
- **Dependencies:** S-020, S-031.

---

## 5. How this backlog is maintained

- Add new work as a new numbered story in the next free S-ID slot; never reuse
  or renumber completed stories.
- Keep batches at or below 20 stories; open a new batch when a batch reaches 20.
- Move a story's status only in `STORYBOARD_STATUS.md`, never here.
- Preserve the architectural boundaries in §3 in every story's scope.
- If this backlog diverges from repository reality, fix the storyboard with a
  separate documentation change and record it in the status file's change log.
