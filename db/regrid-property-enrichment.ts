import { sql } from './client'
import type {
  RegridPropertyEnrichmentReceipt,
  RegridPropertyEnrichmentRepository,
  RegridPropertyEnrichmentWrite,
  RegridPropertyTarget,
} from '../services/regrid/types'

type TargetRow = {
  id: string
  address_line1: string | null
  city: string | null
  state_or_province: string | null
  postal_code: string | null
  country: string | null
  iso_country_code: string | null
  listing_identifier: string | null
}

type ReceiptRow = {
  id: string
  listing_identifier: string | null
  catastro_source: string | null
  regrid_parcel_number: string | null
  regrid_ll_uuid: string | null
  regrid_enriched_at: unknown
}

function compact(value: unknown): string | null {
  if (value === null || value === undefined) return null
  const result = String(value).trim()
  return result || null
}

/** SQL adapter for the Regrid integration. All database work remains behind db/client's DatabaseGateway. */
export class SqlRegridPropertyEnrichmentRepository
  implements RegridPropertyEnrichmentRepository
{
  async getTarget(propertyId: string): Promise<RegridPropertyTarget | null> {
    const rows = (await sql`
      select
        id,
        address_line1,
        city,
        state_or_province,
        postal_code,
        country,
        iso_country_code,
        listing_identifier
      from property
      where id = ${propertyId}
        and archived_at is null
      limit 1
    `) as TargetRow[]
    const row = rows[0]
    if (!row) return null
    return {
      id: row.id,
      addressLine1: compact(row.address_line1),
      city: compact(row.city),
      stateOrProvince: compact(row.state_or_province),
      postalCode: compact(row.postal_code),
      country: compact(row.country),
      isoCountryCode: compact(row.iso_country_code),
      catastroNumber: compact(row.listing_identifier),
    }
  }

  async persist(input: RegridPropertyEnrichmentWrite): Promise<RegridPropertyEnrichmentReceipt> {
    const p = input.parcel
    const rawFeature = JSON.stringify(p.rawFeature)

    const rows = (await sql`
      update property
      set
        regrid_ll_uuid = ${p.llUuid},
        regrid_parcel_number = ${p.parcelNumber},
        regrid_path = ${p.path},
        regrid_lookup_query = ${input.lookupQuery},
        regrid_match_address = ${p.situsAddress},
        regrid_enriched_at = now(),
        regrid_data = ${rawFeature}::jsonb,

        -- LISTING-01 already reads listing_identifier as the Catastro number.
        -- Regrid may fill a blank value, but it NEVER overwrites human/imported truth.
        listing_identifier = coalesce(listing_identifier, ${p.parcelNumber}),
        catastro_source = case
          when listing_identifier is null and ${p.parcelNumber} is not null then 'regrid'
          else catastro_source
        end,

        -- Promote only high-value standard parcel facts, and only into blanks.
        -- The complete Regrid feature remains in regrid_data for later mapping.
        legal_owner_name = coalesce(legal_owner_name, ${p.owner}),
        latitude = coalesce(latitude, ${p.latitude}),
        longitude = coalesce(longitude, ${p.longitude}),
        neighborhood = coalesce(neighborhood, ${p.neighborhood}),
        year_built = coalesce(year_built, ${p.yearBuilt}),
        stories = coalesce(stories, ${p.stories}),
        bedrooms = coalesce(bedrooms, ${p.bedrooms}),
        bathrooms = coalesce(bathrooms, ${p.bathrooms}),
        square_feet = coalesce(square_feet, ${p.buildingSquareFeet}),
        lot_size = coalesce(lot_size, ${p.lotAcres}),
        lot_size_units = case
          when lot_size is null and ${p.lotAcres} is not null
            then coalesce(lot_size_units, 'acres')
          else lot_size_units
        end,
        updated_at = now()
      where id = ${input.propertyId}
        and archived_at is null
      returning
        id,
        listing_identifier,
        catastro_source,
        regrid_parcel_number,
        regrid_ll_uuid,
        regrid_enriched_at
    `) as ReceiptRow[]

    const row = rows[0]
    if (!row) throw new Error(`Property '${input.propertyId}' was not found while saving Regrid enrichment`)
    return {
      propertyId: row.id,
      catastroNumber: compact(row.listing_identifier),
      catastroSource: compact(row.catastro_source),
      regridParcelNumber: compact(row.regrid_parcel_number),
      regridLlUuid: compact(row.regrid_ll_uuid),
      regridEnrichedAt: row.regrid_enriched_at,
    }
  }
}
