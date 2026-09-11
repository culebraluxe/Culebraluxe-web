import 'server-only'

import { randomUUID } from 'node:crypto'

import { SqlListingPropertyRepository } from '@/db/listing-property-service-repository'
import { SqlPersonRepository } from '@/db/person-service-repository'
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
import { formatAddressLine } from '@/lib/address-format'
import { appServiceErrorSink } from '@/lib/service-error-sink'
import { formEntitlements } from './form-service-runtime'
import { SqlFormInstanceRepository } from '@/db/form-service-repository'
import { FORM_OPERATIONS, FormService } from '@/services/forms'

const serviceInfrastructure = {
  authorization: formEntitlements,
  errors: appServiceErrorSink(),
}
const personService = new PersonService(new SqlPersonRepository(), serviceInfrastructure)
const propertyService = new PropertyService(new SqlListingPropertyRepository(), serviceInfrastructure)
const formService = new FormService(new SqlFormInstanceRepository(), serviceInfrastructure)

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


function compact(value: string | null | undefined): string {
  return value?.trim() ?? ''
}

// Single-line form fields: a multi-line Apple street must not glue together
// here. Shared with the property repository so both agree (lib/address-format).
function formatAddress(address: PropertyAddressDto | null | undefined): string {
  return formatAddressLine(address)
}

async function latestListingEvidence(personId: string): Promise<FormEvidence | null> {
  const evidence = await serviceValue(
    formService.execute({
      operation: FORM_OPERATIONS.LATEST_EVIDENCE,
      payload: { templateId: 'LISTING-01', personId, roles: ['client', 'seller', 'owner'] },
      context: serviceContext(),
    }),
    'listing evidence',
  )
  if (!evidence) return null
  return {
    id: evidence.formInstanceId,
    propertyId: evidence.propertyId,
    fields: asFieldValues(evidence.fieldValues),
    updatedAt: evidence.updatedAt ?? '',
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
    // "Known as" on the Listing agreement. The property's own name wins; when
    // the record has none — an Apple address, or an entity-owned property —
    // the owner's Company is how the property is known, and failing that the
    // person's name. This is what makes an LLC listing fill itself without
    // anyone typing the entity into the form by hand.
    property: choose(
      physical?.property.localName ?? person.company ?? person.displayName,
      physical?.property.localName || person.company ? 'property' : 'person',
      form.property,
    ),
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
