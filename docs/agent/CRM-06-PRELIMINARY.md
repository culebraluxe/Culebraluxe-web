# CRM-06 Preliminary Architecture

## Phone / SMS / iMessage Intake

Status: Preliminary Lead design; ready for architecture review. No implementation is authorized.

## Goal and Boundary

Define the smallest provider-neutral adapter POC that can translate call, SMS, and iMessage transport events into the existing CRM-02/03 intake contracts. The POC is fixture-only: no Apple access, carrier/provider SDK, webhook, polling, message sending, recording, transcription, route, UI, database write, or environment configuration.

## Existing Foundations

- `person_identity(identity_type = 'phone', identity_value)` is the canonical phone identity. Its existing uniqueness constraint makes one normalized phone number belong to at most one canonical person.
- CRM-02 already requires strict E.164 phone identities. National formats without an explicit country code are rejected.
- `interaction.channel` already supports `call`, `sms`, and `imessage`; direction, duration, event type, source identity, summary, and bounded source metadata already exist.
- CRM-03 already supplies safe explicitly authorized person creation, atomic phone-identity claiming, and existing-person-wins race recovery.
- SMS and iMessage are transport channels, not identity types. Both resolve through the same canonical `phone` identity as calls.

## Provider-Neutral Adapter Contract

A future `CommunicationsProviderEvent` should contain only normalized transport facts:

- provider and stable non-secret account namespace;
- transport kind: `call`, `sms`, or `imessage`;
- stable provider event ID and occurred-at timestamp;
- an explicit connector assurance classification for the external endpoint, derived only from an approved transport-specific capability;
- provider-declared direction when trustworthy;
- endpoint collections containing phone number plus an explicit owned/internal-line flag derived from injected configuration;
- for a completed call: start/end or duration, and a small allowlisted disposition when the provider supplies one;
- for SMS/iMessage: provider-extracted plain-text message content;
- optional provider conversation/thread ID as correlation metadata only;
- optional trusted exact property/deal context hints;
- an optional sender display-name hint, never an identity key;
- explicit allowlisted metadata only, never an arbitrary provider payload.

Provider SDK objects must be mapped into this neutral contract outside CRM. The adapter must not contain Twilio, Apple, carrier, or device-specific repository behavior.

## Transport Assurance Policy

Webhook authenticity proves that an event came from the configured provider; it does not prove that an inbound caller or sender owns an asserted phone number. The neutral contract must keep those claims separate.

- Exact phone ownership already present in `person_identity` may resolve regardless of creation assurance, subject to the unchanged active/archived/conflict rules.
- Unknown inbound auto-creation is allowed only when the connector supplies an explicitly approved endpoint-assurance value backed by a transport capability that verifies control or ownership strongly enough for canonical creation. A signed webhook, caller ID, SMS `From`, or provider-parsed envelope alone is insufficient and must be classified `transport_observed`; unknown `transport_observed` actors return `resolution_required`.
- Approved assurance classifications are `transport_observed`, `ownership_verified`, and `authenticated_actor`. Only `ownership_verified` or `authenticated_actor` may map to CRM-03 `provider_asserted` or `authenticated` evidence for creation. Approval of a concrete provider capability is a separate architecture/security decision; the preliminary POC supplies assurance as fixture configuration and does not declare any real provider approved.
- For outbound events, the configured owned business line establishes CulebraLuxe as sender, but delivery to an external number does not prove who owns that recipient number. An exact existing identity may resolve; an unknown outbound recipient remains `resolution_required` unless the external actor was selected by an authenticated canonical person context. Provider delivery success alone never authorizes person creation.
- Connector assurance is evaluated before CRM-03 creation policy and may only narrow eligibility. Display names, message replies, prior traffic, delivery receipts, and AI cannot raise assurance.

## Canonical Phone Identity

- Every external endpoint used for identity must normalize through the existing strict E.164 function.
- A valid example is `+17875550123`; punctuation, extensions, local/national-only numbers, short codes, alphanumeric sender IDs, withheld/anonymous values, and malformed numbers are not canonical person identities.
- Calls, SMS, and iMessage for the same E.164 value resolve to the same `person_identity` row.
- Display names, contact-card names, device labels, message signatures, subjects, and prior correspondence are hints only.
- External provider/contact identifiers may be retained as bounded correlation metadata but must not supersede the phone identity or silently become a second identity key in this POC.

Deterministic invalid/special endpoint outcomes are:

- withheld or anonymous external endpoint: `resolution_required`, because a legitimate human interaction may require manual attribution;
- valid E.164 endpoint configured as a shared/business external number: `resolution_required`, because it may represent several people and must never become one person's canonical identity;
- configured carrier short code, alphanumeric sender ID, or known system transport endpoint: `excluded`, because it is explicitly non-person traffic;
- unconfigured short code or alphanumeric sender ID: `rejected`, because it cannot satisfy the canonical phone contract safely;
- malformed phone value: `rejected`;
- multiple distinct external actors: `resolution_required`;
- endpoint-derived direction contradicting provider-declared direction: `rejected`.

