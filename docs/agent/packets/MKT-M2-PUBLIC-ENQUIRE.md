# MKT-M2-PUBLIC-ENQUIRE — Public Enquire writes person + property_interest

## Goal

Enquire on `/properties/{slug}` creates or matches a `person` and upserts `property_interest` with `source = web`, same tables as Marketing Log inquiry.

## Scope

- Inspect public property enquire UI (`app/properties` or equivalent)
- Extend the existing form POST
- Reuse `logListingInquiry` / person match-by-email-or-phone
- Honeypot + coarse rate limit
- This packet

No HubSpot. No `marketing_lead` table. Public POST must not write syndication placements.

## Architect brief

Slice E looked at the wrong path. The brochure is `/properties/{slug}`. Grep Enquire first. Match person by email then phone. If `source` check constraint blocks `web`, add migration `104_property_interest_source_web.sql`.

## Context refs

- `docs/portal-next-work-orders.md` M2
- `docs/syndication-slice-e-note.md`
- `app/portal/marketing/actions.ts` `logInquiryAction`
- `db/syndication.ts` `logListingInquiry`

## Acceptance criteria

- Enquire with name + email + slug → person + property_interest source web
- Same email + property does not create a second person
- Public POST cannot write `listing_syndication_placement`
- Honeypot filled → no write
- Person visible in Clients

## Preconditions

Enquire control exists on the public property page.

## Postconditions

Launch Log inquiry and public Enquire share one table.

## Skills

ui
neon

## Loop

intent: grow
loop: 1/3

## Test mode

SCOPED

## Assay commands

- git diff --check
