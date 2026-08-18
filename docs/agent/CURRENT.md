# Current Story

## CRM-03 — Identity Resolution + Safe Person Creation

Status: Architecture complete; bounded Builder work order ready for approval.

## Completed Foundations

- CRM-01 provides canonical interaction/task contracts, source-event idempotency, and injected repository seams.
- CRM-02 provides source-neutral inbound events, deterministic normalization, exact person/property/deal resolution, advisory intents, and canonical interaction input without writes.

## Goal

Safely resolve an inbound actor or atomically create one canonical `person` with claimed `person_identity` rows, without fuzzy matching, duplicate people, workflow behavior, or external integration work.

## Existing Model

- `person` requires `display_name`, `role`, and `status`; UUID and timestamps have database defaults.
- `person_identity` owns canonical identities and has global uniqueness on `(identity_type, identity_value)`.
- Email and phone values are already normalized by CRM-02. External identities are namespaced as `sourceSystem:externalId`.
- The existing Neon HTTP client supports a non-interactive transaction. Current `QueryExecutor` is query-only and must not be replaced by a second database connection path.

## Decisions

### Eligibility

- Creation is never an implicit fallback. A trusted application caller must pass an explicit creation policy.
- At least one normalized email or strict E.164 phone with `authenticated` or `provider_asserted` evidence is required.
- One eligible canonical identity is sufficient. A display name, external identity, or `user_supplied` identity alone is insufficient and returns `resolution_required`.
- External identities may be claimed only when provider-asserted/authenticated and accompanied by an eligible email or phone anchor.
- Duplicate normalized hints are deduplicated before resolution/claiming.
- If an explicit `personId` is supplied but does not resolve, creation is rejected; it is not replaced with a new person.
- Any hints resolving to multiple people are `conflicting`. No person is created.

### Existing People and Mixed Hints

- Existing people always win. If one or more hints resolve to the same active person, return `resolved_existing`.
- New/unclaimed hints accompanying an existing match are not automatically attached in CRM-03. They are returned as unclaimed evidence for later human or separately approved verification.
- An identity owned by an archived person is not reclaimed and returns `resolution_required` with an archived-identity reason.
- Names are never queried for identity resolution.

### Person Fields

- The trusted creation policy supplies the canonical `role`: `buyer`, `seller`, or `both`. The inbound `roleHint` remains advisory and cannot override policy.
- New people receive `status = new`.
- `display_name` uses the normalized display-name hint when present. Otherwise it uses the eligible primary email, then phone, as a temporary editable display value. This fallback is presentation data, never an identity key.
- CRM-03 does not populate budgets, preferences, notes, assignment, or other profile fields from raw metadata.

### Primary Identity Selection

- Primary identity is selected independently per identity type, not globally across all identities.
- Normalization and deduplication preserve the first occurrence of each unique hint. This stable normalized input order is the tie-breaker for primary selection.
- The first eligible normalized email becomes the primary email; additional emails are non-primary.
- The first eligible normalized phone becomes the primary phone; additional phones are non-primary.
- Email-only creation has a primary email and no primary phone. Phone-only creation has a primary phone and no primary email.
- External identities are non-primary.
- SQL inserts may still be sorted by type/value for deterministic lock ordering; that transaction ordering must not change the already-selected primary flags.

### Atomic Claim and Race Handling

- Generate the person UUID in the application before issuing SQL.
- Sort eligible identity claims deterministically by type/value.
- Submit person insert plus all identity inserts through the existing Neon `sql.transaction(...)` path as one non-interactive transaction.
- Identity inserts do not use `ON CONFLICT DO NOTHING`; a uniqueness violation must roll back the person insert and every identity claim.
- On unique violation, re-read every normalized identity hint:
  - if all claimed identities that now have owners belong to exactly one active person, return `resolved_existing` for that person and report every still-unclaimed hint as unclaimed;
  - if claimed identities belong to different active people, return `conflicting`;
  - if any identity belongs to an archived person, return `resolution_required` and do not attach it;
  - if ownership remains unclear, return `resolution_required`;
  - on unexpected database failure, return `rejected`/error and never retry creation blindly.
- A single active existing owner always wins over creating a duplicate person. Unmatched hints remain evidence for a later explicit claim path; they never trigger a second creation attempt.
- The existing uniqueness constraint is sufficient; no schema change is required.

### Source Idempotency

- Check `(source_system, source_external_id)` before person creation. An existing interaction returns `duplicate` and short-circuits identity/context work.
- Concurrent identical intake remains safe in two layers: identity uniqueness selects one canonical person, and CRM-01 interaction uniqueness selects one interaction.
- CRM-03 does not write an interaction. It returns the person result for the existing CRM-02 preparation/persistence boundary.

### Result Contract

- `created`: person and eligible identities committed atomically.
- `resolved_existing`: exact match or concurrency winner found; no new person committed.
- `duplicate`: source interaction already exists; no identity/person write attempted.
- `conflicting`: supplied hints resolve to different people or race resolution reveals conflicting owners.
- `resolution_required`: evidence is insufficient, identity belongs to an archived person, or safe deterministic ownership cannot be established.
- `rejected`: invalid explicit person context, policy violation, or non-recoverable repository failure.

## Repository and Service Boundaries

- Shared exact person-resolution logic belongs in a neutral CRM domain module and remains used by CRM-02.
- The application service owns policy, duplicate short-circuiting, resolution, and result mapping.
- `db/person-identities.ts` owns exact identity reads and the atomic person/identity claim operation.
- Extend the existing executor abstraction only enough to inject a transaction seam for fixtures. Production defaults must remain the existing Neon client and `sql.transaction`.
- Adapters only produce `InboundEvent`; they never call CRM tables directly.

## Schema Decision

No schema or migration change. Existing required person fields and `person_identity_unique(identity_type, identity_value)` support the POC safely.

## Deferred Risks

- Identity verification lifecycle and evidence persistence are not modeled; caller policy is trusted for this POC.
- Attaching a new hint to an existing person is deferred.
- Archived-person recovery and person merging are deferred to explicit human-controlled stories.
- Temporary display names may need later enrichment, but cannot participate in matching.
- CRM-03 proves atomic creation separately from final interaction persistence; a later intake orchestration story may choose a broader unit of work.
