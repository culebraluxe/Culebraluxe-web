# MKT-STELLAR-DRAFT-01 — Stellar listing preparation

## Goal

In Marketing > Syndication, prepare the existing property for a Stellar listing draft: ten property values copied directly, seven values flagged for conversion or review, and fifteen listing-specific values editable on a Stellar extension record.

## Scope and ownership

- `property` owns address, price, beds, baths, size, remarks and media relationships.
- `property_stellar_listing` owns listing dates, service, MLS ID, tax, legal and access facts specific to this listing entry. No duplicate property or person values are stored there.
- Marketing displays the source mapping and its empty fields, saves the additional fields and regenerates the existing manual Stellar placement pack.
- This is a preparation artifact. The exact Stellar/SkySlope field names and requiredness remain subject to the current member account form. No external submission or photo upload is attempted.

## Acceptance

- A property with empty Stellar details shows all fifteen editable fields as missing.
- Saving details reloads their values and the next pack includes them.
- Direct values come from canonical `property`; bath split, lot units, style, agent identity and photos remain explicit review items.
- Invalid dates, negative tax and invalid tax year are refused; expiration cannot predate contract date.
- The Stellar adapter stays manual and does not claim an MLS number or accepted listing.

## Migration and release

Migration `194_stellar_listing_details.sql` must be applied and verified in DEV and PROD before the page is released. The list query uses this table and fails if it is absent. (The draft was written as `191_stellar_listing_details.sql`; `191`–`193` were taken by the Forge perimeter work shipped the same night, so it was renumbered to `194` on intake — the migration ledger is append-only and a duplicate number is a silent overwrite waiting to happen.) This checkout has no `.env.local`; schema application and deployment have not been performed.

## Assay

- `node --import tsx --test workflow_app/tests/stellar-listing-draft.test.ts` — the fence: the mapping counts are DERIVED and true, an empty property reports all fifteen editable fields as missing, a populated one shrinks that list and marks the review items, every refusal rule rejects, the RESO pack carries the listing values, and the adapter stays manual.
- `pnpm typecheck`

**No Forge clean is required to run this, and it must never be part of an assay.** `pnpm forge:clean` is control-plane WIDE — it cancels stale work items, aborts stale instances and obsoletes open tasks across the whole board, and it demands `--force` on PROD — so an automated approval gate is right to refuse it. If a story's own run is stuck, the story-scoped equivalent is `pnpm forge:story:reset <story-id> reset --force`, which touches only that story's rows. A brand-new packet needs neither: nothing has run yet.