These outcomes are part of the adapter contract and must not vary by provider implementation.

## Direction and Actor Selection

Owned business numbers are injected configuration, not discovered from message content.

- Inbound: exactly one external E.164 endpoint communicates to one or more configured owned lines. The external endpoint is the actor.
- Outbound: configured owned line(s) communicate to exactly one external E.164 endpoint. The external endpoint is the actor.
- Provider-declared direction must agree with endpoint classification when supplied.
- Internal-only traffic is excluded.
- No owned endpoint or both sides external is `rejected`; both sides owned is `excluded`; contradictory direction is `rejected`; multiple distinct external participants is `resolution_required`. The adapter never chooses an arbitrary actor.
- Group SMS/iMessage and conference calls are outside the POC because CRM-02 currently requires one canonical person actor.

## Call Metadata Versus Message Content

- The POC emits only a completed-call interaction: channel `call`, event type `call_completed`, direction, occurrence time, and non-negative duration when known. Ringing/answered/ended provider lifecycle callbacks must be collapsed by a future provider connector into one stable completed event before this adapter.
- Allowlisted call metadata may include provider conversation/call correlation ID, completion/disposition category, and whether voicemail was indicated. It must not contain recordings, transcripts, signed recording URLs, tokens, raw SIP headers, or provider payloads.
- SMS emits `sms_received` or `sms_sent`; iMessage emits `imessage_received` or `imessage_sent`.
- Provider-extracted plain text is placed in the bounded canonical interaction summary, not duplicated in raw metadata. The POC validates a 4,000-character ceiling and does not parse rich message archives.
- Attachments, reactions, edits, read receipts, delivery receipts, and message threading UI are deferred.

## Unknown Callers and Creation Policy

- Exact existing phone ownership always wins.
- A normalized external phone from a trusted provider envelope remains `transport_observed` unless the connector also supplies a separately approved ownership assurance. Envelope or webhook trust alone never becomes CRM-03 creation evidence.
- Auto-creation is allowed only when the external actor meets the Transport Assurance Policy, the applicable owned business line has one explicit configured CRM role, and the event has exactly one valid external E.164 actor. Multiple applicable lines must all specify the same role.
- A display name may seed presentation through CRM-03 but never changes eligibility or matching.
- Unknown transport-observed, withheld, shared, or ambiguous endpoints return `resolution_required`; malformed endpoints are rejected and configured non-person system transports are excluded. None creates a person.
- No interaction may be written until a person is safely resolved or created. A future live provider must therefore add a reviewed durable receipt/cursor before acknowledging unresolved delivery.

## Multiple, Shared, and Business Numbers

- Multiple distinct hints that all already resolve to the same active person are compatible with CRM-02/03, but a single call/message actor should normally contribute only its one authoritative endpoint.
- Conflicting hints owned by different people stop processing.
- A new secondary number accompanying an existing owned number remains unclaimed under CRM-03 race/recovery rules; it is not silently attached.
- The current uniqueness constraint cannot model a household, business switchboard, or genuinely shared phone number as identity for multiple people. Such numbers must be configured as non-person/shared endpoints and must not auto-create or auto-link people.
- Modeling organizations, households, shared identities, forwarding, number reassignment, and person merge is explicitly deferred.

## Source Identity and Idempotency

- Normalize provider and account namespace using the bounded collision-safe token grammar already approved for CRM-05.
- Use `source.system = communications:<provider>:<accountNamespace>`.
- Provider event IDs are opaque and case-preserving because some providers use case-sensitive identifiers. After NFKC normalization and trimming, require 1–512 visible ASCII characters, forbid whitespace/control characters, URL-shaped values, secrets, and the delimiter `:`. Use `source.externalId = <transport>:<providerEventId>`, where transport is exactly `call`, `sms`, or `imessage`. The validated transport prefix supplies the namespace without parsing or lowercasing the opaque ID and prevents collisions if a provider reuses identifier spaces across transport resources.
- Provider event ID, not conversation/thread ID or phone number, is authoritative for idempotency.
- Conversation/thread/call correlation identifiers are metadata only. Apply the same opaque, non-secret, non-URL, case-preserving grammar with a 512-character maximum before admitting them to metadata; empty values are omitted rather than stored.
- CRM-01's unique `(source_system, source_external_id)` index remains the concurrent persistence backstop; CRM-02 checks duplicates before person/context resolution.

