import { sql } from './client'
import type { StellarDetails } from '@/lib/syndication/types'

export async function saveStellarListing(propertyId: string, details: StellarDetails): Promise<void> {
  await sql`
    insert into property_stellar_listing (
      property_id, listing_contract_date, expiration_date, listing_type, agent_mls_id,
      tax_id, tax_year, annual_tax, legal_description, zoning, total_area_sqft,
      heated_area_source, ownership_type, hoa_details, showing_instructions, occupant_type
    ) values (
      ${propertyId}, ${details.listingContractDate}, ${details.expirationDate},
      ${details.listingType}, ${details.agentMlsId}, ${details.taxId},
      ${details.taxYear}, ${details.annualTax}, ${details.legalDescription},
      ${details.zoning}, ${details.totalAreaSqft}, ${details.heatedAreaSource},
      ${details.ownershipType}, ${details.hoaDetails}, ${details.showingInstructions},
      ${details.occupantType}
    ) on conflict (property_id) do update set
      listing_contract_date = excluded.listing_contract_date,
      expiration_date = excluded.expiration_date,
      listing_type = excluded.listing_type,
      agent_mls_id = excluded.agent_mls_id,
      tax_id = excluded.tax_id,
      tax_year = excluded.tax_year,
      annual_tax = excluded.annual_tax,
      legal_description = excluded.legal_description,
      zoning = excluded.zoning,
      total_area_sqft = excluded.total_area_sqft,
      heated_area_source = excluded.heated_area_source,
      ownership_type = excluded.ownership_type,
      hoa_details = excluded.hoa_details,
      showing_instructions = excluded.showing_instructions,
      occupant_type = excluded.occupant_type,
      updated_at = now()
  `
}
