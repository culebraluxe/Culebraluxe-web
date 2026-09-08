# Regrid Property Enrichment

Small, one-property-at-a-time integration for CulebraLuxe forms and Property data.

## Contract

- One address lookup makes one Regrid HTTP request.
- Default search scope is Puerto Rico (`/us/pr`); set `REGRID_PATH` or `--path` when a narrower Regrid path is known.
- The request asks for at most two parcels so the same call can distinguish one match from ambiguity.
- Zero or multiple matches do not mutate Property.
- A unique match stores Regrid provenance and the complete returned parcel feature.
- Existing canonical Property values are never overwritten by Regrid enrichment.
- `listing_identifier` (the existing Property Catastro field used by forms) is filled from Regrid `parcelnumb` only when it is currently null.
- `REGRID_API_TOKEN` is sent as `x-regrid-token` and is never persisted or included in error messages.

## Prototype lookup

```bash
REGRID_API_TOKEN=... pnpm exec tsx scripts/regrid-property-lookup.ts \
  "8 Calle Example, Culebra, PR 00775"
```

This is read-only and prints the normalized parcel plus the raw Regrid feature.

## Enrich an existing Property

After the Regrid Property migration is applied to the selected database:

```bash
REGRID_API_TOKEN=... pnpm exec tsx scripts/regrid-property-lookup.ts \
  --property <property-uuid>
```

The command reads the canonical Property address and makes one Regrid call. You can pass an address after `--property` to use it as the lookup query without changing the stored Property address:

```bash
REGRID_API_TOKEN=... pnpm exec tsx scripts/regrid-property-lookup.ts \
  --property <property-uuid> "8 Calle Example, Culebra, PR 00775"
```

## Environment

- `REGRID_API_TOKEN` — required.
- `REGRID_PATH` — optional geographic restriction; defaults to `/us/pr`.
- `REGRID_API_BASE_URL` — optional test/alternate endpoint; defaults to Regrid's v2 API.

## What is promoted to canonical Property

Only blank canonical values are enriched: Catastro/listing identifier, legal owner, centroid latitude/longitude, neighborhood, year built, stories, bedrooms, bathrooms, building square feet, and lot acres. Regrid identifiers, match address, query, timestamp, and the full parcel feature are stored separately for provenance.

`parcelnumb` is treated as a Catastro *candidate* because Regrid defines it as the assessor's primary parcel identifier. We still retain its Regrid provenance, and a manually populated Catastro value always wins.
