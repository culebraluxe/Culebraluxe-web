# Reviewer Checklist

## CRM-05 — Email Intake

Status: Awaiting architecture review; no implementation exists.

Latest result: Not reviewed.

## Architecture and Scope

- Provider connector -> neutral message -> pure adapter -> `InboundEvent` -> CRM-02/03 remains explicit.
- No Gmail coupling, live provider access, credentials, route/UI, send/reply, cursor, acknowledgement, workflow, notification, or persistence.
- No schema/migration, DB-write repository, dependency, or reuse of website receipt.
- POC stops at canonical interaction input/advisory intents using injected fakes.

## Identity and Direction

- Conservative email normalization preserves plus tags and dots.
- Message ID is event identity; thread ID is correlation metadata only; source is provider/account scoped.
- Provider/account tokens use the documented lowercase 1–64 character grammar; delimiters/config variants cannot collide.
- Inbound actor is one external sender; outbound actor is one exact external recipient.
- Missing/multiple sender mailboxes are deterministically rejected before identity work.
- Internal-only/system mail never creates CRM people.
- Multiple external outbound recipients, ambiguous role, or ambiguous envelope never chooses arbitrarily.
- Names, subject/body, AI, aliases, and recipient order are not canonical identity evidence.
- Existing exact person wins. Creation requires every applicable internal mailbox to declare the same one explicit role; missing/conflicting roles require resolution and never choose arbitrarily.

## Exclusions

- Bounce/delivery, auto-submitted, list/bulk, provider-system, and configured no-reply exclusions use exact transport evidence.
- No broad substring checks cause false positives.
- Exclusion occurs before person/context work and yields no canonical intents.

## Content, Privacy, and Attachments

- Event type is exactly `email_received` inbound and `email_sent` outbound.
- Neutral `plainText` is already clean; the provider connector owns extraction/history removal and the POC only validates/bounds it.
- Subject/plain-text limits enforced; no raw MIME/full HTML/entire quoted history.
- Recursive secret rejection and 32 KB metadata ceiling reused.
- Emitted metadata is built from the exact documented allowlist; sanitizer-safe arbitrary provider fields are not admitted.
- Bcc/credentials not emitted or logged.
- Thread/reply/forward/participants remain bounded metadata, not canonical links.
- Attachment provider IDs obey the opaque non-URL grammar; descriptors contain no bytes, URLs, downloads, media inserts, or invented relationship.
- Retention/minimization intent documented without a job.

## Context and Idempotency

- Duplicate source identity short-circuits before identity/property/deal work.
- Same thread/different messages remain distinct.
- Property/deal context is exact/trusted only; no free-text/fuzzy/AI linking.
- Deal/person/property consistency remains CRM-02 behavior.
- Unknown sender is resolution_required unless explicit CRM-03 policy authorizes creation.
- No provider acknowledgement occurs without a future durable boundary.

## Verification

- Fixtures verify behavior/call order with zero Neon/provider access; unexpected calls fail.
- Fixtures cover event types, malformed sender cardinality, token grammar/collision rejection, metadata allowlisting, opaque attachment IDs, and all mailbox-role agreement cases.
- No task, interest, interaction, or DB person write is reachable (mocked CRM-03 behavior only).
- CRM-01/02/03/04 suites remain green.
- No package/lockfile/generated/environment/schema/route/UI changes.
- `git diff --check` and webpack build pass.

## Latest Findings

Awaiting architecture review.
