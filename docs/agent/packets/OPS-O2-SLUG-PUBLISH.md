# OPS-O2-SLUG-PUBLISH — Cannot publish a property without a slug

## Goal

`property.is_published = true` is rejected unless `slug` is a non-empty URL-safe string. Property Admin shows why.

## Scope

- Property Admin publish write path
- Validation helper next to existing property validators
- Targeted test
- This packet

Do not invent a slug generator unless one already exists. Do not mass-unpublish existing rows.

## Architect brief

M1 makes `/properties/{slug}` the public URL. Publishing without a slug ships a 404. Fail closed on the publish toggle. Message: “Add a public slug before publishing.”

## Context refs

- `docs/portal-next-work-orders.md` O2
- `app/portal/property-admin`

## Acceptance criteria

- Empty/null slug publish → error, `is_published` unchanged
- Slug `casa-luar` publish succeeds on DEV
- Slug with spaces or `/portal` rejected
- No existing published row is unpublished by this story

## Preconditions

Property Admin publish path exists.

## Postconditions

New public listings always have a slug.

## Skills

ui

## Loop

intent: grow
loop: 1/3

## Test mode

SCOPED

## Assay commands

- git diff --check
