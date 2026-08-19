# Current Story

## CRM-07 — WhatsApp Intake

Status: Architecture only; awaiting a canonical-channel decision. No implementation is authorized.

## Completed Foundations

- CRM-01 provides source-idempotent interaction inputs and the database uniqueness backstop.
- CRM-02 provides neutral inbound events, strict identity normalization, exact context resolution, and advisory intents.
- CRM-03 provides explicitly authorized atomic person/identity creation with existing-person-wins race recovery.
- CRM-04 provides website intake; CRM-05 provides provider-neutral email intake.
- CRM-06 provides a reviewed fixture-only communications boundary for calls, SMS, and iMessage, including strict-E.164 phone identity, endpoint classification, assurance mapping, duplicate-first coordination, and exact context resolution.

## Goal and Boundary

Design the smallest provider-neutral boundary through which a future WhatsApp Cloud API or equivalent connector could translate terminal message deliveries into canonical CRM intake.

CRM-07 is architecture only. It adds no Meta SDK or types, webhook route, signature verifier, live request, credential, environment variable, acknowledgement, cursor/receipt, database write, schema change, interaction persistence, media fetch, UI, reply/send behavior, task, interest, workflow, notification, or AI behavior.

## Reuse and Separation

Reuse from CRM-06:

- injected owned/shared/system endpoint configuration;
- strict E.164 for every person identity;
- exact phone identity resolution across SMS, iMessage, calls, and WhatsApp;
- source-token and opaque provider-event-ID validation;
- transport observation versus ownership assurance;
- duplicate source identity before person/context resolution;
- exact trusted property/deal context only;
- CRM-02/03 repository boundaries and result semantics.

Do not reuse CRM-06 by pretending WhatsApp is SMS. There is no `whatsapp` person-identity type: WhatsApp actors resolve through canonical `phone`. Transport/channel and identity are separate concepts.

Provider SDK/webhook objects must be translated by a future connector into a neutral `WhatsAppProviderEvent` before CRM code. Meta names, object shapes, signature headers, access tokens, phone-number IDs, and media URLs never cross that boundary.

## Canonical Channel Decision Gate

The current TypeScript and database interaction-channel contracts do not include `whatsapp`. Mapping WhatsApp to `sms`, `imessage`, or generic `message` would corrupt transport meaning; the approved CRM-01 channel set intentionally has no generic `message` value.

Therefore CRM-07 does not authorize implementation or schema work. Before a Builder POC can lower a WhatsApp event into `InboundEvent`, architecture must approve one of:

1. add `whatsapp` to the neutral TypeScript channel and the existing interaction channel constraint in a separately reviewed narrow migration; or
2. approve a different canonical transport model in a separate story.

The recommended future decision is the narrow `whatsapp` channel addition, but no schema change is proposed or authorized in CRM-07.

## Proposed Neutral Event Contract

A future connector would emit a bounded `WhatsAppProviderEvent` containing only:

- normalized provider and account namespace;
- stable, opaque, case-preserving provider message ID;
- occurrence time;
- raw `from` and `to` endpoint addresses;
- optional provider-declared direction for agreement checking only;
- external-actor assurance: `transport_observed | ownership_verified | authenticated_actor`;
- content class: `free_form | template | service | system`;
- optional connector-extracted bounded plain text;
- optional opaque template identifier for outbound template audit only;
- optional bounded attachment descriptors;
- optional display-name hint;
- optional trusted exact property/deal context.

Injected configuration remains authoritative for the owned WhatsApp business number, shared external numbers, non-person system endpoints, account namespace, and any future creation role. Provider event content cannot claim ownership or assign a CRM role. Configuration must reject duplicate/cross-category endpoints exactly as CRM-06 does.

## Direction, Identity, and Assurance

- Direction is derived from configured owned business endpoints and must agree with any provider-declared direction.
- Inbound has exactly one external strict-E.164 actor and an owned business recipient; outbound has an owned sender and exactly one external strict-E.164 recipient.
- Multiple external actors, shared numbers, withheld actors, groups, and ambiguous endpoints return `resolution_required`; malformed or missing owned endpoints reject; internal-only and configured system traffic exclude.
- A WhatsApp address never becomes a new identity kind. The actor hint is `{ kind: 'phone', value: strictE164 }`.
- Names, profile labels, provider contact IDs, message text, prior traffic, templates, AI, and phone-number display names are never identity keys.
- A valid signed webhook proves provider delivery integrity, not human ownership. It maps only to `transport_observed`/`user_supplied`.
- Exact existing active phone ownership may resolve at transport-observed assurance. Unknown transport-observed actors require `resolution_required`; no automatic person creation is authorized.
- A future connector may supply stronger assurance only through a separately reviewed explicit contract. Webhook signature, message receipt, business-number membership, and provider delivery status cannot elevate assurance.
- Archived/conflicting identity ownership keeps CRM-03 behavior and never attaches or creates silently.

