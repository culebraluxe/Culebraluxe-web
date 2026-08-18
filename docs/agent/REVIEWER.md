# Reviewer Checklist

## CRM-03 — Identity Resolution + Safe Person Creation

Status: Awaiting implementation.

Latest result: Not reviewed.

## Architecture and Scope

- Implementation matches `CURRENT.md` and the approved `BUILDER.md` exactly.
- CRM-02 retains Adapter -> InboundEvent -> normalization/resolution -> canonical CRM input.
- Shared person resolution has one neutral implementation; no source-specific repository behavior exists.
- No adapters, integrations, workflow engine, UI, routes, tasks, interactions, or property-interest persistence were added.

## Eligibility and Identity

- Creation is explicitly authorized by trusted application policy.
- At least one authenticated/provider-asserted email or E.164 phone is required.
- User-supplied-only, external-only, name-only, and absent identities cannot create.
- Email/phone normalization is unchanged and names are never queried as keys.
- External identities retain deterministic `sourceSystem:externalId` namespacing.
- Duplicate hints are deduplicated; multiple owners are conflicting.
- Existing people win and unmatched hints are not silently attached.
- Archived identities are not reclaimed automatically.

## Person Data

- Role comes from trusted creation policy, not raw metadata or untrusted role hints.
- Status is `new` and no unrelated profile fields are synthesized.
- Display-name fallback is temporary presentation data and never matching evidence.
- Existing people are not overwritten.
- Primary selection is deterministic and per identity type after normalization/deduplication.
- Multiple emails produce exactly one primary email; multiple phones produce exactly one primary phone.
- Email-only creation has a primary email, and phone-only creation has a primary phone.
- External identities are not primary.

## Atomicity and Concurrency

- Person UUID is generated before the transaction.
- Person and all identity inserts use the existing Neon non-interactive transaction path.
- Identity ordering is deterministic.
- No `ON CONFLICT DO NOTHING` can commit an orphan person or partial identity set.
- Unique violations roll back fully and trigger exact ownership re-resolution.
- One winner returns `resolved_existing`; multiple owners return `conflicting`; unclear/archived ownership requires resolution.
- No blind retry can create another person.
- Mixed race recovery returns the single active owner even when another hint remains unmatched.
- Mixed-race unmatched hints are returned as unclaimed and no second creation attempt occurs.
- Existing `(identity_type, identity_value)` uniqueness and CRM-01 interaction idempotency are not weakened.

## Repository Boundaries and Safety

- Production defaults use the existing Neon client; transaction injection is only a test seam.
- Reads remain exact and writes are confined to the atomic person/identity repository operation.
- Fixture verification performs zero Neon queries/writes.
- No schema, migration, dependency, package, route, UI, or environment changes exist.

## Verification

- All required Builder fixtures cover behavior, rollback, race outcomes, and zero-write branches.
- CRM-01 and CRM-02 verification remain green.
- `git diff --check` passes.
- `pnpm exec next build --webpack` passes.
- Generated files are restored or removed.

## Latest Findings

Awaiting implementation.
