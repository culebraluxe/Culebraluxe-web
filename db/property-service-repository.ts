import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import type {
  FindPropertyByAddressRequest,
  PersonPropertyContextDto,
  PersonPropertyRelation,
  PropertyAddressDto,
  PropertyDto,
  PropertyForPersonDto,
  PropertyObservedAddressDto,
  PropertyRepository,
  SetPropertyDisplayNameRequest,
  SetPropertyStatusRequest,
  UpsertPropertyForPersonRequest,
} from '@/services/property'

type PropertyRow = {
  id: string
  name: string | null
  legal_owner_name: string | null
  listing_identifier: string | null
  status: string
  archived_at: string | Date | null
  address_line1: string | null
  location: string | null
  street_number: string | null
  street_name: string | null
  unit_number: string | null
  city: string | null
  state_or_province: string | null
  neighborhood: string | null
  postal_code: string | null
  country: string | null
  iso_country_code: string | null
}

type PropertyForPersonRow = PropertyRow & {
  relation_type: PersonPropertyRelation
  relation_status: string | null
}

type ObservedAddressRow = {
  source: string
  source_account: string
  source_contact_id: string
  source_label: string | null
  street: string | null
  city: string | null
  state: string | null
  postal_code: string | null
  country: string | null
  iso_country_code: string | null
  ordinal: number
}

type RelationTableRow = { table_name: string | null }

function toIso(value: string | Date | null | undefined): string | null {
  if (value == null) return null
  const date = value instanceof Date ? value : new Date(value)
  return Number.isNaN(date.getTime()) ? String(value) : date.toISOString()
}

function compact(value: string | null | undefined): string | null {
  const next = value?.trim()
  return next ? next : null
}

/**
 * Canonical address text wins. Structured street columns enrich that text but
 * never require Forms to parse Puerto Rico/PO-box syntax just to save business
 * truth. `location` remains the final legacy fallback only.
 */
function canonicalAddressLine(row: PropertyRow): string | null {
  const canonical = compact(row.address_line1)
  if (canonical) return canonical

  const street = [compact(row.street_number), compact(row.street_name)]
    .filter((value): value is string => Boolean(value))
    .join(' ')
  const unit = compact(row.unit_number)
  if (street) return unit ? `${street}, ${unit}` : street

  return compact(row.location)
}

function canonicalAddress(row: PropertyRow): PropertyAddressDto {
  return {
    addressLine1: canonicalAddressLine(row),
    city: row.city,
    stateOrProvince: row.state_or_province,
    neighborhood: row.neighborhood,
    postalCode: row.postal_code,
    country: row.country,
    isoCountryCode: row.iso_country_code,
  }
}

function observedAddress(row: ObservedAddressRow): PropertyAddressDto {
  return {
    addressLine1: row.street,
    city: row.city,
    stateOrProvince: row.state,
    neighborhood: null,
    postalCode: row.postal_code,
    country: row.country,
    isoCountryCode: row.iso_country_code,
  }
}

function addressLabel(address: PropertyAddressDto): string {
  return [
    address.addressLine1,
    address.neighborhood,
    address.city,
    [address.stateOrProvince, address.postalCode].filter(Boolean).join(' ') || null,
    address.country,
  ]
    .filter((value): value is string => Boolean(value?.trim()))
    .join(', ') || 'Property'
}

function toProperty(row: PropertyRow): PropertyDto {
  const address = canonicalAddress(row)
  const localName = compact(row.name)
  return {
    id: row.id,
    displayName: localName ?? addressLabel(address),
    localName,
    legalOwnerName: compact(row.legal_owner_name),
    catastroNumber: compact(row.listing_identifier),
    addressLine1: address.addressLine1,
    municipality: address.city,
    address,
    status: row.status,
    archivedAt: toIso(row.archived_at),
  }
}

function normalized(value: string | null | undefined): string {
  return (value ?? '').trim().toLowerCase().replace(/\s+/g, ' ')
}