## Source Identity and Duplicate Delivery

- Proposed source system: `communications:<provider>:<accountNamespace>` using the existing normalized source-token grammar.
- Proposed external ID: `whatsapp:<providerMessageId>` using the existing bounded opaque identifier rules.
- The provider message ID, not conversation ID, phone, timestamp, template, or content hash, is authoritative for idempotency.
- Duplicate webhook delivery must check `(source_system, source_external_id)` before identity or context work and return the existing interaction reference without later calls.
- Delivery/read/status callbacks, reactions, edits, revocations, retries, and webhook batches do not create additional person interactions in this POC. A future connector must collapse a provider envelope into individual terminal message events before calling CRM.

## Content and Message Class

- `free_form`, `template`, and user-visible `service` describe accepted message provenance; `system` events are excluded from person interaction history.
- Template messages are outbound only. An inbound template classification rejects as contradictory.
- Plain text is connector-extracted, NFKC-normalized, line-ending normalized, control-checked, trimmed, and bounded to 1–4,000 Unicode code points, reusing CRM-06 rules.
- A message may contain text, attachment descriptors, or both. A contentless message rejects.
- `source_metadata` remains closed: `transport: 'whatsapp'`, `contentClass`, optional opaque `templateId`, and optional sanitized attachment descriptors. Text belongs in canonical summary, not metadata.
- No arbitrary webhook payload, headers, contacts collection, credentials, access tokens, signed URLs, phone-number IDs, delivery receipts, or raw provider metadata is admitted or logged.

## Attachments and Media Boundary

Attachment descriptors may contain only a bounded opaque provider media ID, bounded filename when present, exact MIME type, and non-negative safe-integer size when known. Reject URL-like identifiers and cap descriptor count and serialized metadata using the existing email/CRM sanitizer conventions.

Descriptors are references for a future reviewed fetch/import process only. CRM-07 does not fetch bytes, persist descriptors, create `media`, create `property_media`, expose provider URLs, or treat an attachment as property media. Provider-hosted media retention and expiration must not be confused with CulebraLuxe media ownership.

## Context, Intents, and Persistence

- Property/deal context comes only from trusted exact adapter context and retains CRM-02 agreement and deal-ownership rules.
- Text, captions, filenames, templates, names, AI, and conversation history never select property, deal, or person context.
- No requested action is inferred. Advisory task/property-interest intents remain empty.
- No person, interaction, task, interest, receipt, media, or other persistence is reachable in CRM-07 architecture.

## Privacy and Retention

- Retain only normalized facts required for canonical interaction history; never retain the raw webhook envelope in CRM metadata.
- Message bodies may contain sensitive client content and must use the existing bounded canonical summary contract.
- Provider media IDs and descriptors require a future explicit retention/deletion policy before live ingestion.
- A live webhook requires a separately reviewed durable receipt/acknowledgement and retry boundary, signature-verification boundary, least-privilege credential handling, and operational retention schedule. It must not reuse `website_intake_submission` or become a generic staging/ODS store.

## Schema Decision

No schema change is authorized in CRM-07. Architecture has identified a future narrow channel decision, not approved a migration. The existing person identity and all resolution/context contracts are sufficient; only canonical WhatsApp transport representation remains unresolved.

## Risks and Open Decisions

- Approve or reject the recommended narrow `whatsapp` interaction-channel addition before any implementation.
- Select and validate a provider capability before defining webhook acknowledgement, signature verification, cursor/receipt, and stronger assurance.
- Define attachment fetch, malware screening, media ownership, expiry, and retention before media ingestion.
- Define consent, opt-in/opt-out, message-window/template policy, deletion/export, and jurisdictional requirements before live traffic.
- Shared business numbers, number reassignment, groups, replies, reactions, edits, delivery/read state, and outbound sending remain separate stories.
