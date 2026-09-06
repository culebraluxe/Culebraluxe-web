import type { ServiceEnvelopeFor, ServiceOperationName } from '../core'

/**
 * Canonical reusable physical-address DTO.
 *
 * This is intentionally transport/form friendly: Apple Contacts can supply the
 * same shape as Property, and Forms can hydrate from Property without learning
 * anything about Apple ODS or Property persistence.
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

export type PropertyDto = {
  id: string
  displayName: string
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
  relation: 'interest'
  relationStatus: string | null
  property: PropertyDto
}

/**
 * Structured address evidence observed outside canonical Property state.
 * Apple Contacts is the first producer. Matching is advisory until a canonical
 * Property is explicitly selected/created.
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
export type SetPropertyDisplayNameRequest = { propertyId: string; displayName: string }
export type SetPropertyStatusRequest = { propertyId: string; status: string }

export const PROPERTY_OPERATIONS = {
  GET: 'property.get',
  FIND_BY_ADDRESS: 'property.findByAddress',
  FOR_PERSON: 'property.forPerson',
  SET_DISPLAY_NAME: 'property.setDisplayName',
  SET_STATUS: 'property.setStatus',
} as const

export type PropertyOperationMap = {
  'property.get': { request: GetPropertyRequest; response: PropertyDto | null }
  'property.findByAddress': { request: FindPropertyByAddressRequest; response: PropertyDto | null }
  'property.forPerson': { request: GetPropertiesForPersonRequest; response: PersonPropertyContextDto }
  'property.setDisplayName': { request: SetPropertyDisplayNameRequest; response: PropertyDto }
  'property.setStatus': { request: SetPropertyStatusRequest; response: PropertyDto }
}

export type PropertyOperationName = ServiceOperationName<PropertyOperationMap>
export type PropertyEnvelope<K extends PropertyOperationName = PropertyOperationName> =
  ServiceEnvelopeFor<PropertyOperationMap, K>