Message content is NFKC-normalized, line endings become `\n`, leading/trailing whitespace is trimmed, NUL/control characters other than tab/newline are rejected, and the result must be 1–4,000 Unicode code points before it may become the canonical summary. It is never used in source identity or duplicated into metadata.

## Property, Deal, and Intents

- Only trusted adapter context may provide exact property UUID, slug, recognized CulebraLuxe URL, or deal UUID.
- Message text, call notes, contact name, phone history, AI, and provider labels never select property/deal.
- CRM-02 exact agreement and deal ownership checks remain unchanged.
- Intents remain advisory. No task, interaction, `property_interest`, or workflow write is part of the preliminary POC.

## Privacy and Metadata

- Construct `source_metadata` from an explicit allowlist; apply CRM-02 recursive secret rejection and the 32 KB serialized ceiling.
- Never retain auth headers, API keys, cookies, provider credentials, device backups, address books, raw webhook bodies, recordings, transcripts, attachment bytes, or signed URLs.
- Provider configuration and owned/shared-line classification are injected and not copied into canonical metadata.
- No raw metadata logging.

## Application Boundaries

1. A future provider connector authenticates delivery and maps a provider payload into the neutral event.
2. A pure communications adapter validates transport facts, endpoints, direction, exclusions, content bounds, and allowlisted metadata, then emits an `InboundEvent` or explicit excluded/resolution-required/rejected result.
3. An injected coordinator checks source idempotency first, then composes CRM-02 exact resolution and CRM-03 safe creation with explicit owned-line role policy.
4. The fixture POC returns canonical interaction input and advisory intents only. Canonical persistence and provider acknowledgement are separate future boundaries.

Source adapters never write CRM tables directly.

## Preliminary Schema Decision

No schema change is needed for the fixture-only POC. Existing phone identity, interaction channels, direction, duration, event type, source identity, metadata, and CRM-03 creation transaction cover the contract.

Before live ingestion, architecture review must decide on a provider-specific durable delivery receipt/cursor and retry boundary. It must not reuse the website receipt or become a generic staging/ODS table. A future need to model shared phone ownership would require a separate domain decision rather than weakening `person_identity` uniqueness.

Canonical call or message persistence remains unauthorized until CulebraLuxe approves explicit consent, content-retention, deletion/export, and call-recording/transcription policies for the relevant transport and jurisdiction. The fixture POC may produce canonical inputs in memory only; it must not call CRM-01 writes even when identity resolution succeeds.

## Smallest Later Fixture POC

Prefer only:

- `lib/crm-communications-types.ts`;
- `lib/crm-communications-normalization.ts`;
- `lib/crm-communications-intake.ts`;
- `scripts/verify-crm-communications-intake.mjs`.

Use injected repositories that throw on unexpected access. Perform zero provider, Apple, environment, route, UI, and Neon operations.

Required fixtures should cover:

- strict E.164 acceptance/rejection and shared phone identity across call/SMS/iMessage;
- deterministic inbound/outbound actor selection and provider-direction conflicts;
- exact deterministic outcomes for internal-only, withheld/anonymous, configured shared/business external numbers, configured system short codes/alphanumeric senders, unconfigured non-E.164 senders, malformed phones, multiple external actors, and contradictory direction;
- exact existing resolution regardless of creation assurance; transport-observed unknown inbound/outbound non-creation; separately approved ownership-assured plus explicit-role creation; missing/conflicting-role non-creation; authenticated outbound actor context; archived/conflicting owner behavior;
- call duration/disposition allowlist versus prohibited recording/transcript data;
- bounded SMS/iMessage summary without duplication in metadata;
- provider/event/correlation identifier normalization, length, case preservation, delimiter/URL/secret rejection, and transport-qualified source external IDs;
- message NFKC/line-ending/control-character normalization and Unicode code-point bound;
- same provider event retry, conversation ID non-identity, and duplicate short-circuit before resolution;
- exact-only property/deal behavior and advisory-only intents;
- recursive secret rejection, metadata size ceiling, and zero arbitrary provider payload;
- zero person/task/interaction/interest writes, provider calls, or Neon access;
- unchanged CRM-01 through CRM-05 fixture behavior.

## Preliminary Risks and Open Review Questions

- Confirm whether `call_completed` is sufficient or whether business reporting genuinely needs separately canonicalized missed/voicemail outcomes; do not mirror provider lifecycle callbacks as interactions.
- Confirm which owned lines, if any, carry explicit buyer/seller creation roles.
- Decide how configured shared/business external numbers are maintained before live intake.
- Define durable provider acknowledgement/retry semantics before any live connector.
- Review message-content retention, consent, call-recording policy, and number-reassignment handling before persistence.
- Apple iMessage access and provider feasibility are deliberately unresolved and cannot influence the neutral CRM contract.
