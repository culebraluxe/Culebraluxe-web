# CulebraLuxe Agent Operating Context

This file is the repo-owned handbook. Vendor filenames (`CLAUDE.md`, Warp, Cursor) only point here. See `docs/agent/VENDOR-ADAPTERS.md`.

Per-story work lives in `docs/agent/packets/<STORY-ID>.md`. Skills live in `docs/agent/skills/`. Decisions that must outlive a tool live in `docs/agent/MEMORY.md`. `docs/agent/CURRENT.md` is not the memory file.

## Always / Ask / Never

Always

- Load this file, the story packet, and any listed skills before editing.
- Work in the isolated worktree when Forge provisioned one.
- Run only the packet's Assay commands (SCOPED). Do not invent `pnpm test` as FULL.
- Report exact files changed and the tests that ran.
- Commit on the worker branch only when the role is Builder.
- Route every failure that reaches a seam through the durable error-capture framework (see "Error Capture Obligation"). Never let an exception vanish as a silent 500/fallback or only a console.error.

Ask first

- FULL regression.
- Destructive PROD data changes.
- A second coding queue, a vendor-shaped rules file as source of truth, or two writers on one story.
- Relaxing the system-wide single-active lock.

Never

- Commit secrets or `.env.local`.
- Push, merge, or rebase from a worker.
- Reset PROD, copy DEV over PROD, or truncate canonical history.
- Keep a git commit as Scout, Assay, or Inspector.
- Special-case Casa Luar or any one listing in application code.
- Treat WhatsApp as a new identity type.

## Project

- CulebraLuxe is a Next.js application.
- Neon/Postgres stores business and property data.
- Mux provides video delivery.
- Vercel hosts deployments.
- The `main` branch maps to production and is production-sensitive.

## Engineering Philosophy

- Prefer clean architecture and domain boundaries over local hacks.
- Extend existing abstractions before inventing parallel systems.
- Store searchable business facts in explicit relational fields.
- Treat `media` as the reusable abstraction for images, videos, and documents.
- Let `property_media` own property-specific media roles and ordering.
- Derive conditional UI capabilities from available data.
- Avoid listing-specific hardcoding.

## Working Style

- CulebraLuxe operates as a startup with rapid fix-forward delivery, not Fortune-500-style change control.
- Prefer complete vertical slices over chains of partial handoffs.
- Work in bounded stories and inspect before modifying.
- When diagnosing a bug, state the root cause before fixing it.
- Preserve behavior outside the assigned story.
- Prefer the smallest clean change.
- Use targeted tests/builds unless a broader regression is specifically warranted.
- Avoid branch/PR/release ceremony unless explicitly requested.
- Report exact files changed and verification results.
- When implementation is authorized, complete the full release obligation for that story rather than leaving known required deployment steps to Chris.

## Production Guardrails

- Never hardcode secrets.
- Treat `main` as production-sensitive, but do not invent extra enterprise approval gates around normal authorized release work.
- `.env.local` is local configuration and must not be committed.
- Vercel production environment variables are separate from local environment variables.
- Capture database migrations in `db/migrations`.
- If the live database is changed manually, record an equivalent migration.
- Destructive production business-data changes require explicit human authorization.
- Non-destructive PROD schema changes required by an authorized story are part of that same story and should be applied and verified by the implementing agent rather than handed back as a separate operator task.
- Do not reset PROD, copy DEV over PROD, truncate canonical tables, or delete canonical business history to resolve schema drift.

## Database Delivery Rule

A database-affecting story is not complete when the migration file merely exists.

For any released code that creates, changes, or depends on schema, the implementing agent owns the complete promotion cycle:

1. create or reuse the numbered migration;
2. apply and verify it in DEV;
3. run the story's targeted tests/build;
4. apply the same required migration(s) to PROD;
5. verify the required PROD tables, columns, constraints, indexes, views, and materialized views exist and match the released code;
6. refresh derived materialized read models when current data is required;
7. only then report the story complete.

Do not return with "migration ready for PROD", "DEV verified; PROD pending", or application code that references schema newer than PROD.

The completion invariant is:

> **Code + DEV schema + PROD schema + verification = done.**

Vercel Production must never silently fall back to a DEV database. Environment-routing code must fail closed on contradictory or missing production configuration.

See `docs/STARTUP-DELIVERY-OPERATING-RULES.md` for the durable operating contract that survives session/context resets.

**DEV_OPS database playbook:** `docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md` is the operating contract for database work (environment topology, promotion order, the Neon-branch rule for refreshing DEV, hard-won rules). Two gates from it:

- **"Pull PROD down to DEV" means reset the DEV Neon branch from PROD** — instant and byte-exact. The table-by-table `scripts/pull-prod-to-dev.mjs` is the selective/partial fallback, not the normal path.
- **`pnpm db:parity` and `pnpm db:migrations` are release gates.** A branch reset *hides* drift rather than fixing it, so parity must be checked independently. (2026-09-10: DEV and PROD had silently diverged in both directions for weeks — PROD never received 116–122/138, DEV never received the Forge dispatch columns that existed in no migration, and migration 118's rename was only half-reflected in code.) The `schema_migration` ledger (migration 144) now records every apply with a checksum, so "what was run where" is answerable; pre-baseline history is reported as unrecorded rather than claimed.

## Repository Boundary Type Normalization

- Repository boundaries own normalization of database-driver-native values into stable application contracts.
- Code above the repository must not need to know whether Neon/Postgres returned a JavaScript `Date`, `BigInt`, `Buffer`, driver-specific object, or another transport/runtime representation.
- Normalize values before they leave the repository (for example: timestamps to ISO strings or `null`, counts/numerics to the intended safe JavaScript type, and JSON fields to the expected application shape).
- Do not patch UI, domain, or summarizer code to compensate for unnormalized driver types when the repository is the correct ownership boundary.
- Database-backed regression tests must exercise the real driver/runtime value shape when practical, especially for multi-row and multi-source read models where code paths such as sorting, comparison, aggregation, and serialization only execute at higher cardinality.

## Database

- `property` is the canonical listing record.
- `media` is the reusable asset record.
- `property_media` relates properties to media and owns role/order.
- Current media types: `image`, `video`, `document`.
- Current `property_media` roles: `hero`, `gallery`, `video`, `short`, `document`.
- Documents reuse `media` with `media_type = document` and `property_media.role = document`.
- Casa Luar is the current canonical real fixture for multi-image, document, and conditional-video behavior, but application code must never special-case it.
- Do not treat slug or name changes as relationship identity; `property.id` is the stable identity.
- Avoid schema changes when an existing abstraction already supports the feature.

## Property Experience

Current capabilities include:

- Property cockpit
- Gallery and media navigation
- Compact facts
- Overview
- Details
- Conditional Video
- Conditional Documents
- Google Maps
- Responsive and iPad behavior

Google Maps is the selected production map provider. Production uses `GOOGLE_MAPS_API_KEY`; the development spike uses `GOOGLE_MAPS_DEMO_KEY`. Never use the Demo Key in production.

## Buyers

- Inventory is retrieved server-side from Neon at request time.
- `/buyers` must remain request-time dynamic because inventory changes independently of Vercel deployments.
- Search and filtering operate client-side over the freshly retrieved active inventory.
- `featured = true` controls Selected Properties.
- Avoid hardcoded listing discovery.

## UI and Brand

- Brand navy: `#030f23`.
- Brand gold: `#c6a15b`.
- Maintain a luxury, editorial aesthetic.
- Preserve existing geometry unless the story explicitly changes it.
- iPad usability matters.
- Use practical touch targets of approximately 48px where appropriate.

## Build and QA

Run:

```sh
git diff --check
pnpm exec next build --webpack
```

Known issues:

- The `.next`/Turbopack cache can become stale. A clean `.next` restart may be needed before concluding that a component is broken.
- Unrelated, pre-existing `GuideItem` TypeScript errors may appear during `tsc --noEmit`. Do not broaden an unrelated story to fix them unless explicitly requested.

## Agent Roles

### Lead

- Decomposes the story.
- Protects architecture and scope.
- Reviews Builder output.

### Builder

- Implements the assigned bounded story through its actual completion point, including required non-destructive DEV/PROD schema promotion when the story changes or depends on schema.
- Does not leave required PROD migration as a separate human follow-up when implementation/release is authorized.
- Reports exact changes and verification.

### Reviewer

- Works primarily read-only.
- Checks regressions, hardcoding, architecture drift, responsiveness, and build results.
- Does not silently fix findings unless explicitly instructed.

Forge maps Lead → Architect/Inspector (git), Builder → Smith, Reviewer/QA → Assay. See `docs/FORGE-V2.md`.

> Superseded: the Forge control plane has since grown past the V2 three-role map.
> The authoritative role model is the six-role FORGE_SDLC engine
> (Scout → Architect → Lead → Smith → QA/Inspector → DEV_OPS), where **Inspector
> is a QA capability**, not a separate git role. Live topology: the engine XML +
> `workflow_app/forge/` + `docs/agent/MEMORY.md` and the Forge workshop document.
> V2/V3 docs are historical — see `docs/FORGE-V3.md` (marked superseded).

## Error Capture Obligation

New server code that can fail MUST route its failures through the durable capture framework. Do not add a bare `try/catch` that swallows, do not only `console.error`, and do not let a throw escape a route/action/edge uncaptured.

Canonical seams — reuse these; do not invent parallel capture:
- **DB**: `DatabaseGateway` captures normalized DB failures automatically.
- **Service kernel**: `BaseService` + `ServiceErrorSink` (`ServiceInfrastructure.errors`, bound via `composeCoreServices`/`appServiceErrorSink`) — captures unhandled (non-domain) exceptions with domain/operation/correlationId.
- **Route handlers that throw**: `withApiHandler({ label, route })(handler)` (`lib/error-capture-seam.ts`) — captures and returns a 500. When a handler catches-and-returns an error body instead of throwing, call `captureServerError` in the non-auth catch (pattern: `app/api/portal/form-sidecar/*`).
- **Server actions / async fns**: `withServerErrorCapture(label)(fn)`, or `captureServerError`/`captureServerLog` in the catch.
- **Low-level entry**: `recordError`/`captureError` (`db/app-error.ts`), severity `info`/`warn`/`error`/`fatal`.

Severity conveys intent: `info` observed · `warn` soft · `error` recoverable · `fatal` cannot continue. Expected business outcomes (validation failures, authorization denials/FORBIDDEN, "not found") are **audited control flow**, not error rows — never capture them as error noise.

Verify captured rows in `app_error` or the TECH view `/portal/tech/app-errors`. End-to-end probe: `node --env-file=.env.local --import tsx scripts/probe-error-capture.ts`.

Key references: `lib/server-error-capture.ts`, `lib/error-capture-seam.ts`, `lib/service-error-sink.ts`, `db/app-error.ts`, `services/core/base-service.ts`.

Human gate: new code that fails and does NOT use this framework is a review reject.

## Production Release State

Honor any active production freeze or explicit release restriction stated in the current task/context. Otherwise, do not assume a default production prohibition that conflicts with the startup delivery model above.