function relationRank(relation: PersonPropertyRelation): number {
  switch (relation) {
    case 'legal_address': return 0
    case 'physical_property': return 1
    case 'address': return 2
    case 'interest': return 3
  }
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

const SELECT_PROPERTY = `
  p.id, p.name,
  to_jsonb(p)->>'legal_owner_name' as legal_owner_name,
  p.listing_identifier,
  p.status, p.archived_at,
  to_jsonb(p)->>'address_line1' as address_line1,
  p.location, p.street_number, p.street_name, p.unit_number,
  p.city, p.state_or_province, p.neighborhood, p.postal_code,
  to_jsonb(p)->>'country' as country,
  to_jsonb(p)->>'iso_country_code' as iso_country_code
`

/** Production adapter behind PropertyService. */
export class SqlPropertyRepository implements PropertyRepository {
  constructor(private readonly execute: QueryExecutor = sql) {}

  async get(propertyId: string): Promise<PropertyDto | null> {
    const rows = (await this.execute`
      select
        p.id, p.name,
        to_jsonb(p)->>'legal_owner_name' as legal_owner_name,
        p.listing_identifier,
        p.status, p.archived_at,
        to_jsonb(p)->>'address_line1' as address_line1,
        p.location, p.street_number, p.street_name, p.unit_number,
        p.city, p.state_or_province, p.neighborhood, p.postal_code,
        to_jsonb(p)->>'country' as country,
        to_jsonb(p)->>'iso_country_code' as iso_country_code
      from property p
      where p.id = ${propertyId}
      limit 1
    `) as PropertyRow[]
    return rows[0] ? toProperty(rows[0]) : null
  }

  async findByAddress(request: FindPropertyByAddressRequest): Promise<PropertyDto | null> {
    const line = request.addressLine1.trim()
    const municipality = request.municipality?.trim() || null
    const state = request.stateOrProvince?.trim() || null
    const postalCode = request.postalCode?.trim() || null
    const rows = (await this.execute`
      select
        p.id, p.name,
        to_jsonb(p)->>'legal_owner_name' as legal_owner_name,
        p.listing_identifier,
        p.status, p.archived_at,
        to_jsonb(p)->>'address_line1' as address_line1,
        p.location, p.street_number, p.street_name, p.unit_number,
        p.city, p.state_or_province, p.neighborhood, p.postal_code,
        to_jsonb(p)->>'country' as country,
        to_jsonb(p)->>'iso_country_code' as iso_country_code
      from property p
      where p.archived_at is null
        and (${municipality}::text is null or lower(trim(coalesce(p.city, ''))) = lower(trim(${municipality})))
        and (${state}::text is null or lower(trim(coalesce(p.state_or_province, ''))) = lower(trim(${state})))
        and (${postalCode}::text is null or lower(trim(coalesce(p.postal_code, ''))) = lower(trim(${postalCode})))
      order by p.updated_at desc nulls last, p.id asc
      limit 250
    `) as PropertyRow[]
    const match = rows.find((row) => normalized(canonicalAddressLine(row)) === normalized(line))
    return match ? toProperty(match) : null
  }

  async forPerson(personId: string): Promise<PersonPropertyContextDto> {
    const relationTable = (await this.execute`
      select to_regclass('public.person_property')::text as table_name
    `) as RelationTableRow[]
    const canonicalRows: PropertyForPersonRow[] = []

    if (relationTable[0]?.table_name) {
      const rows = (await this.execute`
        select distinct
          p.id, p.name,
          to_jsonb(p)->>'legal_owner_name' as legal_owner_name,
          p.listing_identifier,
          p.status, p.archived_at,
          to_jsonb(p)->>'address_line1' as address_line1,
          p.location, p.street_number, p.street_name, p.unit_number,
          p.city, p.state_or_province, p.neighborhood, p.postal_code,
          to_jsonb(p)->>'country' as country,
          to_jsonb(p)->>'iso_country_code' as iso_country_code,
          pp.relation_type,
          pp.relation_status
        from person_property pp
        join property p on p.id = pp.property_id
        where pp.person_id = ${personId}
          and p.archived_at is null
      `) as PropertyForPersonRow[]
      canonicalRows.push(...rows)
    }

    const legacyInterestRows = (await this.execute`
      select distinct
        p.id, p.name,
        to_jsonb(p)->>'legal_owner_name' as legal_owner_name,
        p.listing_identifier,
        p.status, p.archived_at,
        to_jsonb(p)->>'address_line1' as address_line1,
        p.location, p.street_number, p.street_name, p.unit_number,
        p.city, p.state_or_province, p.neighborhood, p.postal_code,
        to_jsonb(p)->>'country' as country,
        to_jsonb(p)->>'iso_country_code' as iso_country_code,
        'interest'::text as relation_type,
        pi.status as relation_status
      from property_interest pi
      join property p on p.id = pi.property_id
      where pi.person_id = ${personId}
        and p.archived_at is null
    `) as PropertyForPersonRow[]
    canonicalRows.push(...legacyInterestRows)

    const legacySellerRows = (await this.execute`
      select distinct
        p.id, p.name,
        to_jsonb(p)->>'legal_owner_name' as legal_owner_name,
        p.listing_identifier,
        p.status, p.archived_at,
        to_jsonb(p)->>'address_line1' as address_line1,
        p.location, p.street_number, p.street_name, p.unit_number,
        p.city, p.state_or_province, p.neighborhood, p.postal_code,
        to_jsonb(p)->>'country' as country,
        to_jsonb(p)->>'iso_country_code' as iso_country_code,
        'physical_property'::text as relation_type,
        null::text as relation_status
      from property p
      where p.seller_person_id = ${personId}
        and p.archived_at is null
    `) as PropertyForPersonRow[]
    canonicalRows.push(...legacySellerRows)

    const properties: PropertyForPersonDto[] = []
    const seenRelations = new Set<string>()
    for (const row of canonicalRows) {
      const key = `${row.id}:${row.relation_type}`
      if (seenRelations.has(key)) continue
      seenRelations.add(key)
      properties.push({
        relation: row.relation_type,
        relationStatus: row.relation_status,
        property: toProperty(row),
      })
    }
    properties.sort((a, b) =>
      relationRank(a.relation) - relationRank(b.relation) ||
      a.property.displayName.localeCompare(b.property.displayName),
    )

    const observedRows = (await this.execute`
      select
        lp.source, lp.source_account, lp.source_contact_id,
        a.source_label, a.street, a.city, a.state, a.postal_code,
        a.country, a.iso_country_code, a.ordinal
      from integration_source_person_link link
      join l_person lp
        on lp.source = link.source
       and lp.source_account = link.source_account
       and lp.source_contact_id = link.source_identity_key
      join l_person_address a on a.l_person_id = lp.id
      where link.canonical_person_id = ${personId}
        and lp.source = 'apple_contacts'
      order by lp.source_contact_id asc, a.ordinal asc
    `) as ObservedAddressRow[]

    const observedAddresses: PropertyObservedAddressDto[] = []
    for (const row of observedRows) {
      const address = observedAddress(row)
      const matched = address.addressLine1
        ? await this.findByAddress({
            addressLine1: address.addressLine1,
            municipality: address.city ?? undefined,
            stateOrProvince: address.stateOrProvince ?? undefined,
            postalCode: address.postalCode ?? undefined,
          })
        : null
      const duplicate = observedAddresses.some((candidate) =>
        normalized(candidate.address.addressLine1) === normalized(address.addressLine1) &&
        normalized(candidate.address.city) === normalized(address.city) &&
        normalized(candidate.address.stateOrProvince) === normalized(address.stateOrProvince) &&
        normalized(candidate.address.postalCode) === normalized(address.postalCode),
      )
      if (duplicate) continue
      observedAddresses.push({
        source: row.source,
        sourceLabel: row.source_label,
        sourceKey: `${row.source}:${row.source_account}:${row.source_contact_id}:${row.ordinal}`,
        address,
        matchedPropertyId: matched?.id ?? null,
      })
    }

    return { personId, properties, observedAddresses }
  }

  async upsertForPerson(request: UpsertPropertyForPersonRequest): Promise<PropertyForPersonDto> {
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
    const localName = request.localName === undefined ? current?.localName ?? null : compact(request.localName)
    const legalOwnerName = request.legalOwnerName === undefined
      ? current?.legalOwnerName ?? null
      : compact(request.legalOwnerName)
    const catastroNumber = request.catastroNumber === undefined
      ? current?.catastroNumber ?? null
      : compact(request.catastroNumber)

    if (!current && !compact(address.addressLine1) && !localName) {
      throw new Error('Property requires an address or local name before it can be created.')
    }

    let property: PropertyDto
    if (current) {
      const rows = (await this.execute`
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
        returning
          p.id, p.name, p.legal_owner_name, p.listing_identifier,
          p.status, p.archived_at, p.address_line1,
          p.location, p.street_number, p.street_name, p.unit_number,
          p.city, p.state_or_province, p.neighborhood, p.postal_code,
          p.country, p.iso_country_code
      `) as PropertyRow[]
      if (!rows[0]) throw new Error(`Property not found: ${current.id}`)
      property = toProperty(rows[0])
    } else {
      const rows = (await this.execute`
        insert into property (
          name, legal_owner_name, listing_identifier, address_line1,
          city, state_or_province, neighborhood, postal_code,
          country, iso_country_code, source_type
        ) values (
          ${localName}, ${legalOwnerName}, ${catastroNumber}, ${compact(address.addressLine1)},
          ${compact(address.city)}, ${compact(address.stateOrProvince)},
          ${compact(address.neighborhood)}, ${compact(address.postalCode)},
          ${compact(address.country)}, ${compact(address.isoCountryCode)},
          ${compact(request.sourceType) ?? 'manual'}
        )
        returning
          id, name, legal_owner_name, listing_identifier,
          status, archived_at, address_line1,
          location, street_number, street_name, unit_number,
          city, state_or_province, neighborhood, postal_code,
          country, iso_country_code
      `) as PropertyRow[]
      if (!rows[0]) throw new Error('Property creation returned no row.')
      property = toProperty(rows[0])
    }

    await this.execute`
      insert into person_property (
        person_id, property_id, relation_type, relation_status, source_type, source_key
      ) values (
        ${request.personId}, ${property.id}, ${request.relation},
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

    return { relation: request.relation, relationStatus: request.relationStatus ?? null, property }
  }

  async setDisplayName(request: SetPropertyDisplayNameRequest): Promise<PropertyDto> {
    const rows = (await this.execute`
      update property p
      set name = ${request.displayName}, updated_at = now()
      where p.id = ${request.propertyId}
      returning
        p.id, p.name, to_jsonb(p)->>'legal_owner_name' as legal_owner_name,
        p.listing_identifier, p.status, p.archived_at,
        to_jsonb(p)->>'address_line1' as address_line1,
        p.location, p.street_number, p.street_name, p.unit_number,
        p.city, p.state_or_province, p.neighborhood, p.postal_code,
        to_jsonb(p)->>'country' as country,
        to_jsonb(p)->>'iso_country_code' as iso_country_code
    `) as PropertyRow[]
    if (!rows[0]) throw new Error(`Property not found: ${request.propertyId}`)
    return toProperty(rows[0])
  }

  async setStatus(request: SetPropertyStatusRequest): Promise<PropertyDto> {
    const rows = (await this.execute`
      update property p
      set status = ${request.status}, updated_at = now()
      where p.id = ${request.propertyId}
      returning
        p.id, p.name, to_jsonb(p)->>'legal_owner_name' as legal_owner_name,
        p.listing_identifier, p.status, p.archived_at,
        to_jsonb(p)->>'address_line1' as address_line1,
        p.location, p.street_number, p.street_name, p.unit_number,
        p.city, p.state_or_province, p.neighborhood, p.postal_code,
        to_jsonb(p)->>'country' as country,
        to_jsonb(p)->>'iso_country_code' as iso_country_code
    `) as PropertyRow[]
    if (!rows[0]) throw new Error(`Property not found: ${request.propertyId}`)
    return toProperty(rows[0])
  }
}
