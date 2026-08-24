# CulebraLuxe Current-State Continuity Supplement

Date: 2026-08-24  
Purpose: Preserve the engineering delta discovered after the final technical continuity packet and provide a deterministic bootstrap for a new ChatGPT Project/Work session.

Read this as continuation state, not as a new design exercise.

## 1. Required authority order

Read these repository documents before further CRM-27/CRM-26 work:

1. `docs/deal-workflow-architecture-specs-2026-08-24.md`
2. `docs/final-technical-continuity-packet-2026-08-24.md`
3. `docs/ARCH-01-README-SUPPLEMENT.md`
4. This supplement

Then reconcile all four with the actual branch, current code, and current Neon schema. If evidence conflicts, report the conflict. Do not silently redesign around it.

## 2. Current Deal-workflow branch

Remote branch:

`crm-27-26-deal-workflow`

Known commits:

- `5b7ef43` — CRM-27 candidate cherry-picked onto the documentation baseline
- `b2a6959` — Phase-1 CRM-27 durability repair

The branch was pushed to origin. DeepSeek stopped before CRM-26 and E2E because it identified participant-cardinality as a stop condition.

Migrations 067, 068, and 069 were committed as code only and were not proven applied to any Neon branch.

Do not begin CRM-26 until the Phase-1 repair is corrected and reviewed.

## 3. Independent architecture review of b2a6959

Overall assessment: useful and directionally correct, but not merge-ready. Approximate grade: 62/100.

What was good:

- Moved agreement-execution evaluation and marker claim into a canonical command transaction.
- Reused CommandDispatcher and the existing transactional outbox/MQ architecture.
- Kept separate commits and preserved unrelated work.
- Added targeted tests and stopped rather than inventing participant-cardinality semantics.
- Migration 069 changed the execution marker FK from delete cascade to audit-safer restrict semantics.

Critical defects found by reviewing production code paths rather than relying on the implementation report:

### 3.1 Production outbox composition is missing

Production dispatcher composition roots in `lib/commands/index.ts` and the BoldSign webhook path construct `CommandDispatcherImpl` without a real `PostgresOutboxEventRepository` event sink.

Tests inject a fake sink, so the tests prove an injectable design, not production publication.

Consequence:

- agreement_execution marker and command receipt can commit
- AGREEMENT_FULLY_EXECUTED may never be appended to outbox_message

The core acceptance criterion is therefore not satisfied in production.

### 3.2 Incomplete evaluation is terminally receipted

The command can evaluate an agreement as incomplete and finalize a successful command receipt. Reusing the same command identity later replays the old incomplete result rather than reevaluating newly accumulated signature evidence.

The re-drive identity must represent the current evidence generation/fingerprint, or incomplete evaluation must not become a permanently terminal result that suppresses future evaluation.

### 3.3 Re-drive is not durable

`lib/agreements/re-drive.ts` is a callable adapter, not a scheduler, worker, poller, or persisted retry guarantee.

Signature reconciliation intentionally treats agreement evaluation as non-fatal. If evaluation fails and the webhook returns success, there is no proven mechanism that guarantees another evaluation attempt.

A durable reconciliation runtime must reuse an existing worker/reconcile seam. Do not create another queue.

### 3.4 Input validation is insufficient

Unknown/non-P&S templates, missing templates, or zero declared signature roles can reach a vacuous “all required roles satisfied” result.

The canonical command must reject or explicitly classify:

- missing transaction document
- wrong document/template type
- missing immutable issued lineage
- no required execution policy
- no participant/signature slots
- evidence for a different issued version

### 3.5 Marker/event identity mismatch

The marker stores the command-envelope commandId in `agreement_execution.event_id`, while the emitted DomainEvent receives another random UUID.

The immutable transition marker and canonical outbox event need a deterministic, auditable identity relationship. Prefer the marker’s event ID to equal the actual DomainEvent/outbox message ID.

### 3.6 Participant-cardinality seam was only partially diagnosed

`signature_request.transaction_document_id` already scopes evidence to an immutable issued transaction document. The deeper gaps are:

- `transaction_document.source_snapshot` does not appear to include immutable `document_form_participant` data
- participant collections remain draft-side/mutable
- signature evidence records execution_role but not the immutable issued participant/signature-slot identity

