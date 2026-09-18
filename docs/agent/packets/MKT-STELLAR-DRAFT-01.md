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

Two migrations, both APPLIED to DEV and PROD and verified (table, 4 CHECK constraints, and the `property.square_feet` column comment):

- `194_stellar_listing_details.sql` — the extension table. The list query uses it and fails if it is absent. (The draft was written as `191_stellar_listing_details.sql`; `191`–`193` were taken by the Forge perimeter work shipped the same night, so it was renumbered to `194` on intake — the migration ledger is append-only and a duplicate number is a silent overwrite waiting to happen.)
- `195_property_square_feet_semantics.sql` — comments only: it records what `property.square_feet` measures, because the mapping fills two MLS meanings from it and nothing in the schema said so.

## Assay

- `node --import tsx --test workflow_app/tests/stellar-listing-draft.test.ts` — the fence: the mapping counts are DERIVED and true, an empty property reports all fifteen editable fields as missing, a populated one shrinks that list and marks the review items, every refusal rule rejects, the RESO pack carries the listing values, the adapter stays manual, and interior footage is asserted to be the heated area.
- `pnpm typecheck`

**No Forge clean is required to run this, and it must never be part of an assay.** `pnpm forge:clean` is control-plane WIDE — it cancels stale work items, aborts stale instances and obsoletes open tasks across the whole board, and it demands `--force` on PROD — so an automated approval gate is right to refuse it. If a story's own run is stuck, the story-scoped equivalent is `pnpm forge:story:reset <story-id> reset --force`, which touches only that story's rows. A brand-new packet needs neither: nothing has run yet.

## Verified field map (2026-09-18, checked against the repo)

The working set is 32 fields: **10 direct, 7 needing conversion or a verified MLS choice, 15 to collect
or confirm.** Verified rather than assumed:

- Every column the map relies on exists in the property model — `street_number`/`street_name`/
  `unit_number`, `bathrooms_full`/`bathrooms_half`, `lot_size`/`lot_size_units`, `year_built`,
  `public_remarks`, `editorial_description`, `property_type`.
- **The bath split is never inferred.** The only place full/half meet is the property card SUMMING them
  for a display total; the Stellar mapping passes each through untouched.
- **`HeatedAreaSqFt` and `LivingArea` both read `property.square_feet`**, and that is correct HERE: on
  Culebra the interior square footage IS the heated area (no unconditioned-but-enclosed space, unlike a
  northern covered porch). The semantics are recorded on the column by migration 195 and asserted by the
  fence, so a future edit cannot reverse the decision by accident.
- The pack itself is nearly complete: with a filled property it emits 53 keys and 52 are sourced — the
  only key filled by nothing is `PrivateRemarks`, which is internal by design.
- The 15 extension columns cover every "significant gap" in the pre-analysis: dates, listing type,
  parcel/tax, legal description, zoning, total square footage AND its source, ownership, HOA, showing
  instructions, occupant type.

**What is NOT yet modeled:** the conditional yes/no and feature selections from the form's later pages.
Enumerating those needs the real form from the Stellar account — which is also the check that decides
whether these listings use the residential form at all and whether land needs its own.
