# Builder Work Order

## CRM-03 — Identity Resolution + Safe Person Creation

Status: Ready for implementation after human approval.

Follow `AGENTS.md` and `docs/agent/CURRENT.md`. Implement only this bounded fixture-backed POC.

## Scope

1. Extract or expose CRM-02's exact person-resolution logic through a neutral domain module so CRM-02 and CRM-03 share one implementation. Preserve existing CRM-02 behavior.
2. Add neutral CRM-03 contracts for:
   - explicit creation policy with required canonical role;
   - eligible identity claims;
   - temporary display-name provenance;
   - results: `created`, `resolved_existing`, `duplicate`, `conflicting`, `resolution_required`, `rejected`.
3. Extend `db/person-identities.ts` with the smallest read seams needed to inspect active and archived identity ownership exactly.
4. Add one atomic repository operation that:
   - accepts an application-generated person UUID;
   - inserts `person(display_name, role, status = 'new')`;
   - inserts deterministically ordered eligible `person_identity` claims;
   - persists deterministic per-type `is_primary` flags selected before SQL ordering;
   - uses the existing Neon client's non-interactive transaction;
   - rolls back the entire operation on any identity uniqueness conflict.
5. Add a source-neutral application service that:
   - receives a normalized inbound event plus explicit creation policy;
   - checks source-event duplication first;
   - resolves exact existing identity ownership before creation;
   - applies eligibility rules from `CURRENT.md`;
   - never uses a name for matching;
   - creates atomically only when every rule passes;
   - re-resolves identity ownership after a uniqueness race;
   - returns unclaimed hints when an existing person wins and never attempts a second creation;
   - produces no interaction, task, property-interest, workflow, or adapter writes.
6. Extend the existing query-executor abstraction only if required for an injectable non-interactive transaction seam. Do not introduce another connection/environment path.
7. Add fixture/mock verification only. No fixture may query or mutate Neon.

## Likely Files

- `lib/crm-intake-types.ts`
- `lib/crm-intake.ts`
- a focused neutral person-resolution/person-creation module under `lib/`
- `db/person-identities.ts`
- `db/query-executor.ts` only if the transaction seam requires it
- `scripts/verify-crm-person-creation.mjs`

Touch no schema, migration, route, UI, portal, adapter, package, or lock files.

## Required Behavior

- Creation requires explicit permission, a trusted canonical role, and at least one authenticated/provider-asserted email or E.164 phone.
- User-supplied-only, external-only, or name-only actors return `resolution_required`.
- Existing exact matches return `resolved_existing`; no person is created and unmatched hints are not attached.
- Multiple hints matching one person resolve cleanly; multiple owners are `conflicting`.
- A supplied explicit `personId` that does not exist is `rejected` and never triggers replacement creation.
- New person and all eligible identity claims are atomic.
- Concurrent identical claims produce one committed person; the loser returns `resolved_existing` after exact re-resolution.
- After a race, one active owner wins even when other supplied hints remain unclaimed; those hints are returned as unclaimed and no second creation is attempted.
- Archived identity ownership is not reclaimed automatically.
- Primary selection is per type and follows stable normalized input order after deduplication: first eligible email is primary email and first eligible phone is primary phone; later identities of the same type and all external identities are non-primary.
- Email-only and phone-only creation are both valid when the existing eligibility policy is satisfied.
- `roleHint` and raw metadata cannot update canonical role or other person fields.
- CRM-01 source-interaction idempotency remains unchanged.

## Required Fixture Verification

- authenticated email creates one `new` person and primary email identity;
- provider-asserted E.164 phone creates successfully;
- email plus phone are claimed in deterministic order in one transaction;
- multiple eligible emails preserve one deterministic primary email and mark later emails non-primary;
- multiple eligible phones preserve one deterministic primary phone and mark later phones non-primary;
- email-only creation has a primary email and no primary phone;
- phone-only creation has a primary phone and no primary email;
- normalized duplicate hints are claimed once;
- display-name hint is stored but never queried for matching;
- absent display name falls back to primary email, then phone;
- user-supplied-only, external-only, name-only, and no-identity cases require resolution;
- external provider identity may accompany but not replace an eligible canonical anchor;
- existing one-hint and multiple-hint matches return `resolved_existing` with zero writes;
- one existing plus one new hint returns existing and leaves the new hint unclaimed;
- multiple-person matches return `conflicting` with zero writes;
- supplied but nonexistent explicit `personId` is rejected;
- archived identity ownership requires resolution;
- duplicate source interaction short-circuits before identity reads/transaction;
- simulated unique race rolls back and resolves the one active winner;
- mixed race: email claim loses to Person A while phone remains unclaimed, returning `resolved_existing` Person A with the phone reported unclaimed and no second creation attempt;
- simulated race resolving multiple owners returns `conflicting`;
- unexpected transaction failure does not retry or leave a reported created person;
- no interaction, task, or property-interest write is reachable;
- existing CRM-01 and CRM-02 verification remains green.

## Verification

Run:

```sh
pnpm exec tsx --env-file=.env.local scripts/verify-crm-person-creation.mjs
pnpm exec tsx --env-file=.env.local scripts/verify-crm-intake.mjs
pnpm exec tsx --env-file=.env.local scripts/verify-crm-foundation.mjs
git diff --check
pnpm exec next build --webpack
```

Restore generated files. Report exact files changed and confirm zero Neon access, no schema/dependency/UI/route changes, and no commit/push.
