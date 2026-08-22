# CRM-23 — macOS External Activity Observer + Durable Integration Inbox

Status: implemented (DEV branch), awaiting human review on the Story Board.

## Architecture (the Mac is an integration edge, not the CRM)

```
MacIntegrationObserver (lib/mac-observer)
  -> source observers/adapters (ContactsObserver, CalendarObserver,
     MailObserver, MessagesObserver, WhatsAppObserver, future observers)
  -> ExternalActivityEvent            (source-neutral fact, schemaVersion 1)
  -> durable Integration Inbox        (lib/integration-inbox + migration 044)
  -> identity/contact resolver        (existing CRM-03 person resolution)
  -> existing CRM intake stubs        (calendar/email/communications/whatsapp
                                       coordinators + generic intake)
  -> canonical Business Command layer (interaction.record, lib/commands)
  -> canonical CRM truth              (interaction table; person spine)
  -> (future) transactional outbox    (CRM-14J contracts, downstream alerting)
```

**Observer responsibility — acquisition only.** `MacSourceObserver.observe()`
returns raw observations; adapters lower them into the neutral
`ExternalActivityEvent`; `MacIntegrationObserver.acquire()` filters to
`available` capabilities and lowers. Nothing in this layer maps an email to a
task, a WhatsApp to a deal, or a calendar change to a workflow. Those
decisions belong to the mapper/intake stubs (identity + interaction only) and,
for workflow transitions, to future outbox subscribers.

**Inbox responsibility.** Durable receipt of inbound external facts:
`integration_inbox` with UNIQUE `(source, source_account, external_event_id)`
dedupe (insert-or-read, replay-safe), claim/reclaim (`received -> processing`,
15-minute stale reclaim), terminal transitions (`completed | rejected |
resolution_required | duplicate`), bounded retry (`attempt_count` +
`max_attempts`), and **poison/dead-letter** (`poisoned` = attempts exhausted →
HumanRequired escalation). Failure is isolated per receipt — a poisoned event
never blocks other intake. `correlation_id`, `thread_id`, `content_reference`
and `provenance_reference` preserve correlation and provenance.

**Mapper responsibility.** Translates the source-neutral event into the
EXISTING channel intake contracts (`CalendarProviderEvent`,
`EmailProviderMessage`, `CommunicationsProviderEvent`, `WhatsAppProviderEvent`)
and the generic `InboundEvent` (contacts), then hands off to the existing
coordinators (`prepareCalendarIntake`, `prepareEmailIntake`,
`prepareCommunicationsIntake`, `prepareWhatsAppIntake`, `prepareInboundEvent`).
No parallel canonical intake tables. Person auto-creation is NEVER enabled for
observer-derived facts (`allowCreation = false` everywhere — observed
records/transports are not ownership proof, mirroring the calendar rule).

**Command/domain responsibility.** Canonical CRM changes happen through the
canonical Business Command layer: `interaction.record` (claim-first receipt,
emits `INTERACTION_RECORDED`). Production wiring
(`lib/integration-inbox/wiring.ts`) routes the inbox's interaction persistence
through the command dispatcher; tests inject in-memory persistence.

**Outbox (future).** Committed facts → downstream alerting/workflow/
integrations via the CRM-14J transactional outbox contracts
(`lib/events/outbox-contracts.ts`). Not implemented until a real consumer
exists (CRM-14I defer preserved).

## Source access mechanisms (documented + revocable)

| Source | Capability | Required access | Notes |
| --- | --- | --- | --- |
| Contacts | `available` | TCC consent (`tcc:contacts`) + a bounded macOS observer process | Contacts.framework change facts only — never the whole address book. |
| Calendar | `available` | TCC consent (`tcc:calendar`) via EventKit | Calendar change facts only (created/updated/deleted). |
| Mail | `unproven` | Mail + Full Disk Access, or AppleScript Mail automation, or IMAP app-specific credentials | Adapter contract complete; observer emits NOTHING until access is configured and reviewed. |
| Messages/iMessage | `unsupported` | (none acceptable) | No public API; reading the Messages DB needs SIP bypass — unacceptable. Contract stays complete for a future reviewed mechanism; the processor refuses `unsupported` sources. |
| WhatsApp | `unsupported` | (none acceptable) | No public macOS API; desktop DB is proprietary/undocumented. A real connector belongs to the provider webhook seam (CRM-07), below the observer boundary. |

Every access mechanism is least-privilege: the observer persists only the
neutral business facts the CRM needs and never scrapes an app database. Access
is revocable by removing TCC consent / credentials / the observer process —
the node side never holds source credentials.

## Privacy / retention policy (criterion 10)

- Raw payloads, message bodies, and source databases are NEVER persisted into
  canonical tables. `contentReference` / `provenanceReference` point at
  bounded, revocable artifacts (provider content ids, local observation
  files).
- The inbox row stores only neutral essentials: participant identities,
  contact candidates, thread reference, subject/summary, attachment
  descriptors (references, not bytes), correlation + provenance references.
- Canonical CRM stores normalized business data (interaction rows), never
  arbitrary app databases.
- `sanitizeRawMetadata` (existing CRM-02 rule) rejects secret-shaped keys and
  bounds metadata to 32 KB on every intake path.

## Honesty guarantees (criterion 8)

- `SourceCapability.status` is `available | unproven | unsupported` with a
  human-readable reason and the concrete required access. No fabricated
  access, no brittle coupling hidden above the adapter.
- The integration-inbox processor re-checks the capability gate (defense in
  depth) and returns `skipped_unsupported` for non-`available` sources.
- The fake observer (`lib/mac-observer/fake-observer.ts`) declares the SAME
  capabilities as the real adapters by default; tests that exercise a
  non-`available` adapter pass an explicit `fake-for-test` override.

## Files

- `lib/mac-observer/contracts.ts`, `adapters/{contacts,calendar,mail,messages,whatsapp}-adapter.ts`, `fake-observer.ts`, `index.ts`
- `lib/integration-inbox/contracts.ts`, `mapper.ts`, `processor.ts`, `wiring.ts`
- `lib/commands/interaction/record-interaction.ts` (+ `command-types.ts`, `register.ts`)
- `db/migrations/044_integration_inbox.sql`, `db/integration-inbox.ts`
- `db/migrations/045_storyboard_crm23.sql`
- `workflow_app/tests/mac-observer-inbox.test.ts`

## Verification

`workflow_app/tests/mac-observer-inbox.test.ts` (targeted, zero Neon —
in-memory FakeDb + fake observer) proves: neutral contract across all five
sources; stable identity/occurredAt/participants/thread/provenance/correlation
on every event; inbox dedupe + replay; acquisition-only observer; identity
resolution before mutation; reuse of the existing intake stubs; command-seam
persistence; honest unsupported capabilities; poison isolation; minimal
retention. Typecheck (`tsc --noEmit`) passes.