The required execution predicate cannot use DISTINCT role alone. One BUYER request cannot prove that every buyer signed when there are multiple buyers.

Required approach:

- freeze required participant/signature slots into issued document lineage
- identify each slot stably
- associate execution evidence with the exact issued slot and version
- evaluate every required Buyer, Seller, and Seller Broker slot

Do not invent buyer1/buyer2 columns and do not redesign the provider seam casually.

### 3.7 Manual/external execution remains incomplete

The pure predicate accepts a manual boolean, but the durable command hardcodes false and no authorized evidence repository/path exists.

Manual/external execution is first-class, but it needs actor, timestamp, reason/note, document/version, authorization, and audit semantics.

## 4. Required corrective sequence before CRM-26

Use bounded commits.

### Pass 1 — production durability

- Wire every production canonical dispatcher composition root to `PostgresOutboxEventRepository`.
- Prove marker + command receipt + outbox append commit in one transaction.
- Prove rollback leaves none.
- Fix incomplete-evaluation replay semantics.
- Make re-drive genuinely durable using existing reconcile/worker infrastructure.
- Reject invalid document/template/policy/empty-role cases.
- Make marker event identity and emitted DomainEvent identity deterministic and auditable.
- Use targeted tests; do not run the full site regression.

### Pass 2 — execution evidence completeness

- Extend immutable issued lineage with participant/signature slots.
- Bind signature evidence to exact participant slots and issued version.
- Require every Buyer, every Seller, and every Seller Broker.
- Add authorized manual/external execution evidence.
- Prove duplicate evidence, partial evidence, multi-person roles, wrong version, and manual evidence.
- Do not begin CRM-26 until this passes architecture review.

### CRM-26 after repair

Only after CRM-27 is correct:

- consume AGREEMENT_FULLY_EXECUTED through the existing PostgresMessageBroker
- invoke canonical Deal commands
- advance the existing pns_executed workflow task through task correlation
- let existing XML run mark_under_contract
- do not start another workflow
- do not create another MQ or transaction-state model

Run the broader workflow/forms/signature regression only after CRM-27 + CRM-26 + accepted-Offer-to-under-contract E2E is proven.

## 5. Confirmed business decisions

These stop signs have now been answered:

- Purchase-price mismatch: reject P&S projection until the accepted Offer is amended.
- Full PR-PNS execution: every Buyer, every Seller, and every Seller Broker must execute.
- surveyDeadline: first-class canonical Deal deadline and real workflow timer.

Attention warning thresholds remain unresolved. Warning thresholds are policy, not duplicate contractual timers.

## 6. Existing command/MQ invariants

The existing Postgres mini-MQ is the bus:

- CommandDispatcher transaction
- canonical mutation
- workflow_command_receipt
- canonical DomainEvents
- PostgresOutboxEventRepository append in the same transaction
- outbox_message
- mq_subscription exact routing match
- mq_delivery lease/retry/dead-letter
- idempotent consumer-side commands

Do not create a second queue, event table, dispatcher, notification queue, or email queue.

CRM-25 is the semantic outbound action/correlation lifecycle above the existing MQ. It is not a request to build another broker.

## 7. Apple Contacts CRM-intake track

This is real CRM intake/SOP work, not a disposable utility.

### Proven local Swift exporter

Repository path:

`contact-export/`

The Swift CLI uses the supported macOS Contacts framework, not AddressBook SQLite.

Proven:

- Contacts permission granted
- 2,573 contacts visible
- complete JSON export generated
- multiple phone/email/address values preserved
- source contact identifier preserved
- optional sample limit supported

Successful command:

`swift run contact-export > contacts-export.json`

Verification returned 2,573 `sourceId` values.

The generated JSON contains private contact data and must remain ignored/uncommitted. The Swift source, package, tests, README, import script, and SOP should be committed as durable operational artifacts.

Known local staging command used from `contact-export/`:

`git add Package.swift Sources/contact-export/contact_export.swift Tests/contact-exportTests/contact_exportTests.swift .gitignore`

Do not use `git add .`; unrelated work exists.

### Existing intake architecture

Reuse:

