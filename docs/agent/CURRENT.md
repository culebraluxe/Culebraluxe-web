# Current Story

## CRM-05 — Email Intake

Status: Architecture ready for review; implementation has not started.

## Completed Foundations

- CRM-01 provides append-only, source-idempotent interactions and explicit tasks.
- CRM-02 provides the source-neutral `InboundEvent` contract, conservative identity normalization, exact person/property/deal resolution, and advisory intents.
- CRM-03 provides explicitly authorized atomic person/identity creation with existing-person-wins race recovery.
- CRM-04 provides a website-specific durable receipt and canonical website intake coordinator. Its proposed migration remains unexecuted.

## Goal and Boundary

Design the smallest provider-neutral email adapter contract that can translate a provider message into CRM-02 without coupling CRM core to Gmail or performing live ingestion.

CRM-05 is not a mailbox connector. It adds no OAuth, polling, webhook, cursor, send/reply, attachment download, route, UI, task creation, or database write. The POC is pure/injected and fixture-only.

## Provider-Neutral Contract

An `EmailProviderMessage` is transport input to an `EmailAdapter`. It contains:

- provider name and a stable non-secret account namespace, each normalized to a restricted source token;
- provider message ID and optional provider thread ID;
- provider occurrence timestamp;
- exactly one sender mailbox, `to`/`cc`/`bcc` recipients, and optional reply-to addresses;
- provider-declared direction when trusted, otherwise enough envelope context to derive it from configured internal mailboxes;
- subject and provider-extracted clean plain-text body/summary source;
- normalized reply/forward metadata where explicitly supplied by the provider;
- attachment descriptors containing stable provider attachment references, filename, MIME type, and size only;
- optional exact property UUID/slug/recognized CulebraLuxe URL and exact deal UUID supplied by trusted adapter context;
- explicit normalized transport fields only; arbitrary provider metadata is not accepted even when JSON-safe.

Provider-specific SDK objects must be mapped into this neutral contract before reaching CRM code.

## Message and Thread Identity

- The provider message ID is the event identity. Every distinct message becomes at most one interaction.
- Thread ID is correlation metadata only. It must never be used as the interaction idempotency key because one thread contains multiple messages.
- Source identity is deterministic and account-scoped: `source.system = email:<provider>:<accountNamespace>` and `source.externalId = <providerMessageId>`.
- Provider and account namespace are NFKC-normalized, trimmed, lowercased, and must each match `^[a-z0-9](?:[a-z0-9_-]{0,62}[a-z0-9])?$` (1–64 characters). Colons, whitespace, and empty tokens are rejected, preventing source delimiter collisions and configuration drift.
- `accountNamespace` is a configured opaque stable identifier, not a token or mailbox password. It prevents collisions when provider message IDs are only account-scoped.
- Duplicate/retried delivery short-circuits through existing CRM-01/02 `(source_system, source_external_id)` protection before person or context work.

## Direction and Participants

- Internal mailbox addresses are injected configuration and normalized with CRM-02's conservative email rule.
- Exactly one external sender with only internal recipients is `inbound`.
- An internal sender with at least one external recipient is `outbound`; exactly one normalized external recipient may become the actor. Zero is internal/system and excluded; multiple distinct external recipients is `resolution_required`, not an arbitrary choice.
- External sender to external recipients, or a provider direction that conflicts with the normalized envelope, is rejected by the caller boundary.
- `to`, `cc`, `bcc`, and reply-to are transport metadata. They are never alternate canonical actors merely because they appear on the message.
- Names/display labels are hints only and never identity keys.
- A missing sender or any provider input containing more than one sender mailbox is deterministically rejected before identity work.

## Identity and Role Policy

- Email addresses use existing conservative `trim + lowercase` normalization. Dots and plus tags remain unchanged.
- Inbound external sender is one `provider_asserted` email hint only when the adapter guarantees it came from the provider-parsed envelope. This asserts the envelope sender, not legal identity.
- Outbound external recipient uses `provider_asserted` only from trusted sent-mail envelope data.
- Existing exact person identity always wins.
- Each configured internal mailbox may carry an explicit creation role. Creation is allowed only when every applicable internal mailbox for the message is role-configured and all applicable roles resolve to the same single value. For inbound mail, applicable mailboxes are the internal recipients; for outbound mail, they are the internal sender mailboxes. Missing or conflicting applicable roles returns `resolution_required` with no creation. Existing-person exact resolution is not blocked by missing creation policy.
- Role is never inferred from subject/body, display name, AI, recipient aliases, or provider labels.
- Internal addresses, system mailboxes, mailing lists, automated senders, and bounce/notification senders are never auto-created as people.

## Exclusions Before CRM Resolution

The pure adapter classifies and excludes messages before CRM-02/03 when deterministic transport evidence identifies:

- any configured internal-only message;
- provider/system notification categories;
- delivery status/bounce reports (`multipart/report`, delivery-status headers, null return path, or configured mailer-daemon/postmaster address);
- auto-replies or bulk/list mail through exact standard headers such as `Auto-Submitted` (other than `no`), `List-Id`, or provider-supplied categories;
- configured no-reply/system sender addresses.

