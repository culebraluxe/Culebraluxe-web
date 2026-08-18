# Builder Work Order

## CRM-05 — Email Intake

Status: Ready only after architecture review passes.

Follow `AGENTS.md` and `docs/agent/CURRENT.md`. Implement only the provider-neutral, fixture-only email adapter POC. Do not connect to a provider or persist anything.

## Files and Boundaries

Add the smallest cohesive modules, preferably:

- `lib/crm-email-types.ts`
- `lib/crm-email-normalization.ts`
- `lib/crm-email-intake.ts`
- `scripts/verify-crm-email-intake.mjs`

Extend an existing neutral CRM type only if compilation requires it. Do not change schema/migrations, DB repositories, routes, UI, packages, environment files, CRM-04, or provider configuration.

## Required Contract

Define provider-neutral inputs/results for provider/account namespace, message/thread identity, timestamp, exactly one sender plus recipient collections, optional trusted direction, subject/provider-cleaned plain text, explicit reply/reference/forward fields, attachment descriptors, trusted exact context hints, and internal mailbox configuration with explicit per-mailbox creation role.

Attachment descriptors contain only provider reference, filename, MIME type, and size. Do not import Gmail SDK types or leak provider-specific fields into CRM repositories.

## Adapter Rules

1. NFKC-normalize, trim, and lowercase provider/account namespace. Require each to match `^[a-z0-9](?:[a-z0-9_-]{0,62}[a-z0-9])?$` (1–64 characters). Validate non-empty message ID and timestamp.
2. Construct source identity exactly as `email:<provider>:<accountNamespace>` plus provider message ID; delimiter-bearing/invalid source tokens are rejected.
3. Normalize all addresses with existing conservative email normalization; preserve dots/plus tags; deduplicate in stable normalized input order.
4. Determine direction/actor deterministically: external sender to only internal recipients is inbound; internal sender to exactly one external recipient is outbound; internal-only is excluded; multiple external outbound recipients is resolution_required; contradictory/ambiguous envelope or trusted direction is rejected.
5. Require exactly one sender mailbox. Missing or multiple senders are rejected before identity work. Names are display hints only.
6. Apply exact transport exclusions before person resolution: internal/system, configured system/no-reply senders, provider system category, bounce/delivery status, `Auto-Submitted` other than `no`, and `List-Id`/bulk-list evidence. No subject substring heuristics.
7. Map canonical event type exactly: inbound -> `email_received`; outbound -> `email_sent`.
8. Require `plainText` to be provider-extracted, markup-free, and quoted-history-free. Validate/bound subject to 500 and plain text to 4,000; do not implement heuristic stripping.
9. Construct `source_metadata` only from the explicit allowlist: `threadId`, `inReplyToMessageId`, `referenceMessageIds`, `isForward`, `toEmails`, `ccEmails`, `replyToEmails`, and `attachments`. Discard arbitrary provider fields even if sanitizer-safe; then apply recursive secret checking and 32 KB ceiling. Omit bcc.
10. Attachment descriptors contain only provider ID/name/MIME/size. Require provider attachment IDs to match `^[A-Za-z0-9._~+=-]{1,512}$`; reject URLs and bytes.
11. Emit exact property/deal hints only from trusted adapter context. Never parse subject/body/attachments or use fuzzy/AI linking.

## Coordinator Rules

- Use injected repositories; defaulting to Neon is forbidden in this POC.
- Check source identity before person/context resolution; duplicate returns existing interaction without later repository work.
- Feed accepted messages through CRM-02 normalization/resolution and CRM-03 resolve/create policy.
- Exact existing person wins.
- For creation, collect all applicable configured internal mailboxes (inbound internal recipients; outbound internal senders). Every applicable mailbox must declare a role and all roles must be identical. Only that single explicit role may reach CRM-03 creation policy.
- Missing/conflicting applicable mailbox roles return resolution_required with no creation; internal/system actors are never created. Existing exact people still resolve without a creation role.
- Return canonical interaction input/advisory intents only. Do not call interaction/task/interest writes.
- Do not acknowledge provider delivery or model cursor/retry state.

## Required Fixture Verification

- inbound/outbound direction and actor mapping, with `email_received`/`email_sent` respectively;
- missing sender and multiple sender mailboxes rejected before identity work;
- internal-only exclusion and multiple-external outbound resolution_required;
- conflicting trusted direction rejected;
- plus tags/dots preserved;
- message ID, not thread ID, is idempotency identity; same thread/different messages remain distinct;
- duplicate short-circuits before person/property/deal resolution;
- existing person, explicitly eligible CRM-03 creation, missing-role/conflicting-mailbox-role non-creation, and agreeing multi-mailbox-role creation;
- internal/system/no-reply addresses never create people;
- exact bounce, delivery, auto-reply, provider-system, and list-mail exclusions;
- ordinary subjects/business metadata do not cause false exclusion;
- exact property/deal rules remain CRM-02 behavior and no free-text/AI linking exists;
- provider/account token normalization, bounds, delimiter rejection, and deterministic source string;
- subject/plain-text limits and provider-owned markup/quoted-history removal contract (no heuristic stripping);
- explicit metadata allowlist rejects/discards arbitrary safe-looking provider payload; recursive secret rejection and 32 KB ceiling still apply;
- no bcc/raw HTML/MIME/bytes/tokens/signed URLs/logs;
- attachment descriptors preserve only opaque non-URL ID/name/MIME/size and reject URL-like IDs;
- zero tasks, interests, interactions, provider calls, or Neon queries/writes;
- CRM-01/02/03/04 fixtures remain green.

Use fakes that throw on unexpected calls and verify call order, not only returned shapes.

## Verification

Run:

```sh
pnpm exec tsx --env-file=.env.local scripts/verify-crm-email-intake.mjs
pnpm exec tsx --env-file=.env.local scripts/verify-website-intake.mjs
pnpm exec tsx --env-file=.env.local scripts/verify-crm-person-creation.mjs
pnpm exec tsx --env-file=.env.local scripts/verify-crm-intake.mjs
pnpm exec tsx --env-file=.env.local scripts/verify-crm-foundation.mjs
git diff --check
pnpm exec next build --webpack
```

Restore generated files. Report zero Neon/provider access, no schema/dependency/route/UI/environment changes, and no commit/push.
