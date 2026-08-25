# Marlowe Gmail Relationship Census — Read-only acquisition report

Date: 2026-08-24  
Status: **Blocked by connector enumeration limitation; full-history census not claimed**

## Executive result

The connected legacy mailbox and Production identity tables were accessed read-only. A complete, deterministic metadata census could not be produced with the available Gmail connector. The connector's label inventory reports approximately 161,832 Inbox messages and 23,252 Sent messages, but the safe metadata search projection is both incomplete and nondeterministic. A bounded January 2024 query returned 1,577 message IDs while the corresponding safe metadata query returned only 1,102 metadata records. Repeating the same early-history bounded sweep returned 7,711 records after an earlier run returned 5,814.

The body-capable bulk-read operation was not used because it would violate the metadata-only boundary. Acquisition therefore stopped under the work order's explicit connector/API-limitation condition. The accompanying private CSV is a **partial evidence set**, not a full mailbox census.

## Account and time coverage proven

- Connected account identity was verified using its opaque Google account identifier. No mailbox address is repeated in this report.
- Mailbox label inventory: approximately 161,832 Inbox messages and 23,252 Sent messages.
- Known mailbox history: 2011 through 2026.
- Partial CSV acquisition attempted 40 monthly shards from 2011-01 through 2014-04.
- Successfully reduced metadata timestamps span 2011-06-26 through 2013-12-31.
- 126 metadata pages were checkpointed in the partial run.
- A separate diagnostic sweep exhausted 188 monthly query cursors through 2026-08, but its returned metadata population was inconsistent with both label counts and repeat queries; it is not represented as full coverage.

## Partial message-level balance

| Measure | Count |
| --- | ---: |
| Metadata messages examined and deduplicated | 7,696 |
| Inbound | 6,778 |
| Outbound | 835 |
| Internal-only | 82 |
| No usable external identity | 0 |
| Unknown direction | 1 |
| Acquisition/transport errors | 16 |

Balance proof: 6,778 + 835 + 82 + 0 + 1 = 7,696 examined messages. Errors are reported separately and are not counted as examined.

Direction used only two evidence-backed owned mailbox identities and envelope roles. Gmail labels were supporting evidence only. Message IDs were globally deduplicated.

## Partial correspondent census

| Evidence measure | Count |
| --- | ---: |
| Unique normalized external identities | 2,018 |
| Two-way | 115 |
| Account-owner-initiated outbound-only | 60 |
| Inbound-only | 1,843 |
| Automated or bulk evidence | 1,140 |
| Organization/service-domain evidence | 1,164 |

These categories overlap where their meanings overlap; they are not asserted to sum to the identity total. Age was never used as a rejection or quality signal.

## Exact Production reconciliation

Production Neon was queried with SELECT statements only. The current Apple load table exists. Exact normalized-email comparison examined four canonical email identity rows and 248 Apple-load email identity rows.

| Partial CSV reconciliation outcome | Count |
| --- | ---: |
| Canonical exact match | 0 |
| Apple load exact match | 0 |
| Canonical/Apple conflict | 0 |
| Unmatched partial evidence identities | 2,018 |

These results apply only to the partial Gmail evidence set and must not be generalized to the unenumerated mailbox.

## Persisted private contract

The private CSV contains one row per normalized external email with first/last contact timestamps, inbound/outbound timestamps and counts, CC/BCC participation count, distinct-thread count, last direction, Gmail message/thread pointers, deterministic relationship evidence flags, domain/service evidence, exact identity-match IDs where present, reconciliation outcome, and acquisition limitations. It contains no body, HTML, quoted chain, signature, attachment bytes, embedded image, subject, snippet, or model reasoning.

The stable pointer format is `gmail://<opaque-account-token>/messages/<gmail-message-id>`.

## Classification method

- Two-way, outbound-only, and inbound-only evidence derives from configured owned identities plus From/To/CC/BCC roles.
- Multi-recipient/bulk evidence derives from recipient cardinality and Gmail category labels.
- Automated/service evidence uses deterministic sender-local-part patterns and Promotions/Social/Forums category evidence.
- Organization evidence uses non-consumer email domains; it is evidence, not a canonical business-role assertion.
- Airbnb/property-operations evidence uses deterministic source-domain evidence only.
- No fuzzy matching, alias inference, personal/client/prospect labels, or canonical promotion was performed.

## Checkpoint and resume mechanism

The local private checkpoint records phase, current monthly shard, completed shards, next-page cursor, page count, deduplicated Gmail message IDs, aggregate correspondent state, earliest/latest timestamps, balance counters, and update timestamp. The query strategy uses calendar shards to avoid the connector's approximate 5,000-result per-query ceiling.

The checkpoint is sufficient to resume the partial reducer, but it cannot repair records the connector omits nondeterministically.

## Hard connector limitations

1. Safe metadata search does not enumerate all IDs returned by the ID-only search for the same bounded query.
2. Repeated identical bounded metadata sweeps return materially different totals.
3. Bulk message read is body-capable and therefore outside the authorized metadata-only boundary.
4. Safe search projection omits RFC Reply-To, Internet Message-ID/references, and attachment descriptors even though other body-capable operations may expose them.
5. A full-history claim is impossible until a deterministic metadata-only batch-read/export capability exists.

## Smallest recommended Cline implementation slice

Do not import historical messages. First add a resumable metadata adapter that accepts a deterministic Gmail message-ID manifest and fetches **metadata format only** in bounded batches. It should reject any response containing body parts, snippets, raw MIME, or attachment bytes; persist a checkpoint and payload/header fingerprint; aggregate directly into source-neutral Gmail-derived `l_person` / `l_person_identity` evidence; retain first/last inbound/outbound timestamps and counts plus the Gmail pointer; and exact-match against canonical and Apple identities. Expose only a reviewable source/last-contact view. Individual thread retrieval or summarization must remain explicit and separately authorized.

The implementation gate is a connector capability that guarantees: complete ID pagination, batch metadata-only reads, stable account identity, and no read-state mutation. Once available, rerun all 188 historical month shards and require message-ID balance against the manifest before calling the CSV a census.

## Safety confirmation

- No Gmail send, delete, archive, label, mark-read, draft, forward, or batch-modify operation was used.
- No message body, snippet, raw MIME, attachment, or inline image was intentionally read or persisted.
- No attachment was downloaded.
- Neon received read-only catalog and exact-match SELECT queries only.
- No codebase, Git branch, commit, pull request, push, deployment, canonical Person, Interaction, Deal, Task, or Attention record was changed.
- This report contains no private email address, correspondent name, subject, message text, or sample identity.
