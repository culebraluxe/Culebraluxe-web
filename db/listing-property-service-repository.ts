import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import { SqlPropertyRepository } from './property-service-repository'
import type {
  PropertyAddressDto,
  PropertyForPersonDto,
  UpsertPropertyForPersonRequest,
} from '@/services/property'

type RegistrySchemaRow = { ready: boolean }

type IdRow = { id: string }

function compact(value: string | null | undefined): string | null {
  const next = value?.trim()
  return next ? next : null
}

function mergeAddress(
  current: PropertyAddressDto | null,
  patch: Partial<PropertyAddressDto> | undefined,
): PropertyAddressDto {
  const base: PropertyAddressDto = current ?? {
    addressLine1: null,
    city: null,
    stateOrProvince: null,
    neighborhood: null,
    postalCode: null,
    country: null,
    isoCountryCode: null,
  }
  return {
    addressLine1: patch?.addressLine1 === undefined ? base.addressLine1 : patch.addressLine1,
    city: patch?.city === undefined ? base.city : patch.city,
    stateOrProvince: patch?.stateOrProvince === undefined ? base.stateOrProvince : patch.stateOrProvince,
    neighborhood: patch?.neighborhood === undefined ? base.neighborhood : patch.neighborhood,
    postalCode: patch?.postalCode === undefined ? base.postalCode : patch.postalCode,
    country: patch?.country === undefined ? base.country : patch.country,
    isoCountryCode: patch?.isoCountryCode === undefined ? base.isoCountryCode : patch.isoCountryCode,
  }
}

/**
 * LISTING-01 compatibility adapter for the staged Property rollout.
 *
 * Listing only owns the 115/116 Person <-> Property fields. Migration 117 adds
 * PR registry qualifiers for P&S. Until 117 is promoted, Listing must continue
 * to round-trip its canonical fields instead of failing because the generic
 * Property repository also knows about those later optional columns.
 */
export class SqlListingPropertyRepository extends SqlPropertyRepository {
  constructor(private readonly listingExecute: QueryExecutor = sql) {
    super(listingExecute)
  }

  private async registryColumnsReady(): Promise<boolean> {
    const rows = (await this.listingExecute`
      select (
        exists (
          select 1 from information_schema.columns
          where table_schema = 'public' and table_name = 'property' and column_name = 'registry_entry'
        )
        and exists (
          select 1 from information_schema.columns
          where table_schema = 'public' and table_name = 'property' and column_name = 'finca_number'
        )
        and exists (
          select 1 from information_schema.columns
          where table_schema = 'public' and table_name = 'property' and column_name = 'registry_section'
        )
      ) as ready
    `) as RegistrySchemaRow[]
    return rows[0]?.ready === true
  }

  override async upsertForPerson(
    request: UpsertPropertyForPersonRequest,
  ): Promise<PropertyForPersonDto> {
    if (await this.registryColumnsReady()) {
      return super.upsertForPerson(request)
    }

    const registryInput = [
      request.registryEntry,
      request.fincaNumber,
      request.registrySection,
    ].some((value) => value !== undefined && compact(value) !== null)
    if (registryInput) {
      throw new Error('Property registry qualifiers require migration 117.')
    }

    const explicitId = compact(request.propertyId)
    const requestedLine = compact(request.address?.addressLine1)
    let current = explicitId ? await this.get(explicitId) : null

    if (!current && !explicitId && requestedLine) {
      current = await this.findByAddress({
        addressLine1: requestedLine,
        municipality: request.address?.city ?? undefined,
        stateOrProvince: request.address?.stateOrProvince ?? undefined,
        postalCode: request.address?.postalCode ?? undefined,
      })
    }

    const address = mergeAddress(current?.address ?? null, request.address)
    const localName = request.localName === undefined
      ? current?.localName ?? null
      : compact(request.localName)
    const legalOwnerName = request.legalOwnerName === undefined
      ? current?.legalOwnerName ?? null
      : compact(request.legalOwnerName)
    const catastroNumber = request.catastroNumber === undefined
      ? current?.catastroNumber ?? null
      : compact(request.catastroNumber)

    if (!current && !compact(address.addressLine1) && !localName) {
      throw new Error('Property requires an address or local name before it can be created.')
    }

    let propertyId: string
    if (current) {
      const rows = (await this.listingExecute`
        update property p
        set name = ${localName},
            legal_owner_name = ${legalOwnerName},
            listing_identifier = ${catastroNumber},
            address_line1 = ${compact(address.addressLine1)},
            city = ${compact(address.city)},
            state_or_province = ${compact(address.stateOrProvince)},
            neighborhood = ${compact(address.neighborhood)},
            postal_code = ${compact(address.postalCode)},
            country = ${compact(address.country)},
            iso_country_code = ${compact(address.isoCountryCode)},
            updated_at = now()
        where p.id = ${current.id}
        returning p.id
      `) as IdRow[]
      if (!rows[0]?.id) throw new Error(`Property not found: ${current.id}`)
      propertyId = rows[0].id
    } else {
      const rows = (await this.listingExecute`
        insert into property (
          name, legal_owner_name, listing_identifier,
          address_line1, city, state_or_province, neighborhood, postal_code,
          country, iso_country_code, source_type
        ) values (
          ${localName}, ${legalOwnerName}, ${catastroNumber},
          ${compact(address.addressLine1)}, ${compact(address.city)}, ${compact(address.stateOrProvince)},
          ${compact(address.neighborhood)}, ${compact(address.postalCode)},
          ${compact(address.country)}, ${compact(address.isoCountryCode)},
          ${compact(request.sourceType) ?? 'manual'}
        )
        returning id
      `) as IdRow[]
      if (!rows[0]?.id) throw new Error('Property creation returned no row.')
      propertyId = rows[0].id
    }

    await this.listingExecute`
      insert into person_property (
        person_id, property_id, relation_type, relation_status, source_type, source_key
      ) values (
        ${request.personId}, ${propertyId}, ${request.relation},
        ${request.relationStatus ?? null}, ${compact(request.sourceType) ?? 'manual'},
        ${compact(request.sourceKey)}
      )
      on conflict (person_id, property_id, relation_type)
      do update set
        relation_status = excluded.relation_status,
        source_type = excluded.source_type,
        source_key = excluded.source_key,
        updated_at = now()
    `

    const property = await this.get(propertyId)
    if (!property) throw new Error(`Property not found after upsert: ${propertyId}`)
    return {
      relation: request.relation,
      relationStatus: request.relationStatus ?? null,
      property,
    }
  }
}
