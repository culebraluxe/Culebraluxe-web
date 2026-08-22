# CulebraLuxe Storyboard Status

> **Durable execution checkpoint.** Tracks every story in
> [`MASTER_STORYBOARD.md`](MASTER_STORYBOARD.md) by Story ID. This file records
> what has actually happened, never what we hope happened. Status changes are
> made here, never in the master storyboard.
>
> Seed version 1, dated 2026-08-20, main @ `fddcd26`. Documentation only.

## 1. Allowed statuses

| Status | Meaning |
|---|---|
| `PENDING` | Defined in the backlog; not started. |
| `CURRENT` | Actively being worked (exactly one at a time). |
| `PASS` | Completed; acceptance criteria met with recorded evidence. |
| `BLOCKED` | Attempted or ready but unable to proceed; the blocking dependency is named. |
| `SKIPPED` | Deliberately not pursued; reason recorded. |

## 2. Conventions for this checkpoint

- **Never invent completion claims.** A story is `PASS` only where the
  repository documentation and git history establish it.
- **Operational facts** (for example, whether a recorded migration was executed
  against Neon) are flagged as *not verifiable from repository docs* and are
  never asserted as done.
- For `PASS`/`BLOCKED`/attempted stories the record includes: files changed,
  tests/checks run, database mutations, defects found, decisions made, and
  blocking dependency (if any).
- The seed reflects repository reality at `fddcd26`; newer reality updates the
  change log (§8) and this file.

## 3. Next story / next batch

- **Currently `CURRENT`:** none — no story was mid-flight at seed time
  (last completed: S-028 @ `fddcd26`).
- **Next batch:** Batch 4 — Workflow Road Ahead (S-029 … S-041), all `PENDING`.
- **Recommended next story:** **S-029 — TUNIT Formal Suite** — the direct
  continuation of the just-completed trust work (S-026…S-028) and the reason
  the harvest register (S-027) was written.
- **Parallel candidates (each starts a separate work stream):**
  - **S-030 — RE Supermodel Deployment** — operational gate; needs a reviewed
    environment and explicit authorization before any Neon deployment.
  - **S-008 — Canonical WhatsApp Interaction Channel Decision** — a
    documentation/architecture decision that unblocks S-009.

---

## 4. Batch 1 — CRM Intake & Channel Foundations (S-001 – S-009)

| Story ID | Title | Status | Blocking dependency |
|---|---|---|---|
| S-001 | CRM-01 Source-Idempotent Interaction Inputs | PASS | — |
| S-002 | CRM-02 Neutral Inbound Events & Identity Normalization | PASS | — |
| S-003 | CRM-03 Explicitly Authorized Person Creation | PASS | — |
| S-004 | CRM-04 Website / Self-Service Intake | PASS | — |
| S-005 | CRM-05 Provider-Neutral Email Intake | PASS | — |
| S-006 | CRM-06 Phone / SMS / iMessage Communications Intake | PASS | — |
| S-007 | CRM-07 WhatsApp Intake Architecture (Architecture Only) | PASS | S-008 (for implementation) |
| S-008 | Canonical WhatsApp Interaction Channel Decision | PENDING | — |
| S-009 | WhatsApp Provider Connector Implementation | PENDING | S-008 (BLOCKED) |

### S-001 — CRM-01 Source-Idempotent Interaction Inputs

