import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import type {
  FindPropertyByAddressRequest,
  PersonPropertyContextDto,
  PropertyAddressDto,
  PropertyDto,
  PropertyForPersonDto,
  PropertyObservedAddressDto,
  PropertyRepository,
  SetPropertyDisplayNameRequest,
  SetPropertyStatusRequest,
} from '@/services/property'

type PropertyRow = {
  id: string
  name: string
  status: string
  archived_at: string | Date | null
  location: string | null
  city: string | null
  state_or_province: string | null
  neighborhood: string | null
}

type PropertyForPersonRow = PropertyRow & {
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

function toIso(value: string | Date | null | undefined): string | null {
  if (value == null) return null
  const date = value instanceof Date ? value : new Date(value)
  return Number.isNaN(date.getTime()) ? String(value) : date.toISOString()
}

function canonicalAddress(row: PropertyRow): PropertyAddressDto {
  return {
    addressLine1: row.location,
    city: row.city,
    stateOrProvince: row.state_or_province,
    neighborhood: row.neighborhood,
    postalCode: null,
    country: null,
    isoCountryCode: null,
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

function toProperty(row: PropertyRow): PropertyDto {
  const address = canonicalAddress(row)
  return {
    id: row.id,
    displayName: row.name,
    // Compatibility aliases while V1 callers move onto structured address.
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

/**
 * Production adapter behind PropertyService.
 *
 * It deliberately owns both canonical Property reads and the Apple-address
 * evidence projection used to propose/match Property context for a Person.
 * UI and Forms consume the Property service contract, never l_person_address.
 */
export class SqlPropertyRepository implements PropertyRepository {
  constructor(private readonly execute: QueryExecutor = sql) {}

  async get(propertyId: string): Promise<PropertyDto | null> {
    const rows = (await this.execute`
      select id, name, status, archived_at, location, city, state_or_province, neighborhood
      from property
      where id = ${propertyId}
      limit 1
    `) as PropertyRow[]
    return rows[0] ? toProperty(rows[0]) : null
  }

  async findByAddress(request: FindPropertyByAddressRequest): Promise<PropertyDto | null> {
    const line = request.addressLine1.trim()
    const municipality = request.municipality?.trim() || null
    const state = request.stateOrProvince?.trim() || null
    const rows = (await this.execute`
      select id, name, status, archived_at, location, city, state_or_province, neighborhood
      from property
      where archived_at is null
        and lower(trim(coalesce(location, ''))) = lower(trim(${line}))
        and (${municipality}::text is null or lower(trim(coalesce(city, ''))) = lower(trim(${municipality})))
        and (${state}::text is null or lower(trim(coalesce(state_or_province, ''))) = lower(trim(${state})))
      order by updated_at desc nulls last, id asc
      limit 1
    `) as PropertyRow[]
    return rows[0] ? toProperty(rows[0]) : null
  }

  async forPerson(personId: string): Promise<PersonPropertyContextDto> {
    const canonicalRows = (await this.execute`
      select distinct
        p.id, p.name, p.status, p.archived_at,
        p.location, p.city, p.state_or_province, p.neighborhood,
        pi.status as relation_status
      from property_interest pi
      join property p on p.id = pi.property_id
      where pi.person_id = ${personId}
        and p.archived_at is null
      order by p.name asc, p.id asc
    `) as PropertyForPersonRow[]

    const observedRows = (await this.execute`
      select
        lp.source,
        lp.source_account,
        lp.source_contact_id,
        a.source_label,
        a.street,
        a.city,
        a.state,
        a.postal_code,
        a.country,
        a.iso_country_code,
        a.ordinal
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

    const properties: PropertyForPersonDto[] = canonicalRows.map((row) => ({
      relation: 'interest',
      relationStatus: row.relation_status,
      property: toProperty(row),
    }))

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

      // Exact duplicate source rows are noise to callers; keep the first stable one.
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

  async setDisplayName(request: SetPropertyDisplayNameRequest): Promise<PropertyDto> {
    const rows = (await this.execute`
      update property
      set name = ${request.displayName}, updated_at = now()
      where id = ${request.propertyId}
      returning id, name, status, archived_at, location, city, state_or_province, neighborhood
    `) as PropertyRow[]
    if (!rows[0]) throw new Error(`Property not found: ${request.propertyId}`)
    return toProperty(rows[0])
  }

  async setStatus(request: SetPropertyStatusRequest): Promise<PropertyDto> {
    const rows = (await this.execute`
      update property
      set status = ${request.status}, updated_at = now()
      where id = ${request.propertyId}
      returning id, name, status, archived_at, location, city, state_or_province, neighborhood
    `) as PropertyRow[]
    if (!rows[0]) throw new Error(`Property not found: ${request.propertyId}`)
    return toProperty(rows[0])
  }
}
