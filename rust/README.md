# Rust workspace

This directory is the side-by-side Rust backend workspace for CulebraLuxe.

Nothing here is wired into Next.js, Vercel production routing, or the TypeScript
runtime yet. Rust is being introduced behind explicit parity boundaries.

## Structure

- `core/domain` — infrastructure-free domain types and rules.
- `core/db` — the one Rust PostgreSQL pool plus DAOs and DB failure normalization.
- `core/workflow` — workflow engine primitives.
- `core/auth` — server-side authorization/authentication boundary.
- `forge` — Forge runtime and SDLC roles.
- `server` — authoritative CulebraLuxe service/API layer.
- `integrations` — external-system adapters.
- `cli` — operational command-line entry point.

The existing repository-level `db/migrations/` remains the canonical SQL
migration history. Do not create a competing Rust migration tree.

Dependency direction is inward toward `core/domain`. Infrastructure and
integration concerns must not leak into domain code.

Forge role ownership is preserved structurally. QA verifies outcomes and does
not own Git/release authority; DEV_OPS/release owns release identity and
promotion concerns.

## Slice 1: DB + Project service

The first real vertical slice uses the existing Neon `project` table:

```text
ProjectService
    -> ProjectRepository
        -> ProjectDao
            -> Database / SQLx PgPool
                -> Neon PostgreSQL
```

`Database` owns the only Rust PostgreSQL pool. It follows the existing
application environment contract:

- `VERCEL_ENV=production` -> `DATABASE_URL_PROD`
- `VERCEL_ENV=preview|development` -> `DATABASE_URL_DEV`
- otherwise `APP_ENV=production|prod` -> PROD
- otherwise `APP_ENV=development|dev|test|testing` -> DEV
- silence is refused; there is no implicit environment fallback.

Connection URLs using `sslmode=prefer|require|verify-ca` are pinned to
`verify-full`, matching the current TypeScript ForgeDB intent. Query ceilings
remain database defaults; the Rust client does not inject startup `options`
that Neon's pooled endpoint rejects.

### Compile and test

