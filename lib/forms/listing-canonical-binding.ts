import 'server-only'

import { randomUUID } from 'node:crypto'

import { sql } from '@/db/client'
import { SqlPersonRepository } from '@/db/person-service-repository'
import { SqlPropertyRepository } from '@/db/property-service-repository'
import { PERSON_OPERATIONS, PersonService } from '@/services/person'
import {
  PROPERTY_OPERATIONS,
  PropertyService,
  type PropertyAddressDto,
} from '@/services/property'
import type {
  ListingCanonicalFieldName,
  ListingCanonicalFields,
  ListingCanonicalSnapshot,
  ListingFieldOrigin,
} from './listing-field-binding'

const personService = new PersonService(new SqlPersonRepository())
const propertyService = new PropertyService(new SqlPropertyRepository())

type ListingFormEvidenceRow = {
  id: string
  property_id: string | null
  field_values: unknown
  updated_at: string | Date
}

type FormEvidence = {
  id: string
  propertyId: string | null
  fields: Record<string, string>
  updatedAt: string
}

function asFieldValues(value: unknown): Record<string, string> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {}
  const result: Record<string, string> = {}
  for (const [key, raw] of Object.entries(value)) {
    if (typeof raw === 'string') result[key] = raw
  }
  return result
}

function iso(value: string | Date): string {
  const date = value instanceof Date ? value : new Date(value)
  return Number.isNaN(date.getTime()) ? String(value) : date.toISOString()
}

function compact(value: string | null | undefined): string {
  return value?.trim() ?? ''
}

function formatAddress(address: PropertyAddressDto | null | undefined): string {
  if (!address) return ''
  return [
    compact(address.addressLine1) || null,
    compact(address.neighborhood) || null,
    compact(address.city) || null,
    [compact(address.stateOrProvince), compact(address.postalCode)].filter(Boolean).join(' ') || null,
    compact(address.country) || null,
  ]
    .filter((value): value is string => Boolean(value))
    .join(', ')
}

async function latestListingEvidence(personId: string): Promise<FormEvidence | null> {
  const rows = (await sql`
    select f.id, f.property_id, f.field_values, f.updated_at
    from document_form_instance f
    left join deal d on d.id = f.deal_id
    where f.template_id = 'LISTING-01'
      and (
        f.person_id = ${personId}
        or d.client_person_id = ${personId}
        or exists (
          select 1
          from deal_participant dp
          where dp.deal_id = f.deal_id
            and dp.person_id = ${personId}
            and dp.active = true
            and dp.role in ('client', 'seller', 'owner')
        )
      )
    order by f.updated_at desc, f.id desc
    limit 1
  `) as ListingFormEvidenceRow[]

  const row = rows[0]
  if (!row) return null
  return {
    id: row.id,
    propertyId: row.property_id ?? null,
    fields: asFieldValues(row.field_values),
    updatedAt: iso(row.updated_at),
  }
}

function choose(
  canonical: string | null | undefined,
  canonicalOrigin: Exclude<ListingFieldOrigin, 'listing_form' | 'empty'>,
  form: string | null | undefined,
): { value: string; origin: ListingFieldOrigin } {
  const canonicalValue = compact(canonical)
  if (canonicalValue) return { value: canonicalValue, origin: canonicalOrigin }
  const formValue = compact(form)
  if (formValue) return { value: formValue, origin: 'listing_form' }
  return { value: '', origin: 'empty' }
}

async function serviceValue<T>(
  promise: Promise<{ ok: true; value: T } | { ok: false; error: { code: string; message: string } }>,
  label: string,
): Promise<T> {
  const result = await promise
  if (!result.ok) throw new Error(`${label}: ${result.error.code} ${result.error.message}`)
  return result.value
}

function serviceContext(actorId: string | null = null) {
  return {
    actor: { id: actorId, kind: actorId ? 'user' as const : 'system' as const },
    correlationId: randomUUID(),
  }
}

