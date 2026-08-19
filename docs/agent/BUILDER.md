# Builder Work Order

## CRM-07 — WhatsApp Intake

Status: Architecture-only hypothetical work order; awaiting the canonical-channel decision. Do not implement.

Follow `AGENTS.md` and `docs/agent/CURRENT.md`. This document bounds a future fixture POC so architecture can evaluate scope. It does not authorize code, schema, migration, route, provider, environment, database, or UI changes.

## Decision Required Before Builder Starts

The current interaction channel contract has no `whatsapp` value. Do not map WhatsApp to SMS/iMessage or invent a generic message channel. Builder remains blocked until a separately reviewed decision authorizes the canonical representation and, if needed, a narrow migration.

## Hypothetical Allowed Files

After that decision only, prefer:

- `lib/crm-whatsapp-types.ts`
- `lib/crm-whatsapp-normalization.ts`
- `lib/crm-whatsapp-intake.ts`
- `scripts/verify-crm-whatsapp-intake.mjs`

An approved future channel change may narrowly extend the neutral interaction type and add one migration. No other CRM-06 code, DB repository, route, UI, package, lockfile, environment file, receipt, provider configuration, or persistence path should change.

## Hypothetical Adapter Contract

1. Translate only a neutral provider event; import no Meta/provider SDK or webhook types.
2. Reuse CRM-06 source-token, opaque-ID, endpoint-classification, strict-E.164, assurance, content, metadata-sanitization, duplicate-first, and exact-context semantics through a clean shared extraction if justified; do not copy divergent logic.
3. Treat the configured internal business number as configuration authority. Events cannot self-classify endpoints or creation roles.
4. Resolve WhatsApp actors through canonical `phone`; never create a WhatsApp identity kind.
5. Use provider message ID as source idempotency identity and ignore delivery/read/status callbacks as interactions.
6. Accept only the closed content classes and message/attachment rules in `CURRENT.md`; exclude system events and reject contradictory inbound templates.
7. Store no raw webhook data, secrets, URLs, bytes, contact payloads, or arbitrary metadata.
8. Preserve exact trusted property/deal context. Perform no parsing, fuzzy matching, AI linking, task creation, interest persistence, or canonical writes.
9. Keep all repositories injected and fixture-only. No production/Neon/provider default may be reachable.

## Hypothetical Required Fixtures

After the channel decision, verify at minimum:

- inbound/outbound direction from configured business endpoints and provider-direction agreement;
- strict E.164 and identical phone resolution across WhatsApp/SMS/iMessage/call;
- no WhatsApp identity creation;
- signed-webhook/transport-observed assurance cannot create an unknown person;
- exact existing active owner resolution, archived/conflicting behavior, and unknown resolution-required behavior;
- shared/withheld/multiple actors, internal/system traffic, malformed endpoints, and ambiguous groups;
- provider/account/message ID validation, `whatsapp:<providerMessageId>`, and case-preserving provider ID;
- duplicate delivery short-circuits before identity/context work;
- delivery/read/status/reaction/edit callbacks do not create interactions;
- free-form/template/service/system matrix, outbound-only templates, text normalization/bounds, and contentless rejection;
- attachment descriptor runtime types, count/size bounds, URL rejection, metadata ceiling, and no byte/network/media persistence;
- exact property/deal agreement and no content/name/template/AI linking;
- closed metadata keys, recursive secret rejection, no raw webhook logging, and empty advisory intents;
- zero Neon, provider, person, interaction, task, property-interest, receipt, or media writes;
- all earlier CRM fixture suites remain green.

## Hypothetical Verification

Run the new fixture first, then CRM-01 through CRM-06 fixtures, followed by:

```sh
git diff --check
pnpm exec next build --webpack
```

Restore generated files and report exact changes and side-effect absence. Do not commit or push.
