import type { ServiceEnvelopeFor, ServiceOperationName } from '../core'

export type PropertyDto = {
  id: string
  displayName: string
  addressLine1: string | null
  municipality: string | null
  status: string
  archivedAt: string | null
}

export type GetPropertyRequest = { propertyId: string }
export type FindPropertyByAddressRequest = { addressLine1: string; municipality?: string }
export type SetPropertyDisplayNameRequest = { propertyId: string; displayName: string }
export type SetPropertyStatusRequest = { propertyId: string; status: string }

export const PROPERTY_OPERATIONS = {
  GET: 'property.get',
  FIND_BY_ADDRESS: 'property.findByAddress',
  SET_DISPLAY_NAME: 'property.setDisplayName',
  SET_STATUS: 'property.setStatus',
} as const

export type PropertyOperationMap = {
  'property.get': { request: GetPropertyRequest; response: PropertyDto | null }
  'property.findByAddress': { request: FindPropertyByAddressRequest; response: PropertyDto | null }
  'property.setDisplayName': { request: SetPropertyDisplayNameRequest; response: PropertyDto }
  'property.setStatus': { request: SetPropertyStatusRequest; response: PropertyDto }
}

export type PropertyOperationName = ServiceOperationName<PropertyOperationMap>
export type PropertyEnvelope<K extends PropertyOperationName = PropertyOperationName> =
  ServiceEnvelopeFor<PropertyOperationMap, K>
