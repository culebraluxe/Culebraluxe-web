import type { ServiceEnvelopeFor, ServiceOperationName } from '../core'

/**
 * Canonical reusable address/place DTO.
 *
 * Address-shaped facts live in Property. Apple Contacts, Forms, Client Lens,
 * and Contracts can all use this shape without learning one another's storage.
 */
export type PropertyAddressDto = {
  addressLine1: string | null
  city: string | null
  stateOrProvince: string | null
  neighborhood: string | null
  postalCode: string | null
  country: string | null
  isoCountryCode: string | null
}

/**
 * Why a Property matters to a Person. The relationship supplies context;
 * Property remains the owner of the address/place truth.
 */
export type PersonPropertyRelation =
  | 'address'
  | 'legal_address'
  | 'physical_property'
  | 'interest'

export type PropertyDto = {
  id: string
  /** Presentation fallback retained for existing callers. */
  displayName: string
  /** Human/local name for the place, e.g. "Casa Luar" or "Sea to Soul". */
  localName: string | null
  /** Name appearing on title, e.g. a person, LLC, or trust. Not a Person identity. */
  legalOwnerName: string | null
  /** Existing Property title/listing identifier used by LISTING-01 as catastro. */
  catastroNumber: string | null
  /** Compatibility aliases for the first service-core experiment. */
  addressLine1: string | null
  municipality: string | null
  /** Canonical structured address for UI/Form/Contract reuse. */
  address: PropertyAddressDto
  status: string
  archivedAt: string | null
}

/** Contextual relationship to a canonical Property; Property identity stays independent. */
export type PropertyForPersonDto = {
  relation: PersonPropertyRelation
  relationStatus: string | null
  property: PropertyDto
}

/**
 * ODS evidence awaiting promotion into Property. This is provenance/input, not
 * a second canonical Address model. Apple Contacts is the first producer.
 */
export type PropertyObservedAddressDto = {
  source: string
  sourceLabel: string | null
  sourceKey: string
  address: PropertyAddressDto
  matchedPropertyId: string | null
}

export type PersonPropertyContextDto = {
  personId: string
  properties: PropertyForPersonDto[]
  observedAddresses: PropertyObservedAddressDto[]
}

export type GetPropertyRequest = { propertyId: string }
export type FindPropertyByAddressRequest = {
  addressLine1: string
  municipality?: string
  stateOrProvince?: string
  postalCode?: string
}
export type GetPropertiesForPersonRequest = { personId: string }

/**
 * Canonical two-way ingress used by Forms and later ODS promotion.
 * Undefined means "leave unchanged"; null explicitly clears an optional qualifier.
 */
export type UpsertPropertyForPersonRequest = {
  personId: string
  relation: PersonPropertyRelation
  propertyId?: string
  relationStatus?: string | null
  address?: Partial<PropertyAddressDto>
  localName?: string | null
  legalOwnerName?: string | null
  catastroNumber?: string | null
  sourceType?: string
  sourceKey?: string | null
}

export type SetPropertyDisplayNameRequest = { propertyId: string; displayName: string }
export type SetPropertyStatusRequest = { propertyId: string; status: string }

export const PROPERTY_OPERATIONS = {
  GET: 'property.get',
  FIND_BY_ADDRESS: 'property.findByAddress',
  FOR_PERSON: 'property.forPerson',
  UPSERT_FOR_PERSON: 'property.upsertForPerson',
  SET_DISPLAY_NAME: 'property.setDisplayName',
  SET_STATUS: 'property.setStatus',
} as const

export type PropertyOperationMap = {
  'property.get': { request: GetPropertyRequest; response: PropertyDto | null }
  'property.findByAddress': { request: FindPropertyByAddressRequest; response: PropertyDto | null }
  'property.forPerson': { request: GetPropertiesForPersonRequest; response: PersonPropertyContextDto }
  'property.upsertForPerson': { request: UpsertPropertyForPersonRequest; response: PropertyForPersonDto }
  'property.setDisplayName': { request: SetPropertyDisplayNameRequest; response: PropertyDto }
  'property.setStatus': { request: SetPropertyStatusRequest; response: PropertyDto }
}

export type PropertyOperationName = ServiceOperationName<PropertyOperationMap>
export type PropertyEnvelope<K extends PropertyOperationName = PropertyOperationName> =
  ServiceEnvelopeFor<PropertyOperationMap, K>
