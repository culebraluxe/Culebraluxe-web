# MKT-M1-PUBLIC-URL — Canonical public listing URL is /properties/{slug}

## Goal

Syndication QR, WhatsApp/SMS blurbs, seller report, and flyer use the live public path `https://culebraluxe.com/properties/{slug}` (www optional). `/listings/{slug}` must not 404; it redirects to `/properties/{slug}`.

## Scope

- `lib/syndication` public URL builders (`db/syndication.ts`, `share.ts`, QR route)
- `app/portal/marketing/qr/route.ts` allowlist
- New `app/listings/[slug]/page.tsx` redirect to `/properties/[slug]`
- Tests next to those files
- This packet

No new channel. No HubSpot. No Zillow POST. No workbench redesign.

## Architect brief

Live brochure is `/properties/{slug}`. Marketing still mints `/listings/{slug}`, so QR and flyers 404. Change the canonical constructor once. QR regex accepts apex + www + `/properties/{slug}/?`. Generate properties URLs going forward.

## Context refs

- `docs/portal-next-work-orders.md` Block M1
- `app/portal/marketing/qr/route.ts`
- `lib/syndication/share.ts`

## Acceptance criteria

- Slug `casa-luar` → `publicUrl` contains `/properties/casa-luar`
- WhatsApp blurbs contain that URL
- QR GET 200 for www and apex properties URLs
- QR GET 400 for `/portal/marketing`
- `GET /listings/casa-luar` redirects to `/properties/casa-luar`
- Existing syndication adapter tests stay green

## Preconditions

Slug column exists on `property`.

## Postconditions

New prepares store a `/properties/` canonical URL.

## Skills

ui

## Loop

intent: grow
loop: 1/3

## Test mode

SCOPED

## Assay commands

- pnpm exec tsx --test workflow_app/tests/syndication-adapters.test.ts
- git diff --check
