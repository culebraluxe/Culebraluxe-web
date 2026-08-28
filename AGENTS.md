# CulebraLuxe Agent Operating Context

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

## Production Release State

Honor any active production freeze or explicit release restriction stated in the current task/context. Otherwise, do not assume a default production prohibition that conflicts with the startup delivery model above.
