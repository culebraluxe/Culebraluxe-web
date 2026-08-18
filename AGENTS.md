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

- Work in bounded stories and inspect before modifying.
- When diagnosing a bug, state the root cause before fixing it.
- Preserve behavior outside the assigned story.
- Prefer the smallest clean change.
- Report exact files changed and verification results.
- Stop for human review after implementation.

## Production Guardrails

- Never commit or push unless explicitly authorized.
- Never mutate production or business data unless explicitly authorized.
- Never hardcode secrets.
- Treat `main` as production-sensitive.
- `.env.local` is local configuration and must not be committed.
- Vercel production environment variables are separate from local environment variables.
- Capture database migrations in `db/migrations`.
- If the live database is changed manually, record an equivalent migration.

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

- Implements only the assigned bounded story.
- Does not commit or push.
- Reports exact changes.

### Reviewer

- Works primarily read-only.
- Checks regressions, hardcoding, architecture drift, responsiveness, and build results.
- Does not silently fix findings unless explicitly instructed.

## Production Release State

Honor any active production freeze or release restriction stated in the current task/context. Never assume production changes are permitted.