export async function loadListingCanonicalSnapshot(
  personId: string,
): Promise<ListingCanonicalSnapshot> {
  const cleanPersonId = personId.trim()
  if (!cleanPersonId) throw new Error('personId is required.')

  const [person, propertyContext, evidence] = await Promise.all([
    serviceValue(
      personService.execute({
        operation: PERSON_OPERATIONS.GET,
        payload: { personId: cleanPersonId },
        context: serviceContext(),
      }),
      'Person lookup failed',
    ),
    serviceValue(
      propertyService.execute({
        operation: PROPERTY_OPERATIONS.FOR_PERSON,
        payload: { personId: cleanPersonId },
        context: serviceContext(),
      }),
      'Property context failed',
    ),
    latestListingEvidence(cleanPersonId),
  ])

  if (!person) throw new Error(`Person not found: ${cleanPersonId}`)

  const legalAddress = propertyContext.properties.find((row) => row.relation === 'legal_address') ?? null
  let physical = propertyContext.properties.find((row) => row.relation === 'physical_property') ?? null

  if (!physical && evidence?.propertyId) {
    const property = await serviceValue(
      propertyService.execute({
        operation: PROPERTY_OPERATIONS.GET,
        payload: { propertyId: evidence.propertyId },
        context: serviceContext(),
      }),
      'Listing Property lookup failed',
    )
    if (property) physical = { relation: 'physical_property', relationStatus: null, property }
  }

  const form = evidence?.fields ?? {}
  const resolved: Record<ListingCanonicalFieldName, { value: string; origin: ListingFieldOrigin }> = {
    sellerName: choose(person.displayName, 'person', form.sellerName),
    sellerResidenceAddress: choose(
      legalAddress ? formatAddress(legalAddress.property.address) : null,
      'property',
      form.sellerResidenceAddress,
    ),
    property: choose(physical?.property.localName, 'property', form.property),
    propertyLocation: choose(
      physical ? formatAddress(physical.property.address) : null,
      'property',
      form.propertyLocation,
    ),
    legalOwnerName: choose(physical?.property.legalOwnerName, 'property', form.legalOwnerName),
    catastroNumber: choose(physical?.property.catastroNumber, 'property', form.catastroNumber),
  }

  const fields = Object.fromEntries(
    Object.entries(resolved).map(([key, value]) => [key, value.value]),
  ) as ListingCanonicalFields
  const origins = Object.fromEntries(
    Object.entries(resolved).map(([key, value]) => [key, value.origin]),
  ) as Record<ListingCanonicalFieldName, ListingFieldOrigin>

  return {
    personId: cleanPersonId,
    personDisplayName: person.displayName,
    formInstanceId: evidence?.id ?? null,
    formUpdatedAt: evidence?.updatedAt ?? null,
    legalAddressPropertyId: legalAddress?.property.id ?? null,
    physicalPropertyId: physical?.property.id ?? evidence?.propertyId ?? null,
    fields,
    origins,
  }
}

export async function saveListingCanonicalFields(
  personId: string,
  fields: ListingCanonicalFields,
  actorId: string | null,
  physicalPropertyId?: string | null,
): Promise<ListingCanonicalSnapshot> {
  const before = await loadListingCanonicalSnapshot(personId)
  const context = () => serviceContext(actorId)
  const sourceKey = before.formInstanceId

  const sellerName = compact(fields.sellerName)
  if (sellerName) {
    await serviceValue(
      personService.execute({
        operation: PERSON_OPERATIONS.SET_DISPLAY_NAME,
        payload: { personId: before.personId, displayName: sellerName },
        context: context(),
      }),
      'Person write-back failed',
    )
  }

  const legalAddress = compact(fields.sellerResidenceAddress)
  if (legalAddress) {
    await serviceValue(
      propertyService.execute({
        operation: PROPERTY_OPERATIONS.UPSERT_FOR_PERSON,
        payload: {
          personId: before.personId,
          relation: 'legal_address',
          propertyId: before.legalAddressPropertyId ?? undefined,
          address: { addressLine1: legalAddress },
          sourceType: 'listing_form',
          sourceKey,
        },
        context: context(),
      }),
      'Legal address write-back failed',
    )
  }

  const physicalHasData = [
    fields.property,
    fields.propertyLocation,
    fields.legalOwnerName,
    fields.catastroNumber,
  ].some((value) => compact(value))
  const selectedPhysicalId = compact(physicalPropertyId) || before.physicalPropertyId || undefined

  if (physicalHasData || selectedPhysicalId) {
    await serviceValue(
      propertyService.execute({
        operation: PROPERTY_OPERATIONS.UPSERT_FOR_PERSON,
        payload: {
          personId: before.personId,
          relation: 'physical_property',
          propertyId: selectedPhysicalId,
          address: { addressLine1: compact(fields.propertyLocation) || null },
          localName: compact(fields.property) || null,
          legalOwnerName: compact(fields.legalOwnerName) || null,
          catastroNumber: compact(fields.catastroNumber) || null,
          sourceType: 'listing_form',
          sourceKey,
        },
        context: context(),
      }),
      'Physical Property write-back failed',
    )
  }

  return loadListingCanonicalSnapshot(before.personId)
}