Header names/values are normalized conservatively. Broad substring matching is forbidden. A message is not excluded merely because its subject contains words such as “automatic” or “newsletter.” Exclusion yields an explicit reason and no person/interaction/intents.

## Content, Threading, and Attachments

- Canonical event type is deterministic: inbound emits `email_received`; outbound emits `email_sent`.
- Interaction `title` receives a normalized, bounded subject (maximum 500 characters).
- `EmailProviderMessage.plainText` must already be markup-free and quoted-history-free. The provider connector owns deterministic MIME/HTML extraction and quoted-history removal. The pure POC validates the field contract and bounds it to 4,000 characters; it does not guess stripping rules.
- Existing CRM-02 sanitized metadata remains limited to 32 KB.
- Reply/forward state is recorded only from explicit provider headers/fields (`inReplyToMessageId`, reference IDs, explicit forward classification). Subject prefixes alone are not authoritative.
- `source_metadata` is constructed from an explicit allowlist only: `threadId`, `inReplyToMessageId`, `referenceMessageIds`, `isForward`, `toEmails`, `ccEmails`, `replyToEmails`, and `attachments`. Arbitrary provider payload/headers are discarded rather than passed through the sanitizer.
- Attachment descriptors contain only `providerAttachmentId`, `filename`, `mimeType`, and `sizeBytes`. The provider attachment ID must be an opaque non-URL identifier matching `^[A-Za-z0-9._~+=-]{1,512}$`. Descriptors contain no bytes, URLs, access tokens, or raw content. A future attachment story may import bytes into `media` and persist stable media IDs through an explicitly reviewed relationship. CRM-05 neither invents that relationship nor downloads attachments.

## Property and Deal Context

- Property/deal resolution remains the unchanged CRM-02 exact-only path.
- Only trusted adapter context may supply property UUID, slug, recognized CulebraLuxe property URL, or deal UUID.
- Subject, body, signature, thread title, attachment name, AI output, and sender history are never used to choose property/deal.
- Multiple exact hints must agree; deal must belong to resolved person and property.
- Email adapter intents are advisory. No task or `property_interest` write is part of CRM-05.

## Unknown Senders and Durable Intake

- The pure POC returns `resolution_required` when CRM-02/03 cannot safely resolve/create the external actor.
- It never acknowledges or deletes provider messages and has no live provider cursor, so fixture execution cannot lose mail.
- A future live connector requires an explicitly reviewed durable receipt/cursor boundary before acknowledging delivery. It must hold unresolved/retry state without duplicating canonical CRM data.
- CRM-04's website receipt is not reused for email, and CRM-05 does not prematurely add a generic ODS/staging table.

## Privacy and Retention

- Store the minimum canonical interaction summary and bounded audit metadata.
- Never log or persist OAuth tokens, authorization headers, cookies, raw MIME, full HTML, attachment bytes, signed URLs, or provider SDK objects.
- Bcc values may be used transiently for direction but are omitted from retained metadata.
- Full bodies remain provider-owned. Canonical summaries follow interaction retention; transport metadata should be eligible for minimization after 24 months in a future retention story.
- Excluded and unresolved fixture inputs are not persisted in CRM-05.

## Application Boundaries

1. Future provider connector authenticates and maps SDK payload to `EmailProviderMessage`.
2. Pure adapter validates the already-clean plain-text contract, normalizes, classifies exclusions/direction, builds allowlisted metadata, bounds content, and emits `excluded`, `resolution_required`, `rejected`, or an `InboundEvent`.
3. Email intake coordinator checks source idempotency, then uses injected CRM-02/03 repositories and explicit role/creation policy.
4. Canonical persistence remains a later application boundary using CRM-01; this POC stops at canonical interaction input/result and performs no writes.

No Gmail concepts appear in CRM repositories or domain types beyond opaque provider metadata.

## Schema Decision

No schema change is required for the bounded CRM-05 POC.

Existing `interaction` fields support channel, direction, subject, summary, source identity, and bounded source metadata. Existing `person_identity` supports canonical email. A live unresolved-email receipt, mailbox cursor, or interaction-to-media relationship before a provider lifecycle exists would be speculative and risks a generic staging system.

Architecture review must revisit schema before any live connector acknowledges messages or persists attachments.

## Smallest Fixture-Only POC

- neutral email types;
- pure email normalization/classification adapter;
- injected coordinator composing CRM-02/03 without persistence;
- fixtures for inbound/outbound, exclusions, idempotency short-circuit, identity policy, exact context, content bounds, metadata privacy, and attachments-as-descriptors;
- zero provider calls and zero Neon access.

## Deferred / Risks

- Durable provider cursor/receipt and retry ownership.
- Live Gmail/provider adapter, OAuth, webhook/polling, rate limits, and acknowledgements.
- Sending/replying and notifications.
- Attachment download, malware scanning, media relationship, and access policy.
- Shared/business mailbox human assignment, group conversations, and role adjudication.
- Manual resolution UI, retention jobs, legal hold, deletion/export policy.
- Email threading UI and AI summarization/link suggestions.