- **Status:** `PASS`
- **Files changed:** commit `ff6b371` ("Add CRM interaction and task
  foundation"); `db/migrations/005_crm_interaction_task_foundation.sql` and the
  interaction/task repositories.
- **Tests/checks run:** CRM-01 fixture suite
  (`scripts/verify-crm-foundation.mjs`), zero Neon access (RUNLOG).
- **Database mutations:** migration 005 recorded; fixture verification executed
  no Neon statements.
- **Defects found:** none recorded in docs.
- **Decisions made:** `(source_system, source_external_id)` is the canonical
  interaction idempotency key with a database uniqueness backstop.
- **Blocking dependency:** none.

### S-002 — CRM-02 Neutral Inbound Events, Identity Normalization & Context Resolution

- **Status:** `PASS`
- **Files changed:** commit `2067c5c` ("Add CRM intake normalization
  foundation"); `lib/crm-intake-types.ts`, `lib/crm-intake-normalization.ts`,
  `lib/crm-intake.ts`, `lib/crm-person-resolution.ts`, `lib/crm-person-types.ts`.
- **Tests/checks run:** CRM-02 fixture suite
  (`scripts/verify-crm-intake.mjs`), zero Neon access.
- **Database mutations:** none from verification.
- **Defects found:** none recorded in docs.
- **Decisions made:** strict E.164 phone and canonical email identities; exact
  trusted context only — free text never selects person/property/deal context;
  advisory intents remain empty of requested actions.
- **Blocking dependency:** none.

### S-003 — CRM-03 Explicitly Authorized Person Creation

- **Status:** `PASS`
- **Files changed:** commit `09ae5f3` ("Add safe CRM person creation");
  `lib/crm-person-creation.ts`, `lib/crm-person-types.ts`.
- **Tests/checks run:** CRM-03 fixture suite
  (`scripts/verify-crm-person-creation.mjs`), zero Neon access.
- **Database mutations:** none from verification.
- **Defects found:** none recorded in docs.
- **Decisions made:** unknown-person creation requires explicit authorization;
  existing-person-wins race recovery; source-token validation.
- **Blocking dependency:** none.

### S-004 — CRM-04 Website / Self-Service Intake

- **Status:** `PASS` (local implementation PASS; RUNLOG 2026-08-18)
- **Files changed:** commit `d5ad6fc` ("Add website CRM intake pipeline");
  `app/actions/website-intake.ts`, `app/contact/page.tsx`,
  `components/contact.tsx`, `components/property/property-actions.tsx`,
  `db/migrations/006_website_intake_submission.sql`, `db/website-intake.ts`,
  `lib/website-intake.ts`, `lib/website-intake-types.ts`,
  `scripts/verify-website-intake.mjs`.
- **Tests/checks run:** CRM-04 fixture suite
  (`scripts/verify-website-intake.mjs`) passed with zero Neon access;
  CRM-01/02/03 suites passed; `git diff --check` passed;
  `pnpm exec next build --webpack` passed.
- **Database mutations:** migration 006 recorded/committed. Neon execution is
  an operational fact not verifiable from repository docs.
- **Defects found:** implementation review initially returned CHANGES REQUIRED
  (stale claim ownership, rejected-action retry state, fixture gaps); fixed and
  re-reviewed PASS. Restricted known-transient reset behavior to an explicit
  error class; receipt transitions require ownership success before reporting
  completion.
- **Decisions made:** narrow unresolved-intake receipt and retry/idempotency
  boundary justified; pre-receipt validation separate from post-claim
  rejection; deterministic interaction/task presentation fields.
- **Blocking dependency:** none.

### S-005 — CRM-05 Provider-Neutral Email Intake

- **Status:** `PASS` (fixture POC; independently reviewed PASS)
- **Files changed:** commit `9edbe52` ("Add email and communications CRM
  intake"); `lib/crm-email-intake.ts`, `lib/crm-email-normalization.ts`,
  `lib/crm-email-types.ts`, `scripts/verify-crm-email-intake.mjs`.
- **Tests/checks run:** CRM-05 fixture suite passed with zero provider/Neon
  access; `git diff --check` passed; build passed.
- **Database mutations:** none from verification; no migration proposed.
- **Defects found:** review required strict attachment runtime validation,
  exact MIME matching, and context fixture additions before PASS.
- **Decisions made:** envelope parsing separated from identity assurance;
  unknown-person creation requires an `authenticated_pass` verdict plus the
  unanimous mailbox-role policy (conservative default: no creation); bounded,
  case-preserving opaque message/thread IDs; live email ingestion still needs
  durable receipts/cursors and provider security/retention review.
- **Blocking dependency:** none for the fixture POC.

### S-006 — CRM-06 Phone / SMS / iMessage Communications Intake

- **Status:** `PASS` (fixture POC; independently reviewed PASS)
- **Files changed:** commit `9edbe52`; `lib/crm-communications-intake.ts`,
  `lib/crm-communications-normalization.ts`, `lib/crm-communications-types.ts`,
  `scripts/verify-crm-communications-intake.mjs`;
  `docs/agent/CRM-06-PRELIMINARY.md`.
- **Tests/checks run:** CRM-06 fixture suite passed with zero provider/Neon
  access; matrix/idempotency/nonidentity fixtures added; `git diff --check`
  passed; build passed.
- **Database mutations:** none from verification; no migration proposed.
- **Defects found:** review required duplicate owned-line rejection and
  deterministic special-endpoint outcomes before PASS.
- **Decisions made:** delivery authenticity separated from endpoint ownership
  assurance; unknown-person creation requires approved anti-spoof assurance
  plus line-role policy; no canonical persistence until consent/retention
  policy approval.
- **Blocking dependency:** none for the fixture POC.

### S-007 — CRM-07 WhatsApp Intake Architecture (Architecture Only)

- **Status:** `PASS` (architecture scope only; independently reviewed PASS,
  RUNLOG 2026-08-18)
- **Files changed:** architecture recorded in `docs/agent/CURRENT.md`
  (CRM-07 section); provider-neutral scaffolding `lib/crm-whatsapp-types.ts`,
  `lib/crm-whatsapp-normalization.ts`, `lib/crm-whatsapp-intake.ts` committed
  under `b9cc403` ("Add CRM participant, showing, offer, and WhatsApp read
  models").
- **Tests/checks run:** independent read-only architecture review; no
  Critical/High/Medium findings.
- **Database mutations:** none (architecture only; no schema change
  authorized).
- **Defects found:** none (architecture gate passed clean).
- **Decisions made:** WhatsApp is a transport channel, not an identity type —
  actors resolve through canonical `phone`; provider SDK objects/signature
  headers/access tokens/phone-number IDs never cross the neutral boundary;
  implementation blocked pending the S-008 channel decision.
- **Blocking dependency:** implementation blocked on **S-008**.

### S-008 — Canonical WhatsApp Interaction Channel Decision

- **Status:** `PENDING`
- **Files changed:** none yet.
- **Tests/checks run:** none.
- **Database mutations:** none.
- **Defects found:** none.
- **Decisions made:** the recommended future direction is the narrow `whatsapp`
  channel addition (per `docs/agent/CURRENT.md`), but the decision itself has
  not been made.
- **Blocking dependency:** unblocks **S-009**; driven by S-007 architecture.

### S-009 — WhatsApp Provider Connector Implementation

- **Status:** `PENDING` (`BLOCKED` on S-008)
- **Files changed:** scaffolding only exists today (`lib/crm-whatsapp-*.ts` via
  `b9cc403`); no connector, webhook, route, or runtime use.
- **Tests/checks run:** none (no implementation).
- **Database mutations:** none.
- **Defects found:** none.
- **Decisions made:** none beyond S-007 architecture.
- **Blocking dependency:** **S-008** (canonical channel decision).

## 5. Batch 2 — Data, Auth & Portal Foundations (S-010 – S-016)

| Story ID | Title | Status | Blocking dependency |
|---|---|---|---|
| S-010 | V1 DB Unblock M-1: WhatsApp Interaction Channel | PASS | — |
| S-011 | V1 DB Unblock M-2: General Enquiry Website Intake | PASS | — |
| S-012 | V1 DB Unblock M-3: Deal Participants | PASS | — |
| S-013 | V1 DB Unblock M-4: Showing Lifecycle | PASS | — |
| S-014 | V1 DB Unblock M-5: Offer Model | PASS | — |
| S-015 | AUTH-01 Auth & Security Model Foundation | PASS | S-016 (for enforcement) |
| S-016 | AUTH-02 Auth Runtime Enforcement Activation | PENDING | S-015 + human bootstrap |

### S-010 — V1 DB Unblock M-1: WhatsApp Interaction Channel (migration 010)

- **Status:** `PASS` (migration recorded and committed)
- **Files changed:** `db/migrations/010_whatsapp_channel.sql`;
  `db/manual/2026-08-20_v1_database_unblock.sql` (commit `61316de`,
  "Add V1 CRM database unblock tranche").
- **Tests/checks run:** migration review per `docs/agent/CURRENT.md` M-1 notes.
- **Database mutations:** migration 010 recorded/committed. Neon execution is
  an operational fact not verifiable from repository docs.
- **Defects found:** none recorded.
- **Decisions made:** `whatsapp` is a canonical interaction channel, not a new
  identity type; source idempotency reuses `(source_system, source_external_id)`.
- **Blocking dependency:** use is gated by the S-008 decision.

### S-011 — V1 DB Unblock M-2: General Enquiry Website Intake (migration 011)

- **Status:** `PASS` (migration recorded and committed)
- **Files changed:** `db/migrations/011_website_intake_general_enquiry.sql`
  (commit `61316de`).
- **Tests/checks run:** migration review per CURRENT.md M-2 notes.
- **Database mutations:** migration 011 recorded/committed. Neon execution is
  an operational fact not verifiable from repository docs.
- **Defects found:** none recorded.
- **Decisions made:** `property_id` nullable with CHECK (property-scoped
  requests require a property; `general_enquiry` forbids one); generic
  `/contact` flows through the canonical pipeline; CRM-04 rules unchanged.
- **Blocking dependency:** none.

### S-012 — V1 DB Unblock M-3: Deal Participants (migration 012)

- **Status:** `PASS` (migration recorded and committed)
- **Files changed:** `db/migrations/012_deal_participant.sql` (commit
  `61316de`).
- **Tests/checks run:** migration review per CURRENT.md M-3 notes.
- **Database mutations:** migration 012 recorded/committed; backfill from
  legacy columns documented. Neon execution is an operational fact not
  verifiable from repository docs.
- **Defects found:** none recorded.
- **Decisions made:** role is a checked structural category
  (`client`/`owner`/`seller`/`other`) + optional `role_label`; legacy FKs
  remain source-of-truth until a later story migrates them.
- **Blocking dependency:** none.

### S-013 — V1 DB Unblock M-4: Showing Lifecycle (migration 013)

- **Status:** `PASS` (migration recorded and committed; documented only)
- **Files changed:** `db/migrations/013_showing.sql` (commit `61316de`).
- **Tests/checks run:** migration review per CURRENT.md M-4 notes.
- **Database mutations:** migration 013 recorded/committed. Neon execution is
  an operational fact not verifiable from repository docs.
- **Defects found:** none recorded.
- **Decisions made:** `showing` is the mutable lifecycle entity; `interaction`
  remains the immutable timeline; the showing→interaction write behavior is a
  later bounded story.
- **Blocking dependency:** write behavior deferred to a later story (not yet
  assigned an S-ID).

### S-014 — V1 DB Unblock M-5: Offer Model (migration 014)

- **Status:** `PASS` (migration recorded and committed)
- **Files changed:** `db/migrations/014_offer.sql` (commit `61316de`).
- **Tests/checks run:** migration review per CURRENT.md M-5 notes.
- **Database mutations:** migration 014 recorded/committed. Neon execution is
  an operational fact not verifiable from repository docs.
- **Defects found:** none recorded.
- **Decisions made:** original offers `parent_offer_id = null`; counters are new
  rows with `status='submitted'`; `status='countered'` is not used;
  `deal.offer_price` untouched; no backfill.
- **Blocking dependency:** none.

### S-015 — AUTH-01 Auth & Security Model Foundation

- **Status:** `PASS` (schema foundation + docs; **runtime enforcement not yet
  active**)
- **Files changed:** `db/migrations/015_auth_security_model.sql`,
  `016_auth_identity.sql`, `017_security_audit_event.sql`; manual bootstrap SQL
  in `db/manual/` (`v2`, `v2a`, `v3`, `v6`); `docs/auth-security-model.md`,
  `auth-command-map.md`, `authjs-adapter.md`, `auth-test-matrix.md`,
  `auth-bootstrap-order.md`; `lib/auth/break-glass-secret.ts`;
  `scripts/generate-break-glass-hash.mjs`, `scripts/verify-break-glass-secret.mjs`;
  Portal login routes (`app/login`, `app/login/recovery`).
- **Tests/checks run:** `node scripts/verify-break-glass-secret.mjs`
  (hash/verify symmetry, wrong-secret rejection, malformed-hash rejection,
  deterministic hashing per salt).
- **Database mutations:** migrations 015–017 recorded/committed;
  `docs/auth-bootstrap-order.md` states migration 016 is already live
  (`identity_count = 0`). Remaining bootstrap mutations are manual/operational
  and not verifiable from repo docs.
- **Defects found:** none recorded in docs.
- **Decisions made:** `app_user → role → authority`; account-type enforcement;
  cross-type role assignment blocked at the DB boundary; `person` is never the
  auth principal; authorization never decides domain legality.
- **Blocking dependency:** runtime enforcement is blocked on the S-016
  bootstrap sequence.

### S-016 — AUTH-02 Auth Runtime Enforcement Activation

- **Status:** `PENDING` (`BLOCKED` on the documented human bootstrap sequence)
- **Files changed:** none yet.
- **Tests/checks run:** none.
- **Database mutations:** none.
- **Defects found:** none.
- **Decisions made:** the bootstrap order is fixed
  (`docs/auth-bootstrap-order.md`): provider login proven → subject linked →
  owner role assigned → break-glass proven → only then enforcement. Route
  protection is never active before both login paths are proven.
- **Blocking dependency:** **S-015** plus human/operational steps (provider
  console, environment values, subject linking).

## 6. Batch 3 — Workflow Engine Trust & TUNIT (S-017 – S-028)

| Story ID | Title | Status | Blocking dependency |
|---|---|---|---|
| S-017 | WF-01 Workflow Engine Preservation & Architecture Assessment | PASS | — |
| S-018 | WF-02 Ogden Integration Seam | PASS | — |
| S-019 | WF-03 CRM-14 Transaction Workflow Foundation | PASS | — |
| S-020 | WF-04 XML-Driven RE Supermodel (CRM-14E) | PASS | — |
| S-021 | WF-05 Neon Workflow Transaction Adapter | PASS | — |
| S-022 | WF-06 Workflow Task Completion Seam | PASS | — |
| S-023 | WF-07 Neon Interactive Transaction Handling | PASS | — |
| S-024 | WF-08 Workflow Command Receipts & Idempotent Replay | PASS | — |
| S-025 | WF-09 Workflow Reset & IT Support Diagnostics | PASS | — |
| S-026 | WF-10 Workflow End-to-End Trust Validation | PASS | — |
| S-027 | WF-11 TUNIT Harvest Register | PASS | — |
| S-028 | WF-12 Join Release Concurrency Regression Test | PASS | — |

### S-017 — WF-01 Workflow Engine Preservation & Architecture Assessment

- **Status:** `PASS` (read-only assessment; no engine changes)
- **Files changed:** `docs/workflow-engine-archaeology.md`,
  `workflow_engine/ARCHITECTURE_BOUNDARY.md` (engine preserved).
- **Tests/checks run:** read-only inspection; no code/SQL/package changes.
- **Database mutations:** none.
- **Defects found:** engine gaps documented for future stories (§7 unchecked
  optimistic guards, §8 weak spots — retry idempotency, stale token execution,
  expired-lease requeue, §10 timer auto-advance, §11 optional/cancelled/failed
  branches and join correlation, §12 missing error/terminal/conflict
  semantics).
- **Decisions made:** engine is preserved and evaluated first; it remains
  generic and domain-neutral; gaps map to S-033…S-038.
- **Blocking dependency:** none.

### S-018 — WF-02 Ogden Integration Seam (Application-Side Contracts)

- **Status:** `PASS`
- **Files changed:** `lib/workflow/contracts.ts`, `command-inventory.ts`,
  `adapter.ts`, `operational-contracts.ts`; `docs/workflow-integration-contract.md`.
- **Tests/checks run:** contract review; no runtime.
- **Database mutations:** none.
- **Defects found:** none; explicit non-goals documented (no DSL, no visual
  modeler, no timers worker, no alert delivery, no SME portal, no schema
  collapse, no event sourcing, no RPC framework).
- **Decisions made:** the application never imports the engine; the engine
  never imports the application; correlation/causation and task boundary
  fixed.
- **Blocking dependency:** none.

### S-019 — WF-03 CRM-14 Transaction Workflow Foundation

- **Status:** `PASS`
- **Files changed:** commit `2b83f53` ("Build CRM-14 transaction workflow
  foundation"); `db/workflow-command-receipt.ts`, `db/deal-stage.ts`,
  `db/offer-acceptance.ts`, `db/deal-closing-date.ts`, `db/deal-financing.ts`,
  `db/tx.ts`; `db/migrations/018_workflow_command_receipt.sql`,
  `019_workflow_task_correlation.sql`, `020_deal_financing_type.sql`;
  `workflow_app/command-router.ts`, `engine-bridge.ts`, `application-port.ts`,
  `facts.ts`, `responsibility.ts`, `financing.ts`, `correlation.ts`,
  `idempotency.ts`, `engine-client.ts`.
- **Tests/checks run:** `workflow_app/tests/acceptance.test.ts`,
  `deal-closing-date.test.ts`, `financing.test.ts`, `uniqueness.test.ts`;
  in-memory fakes, no DB required.
- **Database mutations:** migrations 018–020 recorded/committed; manual bundles
  `db/manual/2026-08-20_v4_crm14_workflow_activation.sql` (activation) and
  `v5_crm14_verify_readonly.sql` (read-only verification) recorded. Neon
  execution is an operational fact not verifiable from repository docs.
- **Defects found:** none recorded at foundation stage; later hardening
  captured in S-023/S-024.
- **Decisions made:** claim-first receipt pattern; compare-and-set deal stage
  transitions; canonical application commands authoritative over workflow
  requests.
- **Blocking dependency:** none.

### S-020 — WF-04 XML-Driven RE Supermodel (CRM-14E)

- **Status:** `PASS`
- **Files changed:** commit `6c8e82e` ("Add XML-driven RE workflow supermodel");
  `workflow_app/xml/mini-xml.ts`, `xml-parser.ts`, `graph-validator.ts`;
  `workflow_app/definitions/RE_supermodel-v1.xml`, `re-supermodel.ts`,
  `version-policy.ts`; `workflow_app/scripts/deploy-process-definition.ts`;
  `db/manual/2026-08-20_v4_crm14_workflow_activation.sql`,
  `v5_crm14_verify_readonly.sql`; `docs/workflow-xml-model.md`.
- **Tests/checks run:** `workflow_app/tests/mini-xml.test.ts` (13),
  `xml-parser.test.ts` (15), `graph-validator.test.ts` (25),
  `re-supermodel.test.ts` (22), `version-policy.test.ts` (4),
  `uniqueness.test.ts` (3).
- **Database mutations:** none deployed by this story — the deploy command is
  recorded but not executed; activation/verify SQL recorded as manual bundles
  (execution not verifiable from repo docs).
- **Defects found:** none recorded; the validator deliberately rejects only
  structures the engine cannot run plus unambiguous authoring errors.
- **Decisions made:** XML is the authoritative source format; XML node id IS
  the workflow state identity (legacy Story 116); jurisdiction as facts only
  (Story 119); `deal.stage` remains separate and command-driven; no new
  dependency — bounded `mini-xml.ts` implemented (XML doc §16);
  `closingReadinessVerified` boolean removed as the wrong semantic shape
  (legacy Stories 135/136).
- **Blocking dependency:** deployment is a separate story (S-030).

### S-021 — WF-05 Neon Workflow Transaction Adapter

- **Status:** `PASS`
- **Files changed:** commit `ffac351` ("Fix Neon workflow transaction
  adapter"); `db/tx.ts`, `workflow_app/engine-client.ts`, workflow transaction
  adapter surface.
- **Tests/checks run:** workflow tests exercising `TxRunner`; live DEV runs.
- **Database mutations:** none from tests (in-memory fakes); live DEV runs per
  TUNIT register.
- **Defects found:** initial transaction-adapter defect corrected by this
  commit; a follow-up interactive-handling defect was fixed by S-023.
- **Decisions made:** one transaction per engine operation; interactive Neon
  semantics preserved.
- **Blocking dependency:** none.

### S-022 — WF-06 Workflow Task Completion Seam

- **Status:** `PASS`
- **Files changed:** commit `df72e80` ("Add workflow task completion seam");
  `workflow_app/task-completion.ts` (`completeWorkflowTaskCore` + deps),
  `task-materialization.ts`, `task-reconciliation.ts`;
  `db/migrations/019_workflow_task_correlation.sql`;
  `workflow_app/tests/task-completion.test.ts`, `materialization.test.ts`.
- **Tests/checks run:** `workflow_app/tests/task-completion.test.ts` (3),
  `materialization.test.ts` (3); live checks `materializedTasks: 0` on
  re-reconcile and `duplicate_correlations: 0` (TUNIT register #7/#8).
- **Database mutations:** none from tests (in-memory fakes); live DEV
  verification per TUNIT register.
- **Defects found:** the seam initially lacked a dependency-injected core;
  `completeWorkflowTaskCore` was extracted so the seam is unit-testable
  (TUNIT register note).
- **Decisions made:** canonical CulebraLuxe `task` stays the user-facing work
  item; `workflow_task_correlation` is the deterministic 1:1 key; no dual truth.
- **Blocking dependency:** none.

### S-023 — WF-07 Neon Interactive Transaction Handling

- **Status:** `PASS`
- **Files changed:** commit `3ac2a1` ("Fix Neon interactive transaction
  handling"); `lib/neon-interactive.ts` (WebSocket Pool + lazy thenable).
- **Tests/checks run:** live DEV runs CRM-14H/J prove atomic interactive
  transactions (TUNIT register #2).
- **Database mutations:** live DEV instance exercised via runtime seams only.
- **Defects found:** interactive transaction handling was incorrect after the
  first adapter fix; corrected here.
- **Decisions made:** interactive (WebSocket Pool) transactions are atomic;
  lazy thenable semantics preserved.
- **Blocking dependency:** none.

### S-024 — WF-08 Workflow Command Receipts & Idempotent Replay

- **Status:** `PASS`
- **Files changed:** commit `1661937` ("Harden workflow command replay"),
  commit `7eb8690` ("test: add assertion that pending receipt outcome is
  conflict"); `db/workflow-command-receipt.ts` (`claimReceipt`,
  `finalizeReceipt`, `readFinalReceipt`, `replayOutcome`);
  `db/migrations/018_workflow_command_receipt.sql`;
  `workflow_app/tests/command-receipt.test.ts`.
- **Tests/checks run:** `workflow_app/tests/command-receipt.test.ts` (6 tests);
  `tsx --test` runner; `replayOutcome` unit coverage plus `setDealStage`
  integration fakes.
- **Database mutations:** none from tests (in-memory fakes); migration 018
  recorded/committed.
- **Defects found:** a `pending` receipt could previously be mistaken for a
  terminal outcome or re-run a mutation; hardened so null/`pending` maps to a
  retryable `conflict`, and a pending receipt never mutates the deal
  (regression test).
- **Decisions made:** claim-first pattern — `UNIQUE(command_id)` is the
  serialization boundary; the losing INSERT blocks until the winner
  commits/rolls back; persisted `pending` is a sentinel, never a terminal
  `CommandOutcome`.
- **Blocking dependency:** none.

### S-025 — WF-09 Workflow Reset & IT Support Diagnostics

- **Status:** `PASS`
- **Files changed:** commit `cc4c6da` ("Add workflow reset and IT support
  diagnostics"); `workflow_app/reset.ts`, `workflow_app/diagnostics.ts`;
  `workflow_app/tests/reset.test.ts`, `reconcile.test.ts`.
- **Tests/checks run:** `reset.test.ts` (2), `reconcile.test.ts` (2); live
  terminal-invariant sweep (all clean) per TUNIT register #15.
- **Database mutations:** read-only diagnostics; reset path is bounded and
  explicit (no production impact without authorization).
- **Defects found:** anomaly detectors surface `failed-process`,
  `pending-receipt`, `ready-task-uncorrelated`, `correlation-dangling-*`,
  `open-job-on-closed-token`, `multiple-active-instances`; live sweep clean at
  seed time.
- **Decisions made:** diagnostics are read-only anomaly detectors; reset is a
  separate, explicit, bounded path.
- **Blocking dependency:** none.

### S-026 — WF-10 Workflow End-to-End Trust Validation

- **Status:** `PASS`
- **Files changed:** commit `ec3947b` ("Complete workflow end-to-end trust
  validation"); `workflow_app/tests/acceptance.test.ts` (8),
  `deal-closing-date.test.ts` (4), `closing-timer.test.ts` (7),
  `re-supermodel.test.ts` (22), `materialization.test.ts` (3),
  `task-completion.test.ts` (3); `workflow_app/reconcile.ts`.
- **Tests/checks run:** full workflow_app test suite (in-memory fakes); live
  DEV verification per TUNIT register #7/#8/#12/#16.
- **Database mutations:** none from tests; live DEV instance exercised via
  runtime seams only.
- **Defects found:** duplicate command replay, closing-date reschedule
  duplicates, and task-correlation duplicates were the trust gaps validated
  here; all closed.
- **Decisions made:** duplicate replay must not double-mutate (`replayed: true`
  date unchanged); reschedule reuses the same instance/job; join releases
  exactly once; blockers gate closing readiness.
- **Blocking dependency:** none.

### S-027 — WF-11 TUNIT Harvest Register

- **Status:** `PASS`
- **Files changed:** `docs/tunit-harvest-register.md` (16 mechanisms).
- **Tests/checks run:** cross-checked against the listed artifacts
  (`workflow_engine/tests/hardening.test.ts`,
  `workflow_app/tests/command-receipt.test.ts`, `re-supermodel.test.ts`,
  `materialization.test.ts`, `task-completion.test.ts`, `closing-timer.test.ts`,
  `acceptance.test.ts`, `deal-closing-date.test.ts`, `version-policy.test.ts`,
  `workflow_app/diagnostics.ts`).
- **Database mutations:** none (documentation only).
- **Defects found:** one known remaining gap recorded — transactional
  concurrency of the join release under two simultaneous branch completions
  (sequential release was proven). **Superseded by S-028.**
- **Decisions made:** classification key (`UNIT` / `APPLICATION INTEGRATION` /
  `LIVE DEV` / `GLOBAL INVARIANT`); this is a harvest, not the TUNIT suite
  itself.
- **Blocking dependency:** none; feeds S-029.

### S-028 — WF-12 Join Release Concurrency Regression Test

- **Status:** `PASS`
- **Files changed:** commit `fddcd26` ("test: add regression test for
  concurrent fork branches joining exactly once");
  `workflow_app/tests/concurrency.test.ts` (4 tests, including join-exactly-
  once under simultaneous completions).
- **Tests/checks run:** `workflow_app/tests/concurrency.test.ts` via `tsx
  --test` (in-memory model of the `UNIQUE(command_id)` claim boundary).
- **Database mutations:** none (in-memory fake).
- **Defects found:** the TUNIT register's known-remaining-gap (join release
  concurrency) is now covered by a regression test. The note in
  `docs/tunit-harvest-register.md` predates this test and remains a
  documentation-update candidate (a separate doc change, not this story).
- **Decisions made:** two required fork branches completing concurrently
  against the same join release exactly once and produce exactly one downstream
  token; pending receipt rollback does not poison a future retry.
- **Blocking dependency:** none.

## 7. Batch 4 — Workflow Road Ahead (S-029 – S-041)

> All stories in this batch are `PENDING` at seed time. None are attempted.
> Detailed fields (files changed, tests, mutations, defects, decisions) are
> recorded when a story is attempted; for now only status and dependencies are
> tracked.

| Story ID | Title | Status | Blocking / note |
|---|---|---|---|
| S-029 | TUNIT Formal Suite | PENDING | Recommended next story |
| S-030 | RE Supermodel Deployment to Neon | PENDING | Operational gate + authorization |
| S-031 | Portal Workflows Experience | PENDING | Held by "no UI yet" constraint |
| S-032 | CRM-14 Closing Orchestration | PENDING | — |
| S-033 | Engine Error / Terminal Semantics | PENDING | — |
| S-034 | Engine Job Lease Requeue & Timer Auto-Advance | PENDING | — |
| S-035 | Engine Join Correlation & Optional Branch Hardening | PENDING | — |
| S-036 | Engine Optimistic Concurrency Guard Enforcement | PENDING | — |
| S-037 | Application Command Inventory Completion | PENDING | — |
| S-038 | Operational Seams: Alerts / Deadlines / SME / Audit | PENDING | Reviewed deferral acceptable |
| S-039 | Domain Event Persistence & Audit Trail | PASS | Decision recorded: DEFER (no new subsystem) |
| S-040 | Media / Attachment & Retention Policy | PENDING | Policy before ingestion |
| S-041 | Workflow Visual Modeler (Story 129) | PENDING | Held by "no UI yet" constraint |

- **S-029 — TUNIT Formal Suite.** PENDING. Depends on S-027/S-028. *Recommended
  next story.*
- **S-030 — RE Supermodel Deployment to Neon.** PENDING. Depends on S-020.
  Requires a reviewed environment and explicit authorization before any Neon
  deployment; production deploy is out of scope for the default path.
- **S-031 — Portal Workflows Experience.** PENDING. Depends on S-020/S-022/
  S-026. Held by the current "do not build the UI yet" constraint. Read-only
  summaries already exist (`workflow_app/read-service.ts`,
  `app/portal/workflows`).
- **S-032 — CRM-14 Closing Orchestration.** PENDING. Depends on S-019/S-020/
  S-024/S-026. Deferred from the V1 unblock tranche (`docs/agent/CURRENT.md`).
- **S-033 — Engine Error / Terminal Semantics.** PENDING. Depends on S-017/
  S-019. Archaeology §12 gaps.
- **S-034 — Engine Job Lease Requeue & Timer Auto-Advance.** PENDING. Depends
  on S-017/S-020. Archaeology §8/§10 gaps.
- **S-035 — Engine Join Correlation & Optional Branch Hardening.** PENDING.
  Depends on S-017/S-020/S-028. Archaeology §11 gaps.
- **S-036 — Engine Optimistic Concurrency Guard Enforcement.** PENDING.
  Depends on S-017/S-021/S-028. Archaeology §7/§8 gaps.
- **S-037 — Application Command Inventory Completion.** PENDING. Depends on
  S-018/S-019/S-024.
- **S-038 — Operational Seams: Alerts / Deadlines / SME / Audit.** PENDING.
  Depends on S-018/S-020/S-022. A reviewed deferral of a specific seam is an
  acceptable outcome.
- **S-039 — Domain Event Persistence & Audit Trail.** PASS. Depends on
  S-018/S-024. Decision recorded in
  [`docs/domain-event-persistence-decision.md`](../domain-event-persistence-decision.md):
  **DEFER** — no application `domain_event` table; canonical tables + engine
  `process_events`/`process_commands` + command receipts + `security_audit_event`
  already cover audit, correlation/causation, and replay. See the detailed
  record below.
- **S-040 — Media / Attachment & Retention Policy.** PENDING. Depends on
  S-005/S-006 boundaries and S-009. Policy must precede any byte ingestion.
- **S-041 — Workflow Visual Modeler (legacy Story 129).** PENDING. Depends on
  S-020/S-031. Held by the "no UI yet" constraint; editor round-trips the same
  XML grammar.

### S-039 — Domain Event Persistence & Audit Trail (CRM-14I)

- **Status:** `PASS`
- **Files changed:** `docs/domain-event-persistence-decision.md` (new decision
  record); `docs/workflow/STORYBOARD_STATUS.md` (this record + change log);
  `docs/workflow-integration-contract.md` (pointer from persistence posture).
  Documentation only — no code, no schema, no migration.
- **Tests/checks run:** scoped per ENG-20A runtime policy (SCOPED mode; full
  regression not authorized). `git diff --check` clean; `domain_event` grep
  across the repo confirms no table/code reference exists; every claim in the
  decision record line-checked against the cited seams
  (`workflow_engine/lib/workflow/engine.ts` `_handleCommand`/`_event`,
  `workflow_app/engine-bridge.ts`, `db/workflow-command-receipt.ts`,
  `db/offer-acceptance.ts`, `db/deal-stage.ts`, migrations 005/013/014/017/018,
  `docs/workflow-engine-archaeology.md`, `docs/workflow-integration-contract.md`).
  No tsc/build run — no code touched to warrant it.
- **Database mutations:** none (decision is DEFER; no migration recorded).
- **Defects found:** none. The archaeology §12 "no join event" note is a
  framework-event completeness item tracked by S-035, not an application
  domain-event persistence gap.
- **Decisions made:** **DEFER** — no application `domain_event` table now.
  Canonical immutable domain rows are the business audit trail;
  `workflow_command_receipt` provides idempotent command replay;
  `process_events`/`process_commands` are the durable engine-side execution log;
  `security_audit_event` covers auth/break-glass. The correlation/causation
  chain (instance id → `CommandEnvelope.correlationId` → `commandId` →
  `emittedEvents[].causationId` → `DomainEvent.eventId`) is preserved by
  existing seams and partly already durable. None of the three change-conditions
  (cross-cutting consumer; archaeology insufficiency; event-sourced replay
  requirement) is demonstrated. If one ever holds, the reviewed narrow
  append-only `domain_event` table design (invariants + same-transaction write
  via the existing claim-first pattern) is recorded in the decision doc.
- **Blocking dependency:** none.

## 8. Change log

| Date | Change | Author | Status impact |
|---|---|---|---|
| 2026-08-20 | Seed version 1: created `MASTER_STORYBOARD.md` (S-001…S-041, 4 batches) and this status file at main @ `fddcd26`. Documentation only. | Lead (storyboard story) | Established baseline; no stories `CURRENT` |
| 2026-08-22 | S-039 (CRM-14I) recorded `PASS`: decision documented in `docs/domain-event-persistence-decision.md` — DEFER, no application `domain_event` table; consumer-gap analysis + correlation/causation proof + change-condition evaluation recorded. Documentation only; no code/schema. | Builder (CRM-14I) | S-039 PENDING → PASS |