From `rust/`:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets
cargo check --workspace --all-targets
```

### Read-only DEV smoke

With the existing DEV environment loaded:

```bash
APP_ENV=dev cargo run -p cli -- db-smoke
```

The smoke command connects only to the explicitly declared target, runs
`select 1`, then calls the Rust `ProjectService.list()` path. It prints only
the target, project count, and non-sensitive project identity/status metadata.
It performs no writes and never prints a database URL or credential.


## Slice 2: service kernel + transactions

The shared `core/service` crate owns transport-neutral service context and the
authorization, audit, and domain-event ports. Project is the first consumer.

The database crate remains the only Rust owner of SQLx pool and transaction
mechanics. `Database::begin()` returns an opaque `DbTransaction`, and
`ProjectTxDao` binds repository operations to that transaction without leaking
SQLx transaction types into the server/service layer.

The DEV-only transaction smoke is:

```bash
APP_ENV=dev cargo run -p cli -- tx-smoke
```

It refuses PROD, creates a unique Project inside one transaction, reads it,
updates it from open to doing through `ProjectService`, verifies audit and
domain-event capture, explicitly rolls back, then verifies through the pooled DAO
that the test row does not exist.

GitHub Actions keeps this database smoke parked behind `RUST_DB_CI=true` until
`DATABASE_URL_DEV` is configured as an Actions secret. Normal Rust format,
unit-test, and compile gates remain mandatory on every push.


## Slice 3A: Person + Firm

The first reduced core-service migration slice ports only the two vanilla
canonical services:

- Person: get, identity lookup, display-name write, identity attach, operator search.
- Firm: get, exact name lookup, create/enrich upsert.

Both use the shared Rust service authorization/audit/event kernel and the single
Rust SQLx pool. Person preserves canonical identity ownership and phone/email
normalization. Firm preserves the TypeScript distinction between an omitted
optional field and an explicit null through `FieldPatch<T>`.

This slice intentionally does not include Property, Contract, Showing, Security,
or WBS. Each later slice must be green before the next domain is added.


## Slice 3B: Property

Canonical Property ownership is now ported to Rust on top of the green
Person/Firm slice. The service preserves address lookup, Person-to-Property
relationship context, display/status writes, and atomic Property + relationship
upserts. Optional field writes preserve omitted-vs-explicit-clear semantics via
the shared `FieldPatch<T>` type.

The original bulk-sweep Property service was salvaged, but its persistence layer
was rewritten to use compile-time static SQL accepted by SQLx 0.9; no
`AssertSqlSafe` escape hatch is used.


## Slice 3C: Security + Showing

Security and Showing are salvaged from the original core-service sweep.
Security keeps provider-subject resolution, active-user principal loading,
role/authority projection, level resolution, and fail-closed behavior.
Showing keeps Person/Property ownership checks through `ServiceDirectory`,
binding-conflict protection, report validation, and service audit/events.

Both DAOs use static SQL and were rechecked against the live DEV schema before
publication.


## Slice 3D: WBS

WBS is salvaged from the original sweep with its fixed category/status/entity
vocabulary, Project scoping, due work queries, create/save/complete/dismiss
commands, and service audit/events.

The rejected runtime-built SELECT strings were replaced with compile-time static
SQL accepted by SQLx 0.9. The DAO was rechecked against the live DEV
`wbs_item` schema before publication.


## Slice 3E: Contract

Contract completes the required Rust core-service sweep. The salvaged service
preserves contextual Person/Firm Role vocabulary, SUBJECT_PROPERTY mapping,
draft-only mutation, predecessor validation, recursive effective-state lineage,
execution evidence, and stronger BUSINESS_POWER_USER authorization for
`contract.execute`.

Contract writes remain atomic through the shared Rust transaction boundary.
The original dynamic mapping-delete loop was replaced with literal static SQL
accepted by SQLx 0.9, and SQLx JSON support is enabled for canonical JSONB
facts and role attributes.


## Slice 4A: Comms

Comms is the first full-runtime optional service moved to Rust after the
required business core. It is read-only and preserves the CRM Client-pane
contract: aggregate relationship counts, one canonical source row per
communication source, and paginated canonical interaction moments.

The Rust port keeps Phone and FaceTime as distinct source rows, recovers
FaceTime moments from the intake delineation when interaction.channel is
`call`, excludes bulk/service evidence from meaningful-contact freshness,
reads warehouse/read-model relations only, and never reads an ODS `l_` table.


## Slice 4B: Forms

Forms moves the mutable document-form-instance domain into Rust. It owns
create/get/update/list, bounded deal prefill facts, participant seeding, latest
form evidence, direct/listing/showing lineage bindings, and signer-person
resolution.

The port preserves the important boundary: a Form instance is mutable working
state, not the immutable business record. Listing context cannot be rebound
after issuance history exists, Showing bindings are non-transferable, and the
Listing/Purchase-and-Sale signer path keeps the configured Lisa Penfield
SELLER_BROKER rule.


## Slice 4C: Vault

Vault moves transaction-document state, immutable issuance lineage, signed-artifact
state, source idempotency, command receipts, media bytes, and form-to-Contract
binding into Rust.

Issuance remains one authoritative transaction: claim command receipt, read the
Form snapshot and signer context, invoke the required artifact-render port,
persist PDF media, supersede the prior issued document, append the immutable
transaction_document row, mark the Form issued, finalize the receipt, then
commit once. Renderer/provider code does not own issuance truth.

The Rust model makes signed media + signed time one SignedArtifactRef and DOC-06
issuance evidence one all-or-nothing IssuedDocumentEvidence, so partial records
are not representable through the service contract.


## Slice 4D: Calendar + Media

Calendar and Media finish the small application-service sweep.

Calendar reads canonical Showings plus durable Apple Calendar landing rows and
queues explicit Apple Calendar writes through the existing outbox route. Apple
Calendar/EventKit remains authoritative at the native Mac gateway; Rust does not
replace the Swift/native edge.

Media is intentionally read-only and projects canonical assets from
`property_media + media` without copying, renaming, or deleting files.


## Slice 5A: Axum read transport

The Rust backend now has a real Axum HTTP boundary without moving production
traffic yet. The binary defaults to `127.0.0.1:8080`, refuses to start without
`CULEBRA_INTERNAL_API_KEY`, and exposes public liveness/readiness plus
authenticated read routes for the already-ported service kernel.

Auth.js remains the authentication edge. The trusted Next/server edge forwards
only provider + stable provider subject; Rust Security resolves the canonical
active app_user, roles, and SecurityLevel from the database. The transport does
not trust forwarded role or level headers.

Success responses preserve the service-kernel envelope shape:
`{ ok, value, correlationId }`. Errors preserve typed codes, retryability,
correlation IDs, and database incident IDs without exposing connection details.

This slice is intentionally read-first. No HTTP business-write route is exposed
until the transport and identity boundary are proven. Vault issuance is not
reachable from HTTP until the artifact-render/provider adapter is wired.


## Slice 5B: Edge bridge + container

The Next/Auth.js edge now has a dormant GET-only Rust client contract. It
forwards only the authenticated provider + stable provider subject, the shared
internal transport key, and correlation/causation IDs. It never forwards a
trusted role list or SecurityLevel; Rust Security resolves those from canonical
database state.

Pure bridge helpers are covered by the existing Node harness tests, including
production URL behavior, minimum internal-key length, correlation/causation
headers, and the invariant that role/security headers are absent.

`rust/Dockerfile` packages the Axum binary as a non-root, provider-neutral
container with `/healthz` liveness probing. No production route is switched by
this slice.

Build from repository root:

```bash
docker build -f rust/Dockerfile -t culebraluxe-rust-api .
```

Runtime configuration remains external:
`DATABASE_URL_DEV` / `DATABASE_URL_PROD`, explicit environment target,
`CULEBRA_INTERNAL_API_KEY`, and optional `RUST_API_BIND`.


## Slice 6A: Signature seam + BoldSign trust edge

The provider-neutral signature lifecycle is now represented in Rust: canonical
signature requests, immutable envelope recipients, one-active-envelope
idempotency, neutral status transitions, and the revoke-first cancellation
ordering required before an active legal envelope slot may be released.

The canonical send boundary revalidates slot-bound recipients against the exact
issued document's immutable `source_snapshot.issuedParticipants`. The client is
never authoritative for execution role, slot identity, or signer mailbox.

The BoldSign adapter boundary now owns status normalization plus webhook trust:
timestamped HMAC-SHA256 verification over the exact raw request body, replay
tolerance, provider-event dedupe, provider-envelope lookup, and fail-closed
handling for reassignment/authentication/identity-verification events.
Recipient-level Signed activity remains neutral Viewed until BoldSign proves the
whole envelope Completed.

Signature writes are not exposed through Axum yet. Signed-artifact
reconciliation and concrete outbound BoldSign delivery remain the next adapter
step before transport cutover.


## Slice 6B: Signed-artifact reconciliation

SignatureProvider is now an inward-facing port owned by `core/service`.
External adapters implement the port; the server consumes it. This preserves the
dependency rule that provider-specific code stays at the integrations edge and
prevents a server-to-integrations-to-server cycle.

Completed neutral signature observations reconcile into Vault truth with two
idempotency guards: a receipt keyed by the neutral completion attempt and an
already-signed document guard. Provider artifact downloads occur before the
database transaction. Inside one transaction Rust appends new signed/audit media,
advances draft/ready documents through the legal pre-signed chain when required,
records the final signed lineage, and finalizes the reconciliation receipt.
Duplicate completion observations therefore cannot append a second signed PDF.


## Slice 6C: WhatsApp webhook trust edge

The Meta WhatsApp Cloud API trust boundary is represented in Rust without
changing production routing. The adapter verifies the GET subscription challenge
with a constant-time token comparison and validates POST callbacks against
Meta's exact raw-body `X-Hub-Signature-256` HMAC before payload parsing.

Configuration remains external through `WHATSAPP_APP_SECRET`,
`WHATSAPP_PHONE_NUMBER_ID`, `WHATSAPP_OWNED_PHONE_E164`, and
`WHATSAPP_VERIFY_TOKEN`. E.164 normalization is bounded and rejects malformed
owned-line configuration rather than guessing.


## Slice 6D: Concrete BoldSign provider

The Rust BoldSign adapter now implements the provider-neutral
`SignatureProvider` port. It loads the immutable issued PDF and its frozen
signature anchors from Vault lineage, converts PDF bottom-left coordinates to
BoldSign field coordinates, binds each signer to the declared role/slot, and
uses Email OTP with reassignment explicitly disabled.

Legal-envelope creation is deliberately single-attempt. A timeout, transport
failure, or retryable provider error leaves an ambiguous active provider state
and is recorded without an automatic resend; this prevents duplicate legally
signable envelopes. Status polling, revocation, signed-PDF download, and audit
trail download use bounded retries because those operations are read-only or
convergent.

Provider ids and raw statuses remain confined to the BoldSign adapter/store.
Canonical Signature and Vault state continue to use neutral lifecycle values and
signed-document lineage only. Axum signature write routes remain disabled until
the adapter and reconciliation slice are green together.