- `lib/intake/contracts.ts`
- `lib/intake/batch.ts`
- `lib/intake/inbox.ts`
- `db/integration-inbox.ts`
- migration 044 integration inbox

The Swift source adapter must emit a neutral batch and must not know Neon schema.

Target flow:

Apple Contacts → neutral JSON batch → TypeScript intake adapter → integration inbox/staging → normalization → identity matching → human review when ambiguous → canonical Person/contact-point services.

### Apple adapter branch

Remote branch:

`apple-contacts-intake`

Known commits:

- `9b53316` — Apple Contacts neutral intake adapter
- `1e09909` — targeted adapter tests

Files:

- `lib/intake/apple-contacts.ts`
- `workflow_app/tests/apple-contacts-intake.test.ts`

The adapter preserves rich source facts in sourcePayload and deduplicates only exact identity candidates.

Tests were not executed in the isolated review checkout because node_modules was absent. Run the targeted adapter suite in the normal workspace before merge.

The existing integration inbox does not persist the full rich contact profile. A neutral contact-profile staging/persistence seam and TypeScript Neon loader are still required. Reconcile migration numbering after the Deal branch stabilizes.

## 8. Git/worktree safety

Known unrelated local work includes portal UI housekeeping, navigation tests, a deleted uploaded XML artifact, tsconfig build information, CRM-27.patch, contact-export work, and a safety stash created by DeepSeek.

Do not discard, reset, pop, or drop anything without inspecting current `git status` and the stash contents.

Do not use destructive Git commands.

Do not use `git add .`.

## 9. Neon identity/project reconciliation

The prior continuity packet named:

- project ID `billowing-snowflake-76768657`
- database `neondb`

After the Neon plugin was deleted and reinstalled, the authorized identity could not see that project and returned HTTP 404 authorization failure.

The currently visible project is:

- organization: culebraluxe
- project name: CulebraluxeData
- project ID: `snowy-salad-48970537`

Treat the correct current project ID as unverified until a read-only query confirms that `storyboard_story` and expected CulebraLuxe schema exist there.

Do not interpret plugin authorization failures as database outages.

The current Work chat has a stale MCP registry and cannot use the reinstalled Neon plugin. A newly started Project Work chat should receive a fresh plugin registry.

## 10. Story architecture table investigation

There is an actual Neon table used for story-level architecture deep dives. It was part of the successful Factory operating model: Architecture Pro wrote durable requirements into the database before implementation agents executed stories.

The exact table/schema remains to be discovered from Neon. Do not invent it.

When Neon access works:

1. query information_schema for tables containing architect/story
2. identify the architecture deep-dive table
3. read its schema, constraints, indexes, and representative completed-story records
4. read ARCH-01, SOP-1, CRM-26/27/28, and OPS-11A/B/C if present
5. make no writes until the current architecture is understood

The intended operating model should return to:

Architecture Pro → durable story architecture record → bounded implementation agent → targeted verification → architectural checkpoint.

## 11. ChatGPT Project operating model

The previous long Work chat could not be moved into a Project because it was not eligible; absence of “Move to project” is the product signal.

Create a personal Project named:

`CulebraLuxe Engineering`

Use Default memory, with Reference saved memories and Reference chat history enabled.

Start future Work conversations from inside the Project so they receive Project context and freshly registered plugins.

Git remains the exact durable authority. Project memory improves continuity but does not replace versioned architecture documents.

## 12. New-session bootstrap instruction

Use this exact instruction in the new Project Work chat:

Read the Culebraluxe-web repository continuity documents in authority order:
1. docs/deal-workflow-architecture-specs-2026-08-24.md
2. docs/final-technical-continuity-packet-2026-08-24.md
3. docs/ARCH-01-README-SUPPLEMENT.md
4. docs/current-state-continuity-supplement-2026-08-24.md

Then inspect the actual current Git branches and reconcile the documents against code. Do not implement or mutate anything yet. Report contradictions first. Connect read-only to the currently authorized Neon CulebraLuxe project, verify the canonical project by finding storyboard_story, then identify and read the story architecture deep-dive table. Preserve the existing command pattern, transactional outbox, Postgres mini-MQ, canonical Deal ownership, workflow/application boundary, and immutable document lineage. Do not create another model or queue.
