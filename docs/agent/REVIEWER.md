# Reviewer Checklist

## CRM-07 — WhatsApp Intake

Status: Architecture only; awaiting canonical-channel decision. No implementation exists or is authorized.

Latest result: Not reviewed.

## Scope and Boundary

- Provider webhook/SDK -> neutral WhatsApp event -> pure adapter -> canonical intake boundary is explicit.
- No Meta/provider coupling, live call, credential/environment, route, receipt/cursor, acknowledgement, schema, database write, UI, send/reply, workflow, notification, or media fetch.
- No WhatsApp identity kind; strict canonical phone identity is reused.
- WhatsApp is not mislabeled as SMS/iMessage and no generic message channel is invented.

## Canonical Channel Gate

- The absent canonical `whatsapp` channel is identified explicitly.
- Architecture does not silently authorize a schema change.
- The hypothetical Builder remains blocked until a separately reviewed canonical-channel decision.
- Any future migration is narrow and changes only the existing channel contract.

## Direction, Identity, and Assurance

- Injected configuration alone owns the internal business number and endpoint classification.
- Direction is endpoint-derived and agrees with any trusted provider direction.
- Person endpoints are strict E.164 and resolve through `phone` across all communication transports.
- Names, provider contacts, profile labels, content, templates, filenames, history, and AI never identify a person.
- Webhook signature/provider delivery is transport assurance only and cannot authorize unknown-person creation.
- Exact active owners win; archived/conflicting/shared/ambiguous actors never attach or create silently.

## Idempotency and Event Semantics

- Provider message ID, qualified as `whatsapp:<id>`, is authoritative for source idempotency.
- Duplicate webhook delivery short-circuits before person/context work.
- Conversation ID, phone, timestamp, content, and template do not determine deduplication.
- Delivery/read/status, reactions, edits, revocations, retries, and webhook batching do not create duplicate interactions.
- System events exclude; inbound template classification rejects; accepted message classes are deterministic.

## Content, Metadata, and Attachments

- Text normalization/bounds match the existing CRM communications contract.
- Metadata is closed and contains only transport, content class, optional opaque template ID, and bounded sanitized attachment descriptors.
- No raw webhook, headers, signature, token, credential, URL, arbitrary payload, or metadata logging.
- Attachment descriptors use strict runtime types/count/size/serialization bounds and never trigger network fetch, byte storage, `media`, or `property_media` writes.
- Privacy/retention and provider-media expiry are explicitly deferred behind a live-integration review.

## Context and Side Effects

- Property/deal resolution is exact trusted context only.
- No free-text, caption, filename, template, subject, fuzzy, or AI context inference.
- No requested-action inference; advisory intents remain empty.
- No person, interaction, task, interest, receipt, media, schema, provider, or environment side effects are reachable.

## Review Result Format

Return `PASS` or `CHANGES REQUIRED`, followed by concise findings grouped as Critical, High, Medium, and Low. Architecture can pass only if the decision gate remains explicit and implementation remains unauthorized.
