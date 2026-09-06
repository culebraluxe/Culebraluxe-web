import type { Client } from '@/lib/portal/types'
import {
  emptyDealFacts,
  prefillFieldValues,
} from '@/lib/forms/offer-letter-data'
import type { TemplateDefinition } from '@/lib/forms/template-types'
import type {
  PersonPropertyContextDto,
  PropertyAddressDto,
  PropertyForPersonDto,
} from '@/services/property'
import type {
  FormLensFieldModel,
  FormLensFieldOrigin,
} from './model'

const TEMPLATE_DEFAULT_FIELDS = new Set([
  'sellerCivilStatus',
  'brokerName',
  'commission',
  'startDate',
  'endDate',
  'listingType',
])

export function formatPropertyAddress(address: PropertyAddressDto): string {
  return [
    address.addressLine1,
    address.neighborhood,
    address.city,
    [address.stateOrProvince, address.postalCode].filter(Boolean).join(' ') || null,
    address.country,
  ]
    .filter((value): value is string => Boolean(value?.trim()))
    .join(', ')
}

function chooseListingProperty(
  context: PersonPropertyContextDto | null,
  requestedPropertyId: string | null,
): PropertyForPersonDto | null {
  const properties = context?.properties ?? []
  if (requestedPropertyId) {
    const requested = properties.find((item) => item.property.id === requestedPropertyId)
    if (requested) return requested
  }

  return (
    properties.find((item) => item.relation === 'physical_property') ??
    properties.find((item) => item.relation === 'interest') ??
    null
  )
}

function chooseResidenceProperty(
  context: PersonPropertyContextDto | null,
): PropertyForPersonDto | null {
  const properties = context?.properties ?? []
  return (
    properties.find((item) => item.relation === 'legal_address') ??
    properties.find((item) => item.relation === 'address') ??
    null
  )
}

function fieldOrigin(
  name: string,
  value: string,
  hasClient: boolean,
  hasListingProperty: boolean,
  hasResidenceProperty: boolean,
): FormLensFieldOrigin {
  if (name === 'sellerName') return hasClient && value ? 'person' : 'unresolved'
  if (name === 'sellerResidenceAddress') {
    return hasResidenceProperty && value ? 'property_relation' : 'unresolved'
  }
  if (name === 'property' || name === 'propertyLocation') {
    return hasListingProperty && value ? 'property' : 'unresolved'
  }
  if (TEMPLATE_DEFAULT_FIELDS.has(name) && value) return 'template_default'
  return value ? 'manual' : 'unresolved'
}

export type ListingAgreementProjection = {
  selectedPropertyId: string | null
  fields: FormLensFieldModel[]
}

/**
 * Pure Listing Agreement composition seam.
 *
 * Person owns identity. Property owns address/place truth. The form only maps
 * those DTOs into LISTING-01 field names; it does not learn SQL, Apple ODS, or
 * the legacy Deal projection. Unresolved facts remain blank and editable.
 */
export function projectListingAgreement(
  template: TemplateDefinition,
  client: Client | null,
  propertyContext: PersonPropertyContextDto | null,
  requestedPropertyId: string | null,
): ListingAgreementProjection {
  const listingProperty = chooseListingProperty(propertyContext, requestedPropertyId)
  const residenceProperty = chooseResidenceProperty(propertyContext)

  const facts = emptyDealFacts()
  facts.personDisplayName = client?.displayName ?? null
  facts.clientName = client?.displayName ?? null
  facts.propertyName = listingProperty
    ? listingProperty.property.localName ?? listingProperty.property.displayName
    : null
  facts.propertyLabel = facts.propertyName
  facts.propertyLocation = listingProperty
    ? formatPropertyAddress(listingProperty.property.address) || null
    : null

  const values = prefillFieldValues(template, facts)
  if (residenceProperty) {
    values.sellerResidenceAddress = formatPropertyAddress(residenceProperty.property.address) || ''
  }

  const fields = template.fields.map<FormLensFieldModel>((field) => {
    const value = values[field.name] ?? ''
    return {
      name: field.name,
      label: field.label,
      type: field.type,
      required: field.required,
      options: field.options ?? [],
      value,
      origin: fieldOrigin(
        field.name,
        value,
        Boolean(client),
        Boolean(listingProperty),
        Boolean(residenceProperty),
      ),
    }
  })

  // LISTING-01 v4 predates the canonical legal-owner qualifier. The sidecar
  // exposes it explicitly now so we can prove the Property binding before a
  // later immutable template version adopts the field.
  if (!fields.some((field) => field.name === 'legalOwnerName')) {
    fields.push({
      name: 'legalOwnerName',
      label: 'Legal Owner Name',
      type: 'text',
      required: false,
      options: [],
      value: listingProperty?.property.legalOwnerName ?? '',
      origin: listingProperty?.property.legalOwnerName ? 'property' : 'unresolved',
    })
  }

  return {
    selectedPropertyId: listingProperty?.property.id ?? null,
    fields,
  }
}
