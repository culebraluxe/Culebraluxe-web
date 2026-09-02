# Slice E — Public listing inquiry → CRM: not built (no public form)

Status: **blocked / not started** (finding, not a gap to paper over).

## Finding

Slice E ("log a public inquiry against a property + person from the Launch panel")
requires a public listing inquiry form that already writes into the CRM. Inspected
the codebase (`feat/marketing-syndication`):

- The canonical public listing URL is `https://culebraluxe.com/listings/{slug}`. That
  route is **not served by this Next app** (no `app/listings/[slug]` page). The app
  does serve public property pages under `app/properties`, but there is no
  property-scoped inquiry/contact form there that posts to the CRM tables.
- Existing `person` / `inquiry` / lead handling lives behind the portal/CRM surfaces
  and catch-up intake (`db/catchup-lead.ts`, `lib/catchup/lead-intake.ts`); it is not
  wired to a public listing page form.

Per the work order, we do **not** invent a marketing-only form or start HubSpot. No
inquiry→CRM code was added.

## When to revisit

When the public `listings/{slug}` page (or an `app/properties/{slug}` inquiry form)
exists and posts server-side, the Launch panel can attach `property_id` + link the
matching `person` using the existing person/inquiry tables. Until then Slice E stays
a short note.
