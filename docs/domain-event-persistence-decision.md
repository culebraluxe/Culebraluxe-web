# CRM-14I — Domain Event Persistence Decision (S-039)

Status: **DECIDED — DEFER.** Do not build durable application-side
`domain_event` persistence now. No new subsystem is required for V1. The
correct implementation of this story is "no new subsystem required".

This document records the decision, the consumer-gap analysis that supports it,
the proof that the correlation/causation chain is preserved by existing seams,
the change-condition evaluation, and the reviewed design that would apply if a
change-condition ever holds. It is the durable record for storyboard entry
[S-039 — Domain Event Persistence & Audit Trail](workflow/MASTER_STORYBOARD.md).

Companion documents:

- [workflow-integration-contract.md](workflow-integration-contract.md) — the
  seam contract; §"Persistence / audit posture" already states the posture this
  decision makes explicit.
- [workflow-engine-archaeology.md](workflow-engine-archaeology.md) — the CRM-14
  engine evaluation (change-condition 2's evidence source).

---

## 1. The question

Do we need durable application-side domain-event persistence *beyond* what the
current model already gives us — canonical immutable domain rows,
`workflow_command_receipt`, the engine's `process_events`/`process_commands`
logs, and `security_audit_event`?

The answer is **no**. Each concern a durable application event stream would
serve — audit, correlation/causation, replay — is already served durably by an
existing seam, and there is no consumer today that requires a cross-cutting
application event stream.

## 2. The current model (what already exists)

| Concern | Existing seam | Evidence |
|---|---|---|
| Business audit trail (canonical, immutable domain rows) | `interaction` (source-idempotent rows, `event_type`, `source_metadata`), `showing` lifecycle (`requested → scheduled → completed/cancelled` with per-state timestamps), `task` history, `offer` (counter-offer = **new row** with `parent_offer_id`, never an in-place mutation), `deal` stage CAS with `updated_at`/`closed_at` | [`db/migrations/005_crm_interaction_task_foundation.sql`](../db/migrations/005_crm_interaction_task_foundation.sql), [`db/migrations/013_showing.sql`](../db/migrations/013_showing.sql), [`db/migrations/014_offer.sql`](../db/migrations/014_offer.sql), [`db/deal-stage.ts`](../db/deal-stage.ts) |
| Idempotent command replay (application side) | `workflow_command_receipt` — `command_id` PK, claim-first winner, `finalizeReceipt` written **in the same transaction** as the business effect; `pending` is a sentinel, never a terminal outcome | [`db/migrations/018_workflow_command_receipt.sql`](../db/migrations/018_workflow_command_receipt.sql), [`db/workflow-command-receipt.ts`](../db/workflow-command-receipt.ts), [`db/tx.ts`](../db/tx.ts), [`db/offer-acceptance.ts`](../db/offer-acceptance.ts), [`db/deal-stage.ts`](../db/deal-stage.ts) |
| Durable engine-side execution log | `process_events` — append-only, partitioned by `created_at`, PK `(id, created_at)`, immutable by convention (no UPDATE/DELETE path in code); framework events carry `commandId`/`outcome` in `data`; read back by `getProcessHistory`, `workflow_app/read-service.ts`, `workflow_app/diagnostics.ts` | [`workflow_engine/scripts/schema.sql`](../workflow_engine/scripts/schema.sql) §6, [`workflow_engine/lib/workflow/engine.ts`](../workflow_engine/lib/workflow/engine.ts) (`_event` INSERT, `getProcessHistory`), [`workflow_app/read-service.ts`](../workflow_app/read-service.ts) `getWorkflowDetail`, [`workflow_app/diagnostics.ts`](../workflow_app/diagnostics.ts) |
| Durable engine-side command log (correlation root) | `process_commands` — `command_id` UNIQUE, `correlation_id` (= process instance id), `causation_id`, `input`, `outcome`, `message`; idempotent replay reuses the stored row (`command.replayed` event) | [`db/manual/2026-08-20_v4_crm14_workflow_activation.sql`](../db/manual/2026-08-20_v4_crm14_workflow_activation.sql) `create table process_commands`, [`workflow_engine/lib/workflow/engine.ts`](../workflow_engine/lib/workflow/engine.ts) `_handleCommand` |
| Auth / break-glass audit | `security_audit_event` — deliberately separate, security-significant events only | [`db/migrations/017_security_audit_event.sql`](../db/migrations/017_security_audit_event.sql), [`db/security-audit.ts`](../db/security-audit.ts) |

Nothing in the application writes a durable `domain_event`/`application_event`
row today, and **no command currently emits a `DomainEvent`** — every
`CommandResult.emittedEvents` in the codebase is `[]`
([`db/offer-acceptance.ts`](../db/offer-acceptance.ts),
[`db/deal-stage.ts`](../db/deal-stage.ts),
[`db/deal-financing.ts`](../db/deal-financing.ts),
[`db/deal-appraisal.ts`](../db/deal-appraisal.ts),
[`db/deal-closing-date.ts`](../db/deal-closing-date.ts),
[`db/transaction-document.ts`](../db/transaction-document.ts),
[`workflow_app/command-router.ts`](../workflow_app/command-router.ts)). The
`DomainEvent` contract exists as a seam capability
([`lib/workflow/contracts.ts`](../lib/workflow/contracts.ts)), not as a
producer with a durable-sink requirement.

## 3. Acceptance criterion 1 — consumer-gap analysis

No current requirement needs durable application events beyond canonical tables
+ engine `process_events` + command receipts.

| Potential consumer | Served by | Gap? |
|---|---|---|
| Business audit trail (who did what, when, on which aggregate) | Canonical immutable domain rows (`interaction`, `showing`, `task`, `offer`, `deal`) carry the business fact and its timing; `workflow_command_receipt` carries command id + outcome + aggregate id for every routed command; engine `process_events`/`process_commands` carry the orchestration trace | No |
| Workflow cockpit / Portal workflows UI | Reads engine tables directly (`process_instances`, `process_events`, `tokens`, `tasks`, `jobs`) via [`workflow_app/read-service.ts`](../workflow_app/read-service.ts) | No |
| IT diagnostics / anomaly detection | [`workflow_app/diagnostics.ts`](../workflow_app/diagnostics.ts) queries `process_events`/`process_commands` and canonical tables | No |
| Idempotent command replay / dedupe | `workflow_command_receipt` (application) + `process_commands` (engine) — both durable, both keyed by `command_id` | No |
| Auth / break-glass audit | `security_audit_event` (deliberately separate surface) | No |
| Notifications / alerts | **No consumer exists.** Integration-contract non-goal ("no alert delivery"); S-038 explicitly allows a reviewed deferral of alert/SME seams | No consumer → no requirement |
| SME portal | **No consumer exists.** Integration-contract non-goal ("no SME portal") | No consumer → no requirement |
| Reporting over an event stream | **No consumer exists.** Reporting today is a query over canonical tables ([`db/reporting.ts`](../db/reporting.ts)); a future reporting need is served by the canonical tables + engine logs it already uses | No consumer → no requirement |

**Conclusion:** every current consumer is served by an existing durable seam.
There is no cross-cutting event-stream consumer (notifications, SME portal,
event-stream reporting) that canonical tables + engine events cannot serve, so
there is no requirement that would be unmet without an application
`domain_event` table.

## 4. Acceptance criterion 2 — correlation/causation chain preserved by existing seams

The chain defined in the integration contract
([`docs/workflow-integration-contract.md`](workflow-integration-contract.md)
§"Correlation / causation") and the contract types
([`lib/workflow/contracts.ts`](../lib/workflow/contracts.ts)) is:

```
workflow instance id
  → CommandEnvelope.correlationId
  → CommandEnvelope.commandId
  → CommandResult.emittedEvents[].causationId
  → DomainEvent.eventId
```

Every hop is either already durable or carried by an existing seam:

1. **workflow instance id → `CommandEnvelope.correlationId`** — the engine
   constructs the command request with `correlationId: instance.id`
   ([`workflow_engine/lib/workflow/engine.ts`](../workflow_engine/lib/workflow/engine.ts)
   `_handleCommand`), and the application bridge maps it onto the envelope
   verbatim ([`workflow_app/engine-bridge.ts`](../workflow_app/engine-bridge.ts)
   `toCommandEnvelope`). The instance id is durable (PK of
   `process_instances`), and the engine persists the request's
   `correlation_id`/`causation_id` on `process_commands` in the engine
   transaction.
2. **`CommandEnvelope.commandId`** — deterministic
   `sha256(instanceId:nodeId)` ([`engine.ts`](../workflow_engine/lib/workflow/engine.ts)
   `_commandId`); durable on `process_commands.command_id` (UNIQUE) and on
   `workflow_command_receipt.command_id` (PK). A retried step reuses the same
   command id, so the chain is stable across retries.
3. **`CommandEnvelope.commandId` → `CommandResult.emittedEvents[].causationId`**
   — contract-defined mapping ([`contracts.ts`](../lib/workflow/contracts.ts)
   `CommandResult`); the engine already surfaces the command outcome durably
   (`process_commands.outcome` + `process_events` `command.completed`/
   `command.failed`/`command.replayed` events carrying `commandId`). No
   command emits events today, so there is no in-flight producer to lose.
4. **`DomainEvent.eventId` → `.correlationId` / `.causationId`** — contract
   fields on `DomainEvent` ([`contracts.ts`](../lib/workflow/contracts.ts)
   `DomainEvent`); the bridge maps engine command results onto the contract
   shape ([`engine-bridge.ts`](../workflow_app/engine-bridge.ts)
   `toApplicationCommandResult`).

**Conclusion:** the correlation/causation chain is fully preserved by the
existing seams. The engine log is the durable execution-side trace keyed by
instance id / command id; the receipts are the durable application-side
command trace; the contract carries the DomainEvent-shaped chain for the day a
producer appears. Nothing about deferring an application `domain_event` table
breaks or weakens the chain.

## 5. Acceptance criterion 3 — change-condition evaluation

Per the architect brief, an application `domain_event` table is justified only
if at least one of three change-conditions holds. None holds today.

1. **A durable cross-cutting event consumer appears** (notifications, SME
   portal, reporting over an event stream) that canonical tables + engine
   events cannot serve.
   **Not demonstrated.** No such consumer exists (see §3); the integration
   contract lists alert delivery and SME portal as explicit non-goals, and
   S-038 permits — but has not shipped — reviewed seam implementations.
2. **Engine-archaeology (CRM-14 evaluation) concludes the engine
   `process_events` log is insufficient for audit/rebuild.**
   **Not demonstrated — the evaluation concludes the opposite.** The
   archaeology verdict is **"B. ENGINE SUFFICIENT WITH SMALL EXTENSIONS"**
   ([`docs/workflow-engine-archaeology.md`](workflow-engine-archaeology.md)
   §22); `process_events` is classified **KEEP + HARDEN** (§20),
   immutable by convention (§12), and its reconstruction capability is
   explicitly assessed as *sufficient to reconstruct token movement and task
   lifecycle* (§12). The archaeology gap is the *DomainEvent bridge at the
   adapter boundary* (§19/§20), not durable persistence: the log itself is
   durable. No conclusion of insufficiency exists to trigger this condition.
3. **A temporal/event-sourced application-state replay requirement emerges.**
   **Not demonstrated.** V1 is state/command based; the integration contract
   lists **"no event sourcing"** as an explicit non-goal, and no story in the
   roadmap (S-029…S-041) introduces a temporal replay requirement.

**Conclusion:** no change-condition is demonstrated, so **no `domain_event`
table is created** by this story. This story changes documentation only — no
migration, no schema, no code.

## 6. Acceptance criterion 4 — if ever built (conditional design, not built)

The reviewed design below is recorded now so the decision can be revisited
quickly and safely; it is **not implemented** by this story. If a
change-condition from §5 is ever demonstrated, the change must be a NARROW,
additive, append-only application table:

```sql
-- NARROW application-side domain event log (IF EVER BUILT — NOT CREATED NOW)
create table domain_event (
  event_id       text primary key,          -- unique; the DomainEvent.eventId
  event_type     text not null,
  aggregate_type text not null,
  aggregate_id   text not null,
  correlation_id text,
  causation_id   text,
  occurred_at    timestamptz not null,
  payload        jsonb not null default '{}'
);
```

Invariants (from the architect brief):

- `domain_event` is **append-only and immutable** — no UPDATE/DELETE path, no
  partial aggregate rebuild; it is a log, never a source of truth.
- `event_id` unique (PK).
- Correlation/causation chain preserved end-to-end (the §4 chain, now with
  durable app-side hops).
- Written **in the same transaction as the command that produced it** — the
  existing claim-first transaction pattern
  ([`db/tx.ts`](../db/tx.ts), [`db/offer-acceptance.ts`](../db/offer-acceptance.ts))
  is exactly the seam a future write would ride: claim receipt → mutate
  canonical row → insert `domain_event` → finalize receipt, all atomic.

A future implementation would additionally record an additive migration in
`db/migrations` and keep `interaction(source_system, source_external_id)`
untouched (storyboard S-039 acceptance criteria).

## 7. Risks

- **Building now** would create unused infrastructure plus dual-truth drift:
  an application `domain_event` log would duplicate the engine's
  `process_events`/`process_commands` trace and the canonical rows, with no
  consumer to reconcile them.
- **Deferring** means a future consumer must source from canonical tables +
  engine events. That is acceptable and already documented in the integration
  contract (§"Persistence / audit posture"), and the §5 conditions give an
  explicit, bounded trigger for revisiting.

## 8. Verification record

- **Runtime test policy:** SCOPED (ENG-20A). The story acceptance criterion 5
  ("full suite + tsc + build pass") is superseded for this command by the
  runtime test-execution policy: full regression is not authorized, and
  typecheck/build is run only when the touched code warrants it.
- **Change scope:** documentation only — no code, no schema, no migration.
  There are no changed seams, so there are no targeted tests to run.
- **Checks performed:**
  - `git diff --check` (repo whitespace invariant) — clean.
  - Every claim in this document was verified against the cited files at
    commit time (seams listed in §2 and §4 were read and line-checked).
  - Confirmed `grep domain_event` across the repository: no table, no code
    reference exists (no accidental partial implementation).
- **Not performed (per policy):** no `pnpm test*`/tsc/build — no code touched
  to warrant them.

## 9. Artifacts

- `docs/domain-event-persistence-decision.md` (this document)
- `docs/workflow/STORYBOARD_STATUS.md` — S-039 marked `PASS` with evidence
- `docs/workflow-integration-contract.md` — one-line pointer from the
  persistence posture to this decision
