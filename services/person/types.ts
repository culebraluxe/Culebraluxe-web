import type {
  ServiceEnvelopeFor,
  ServiceOperationName,
} from '../core'

export type PersonDto = {
  id: string
  displayName: string
  status: string
  archivedAt: string | null
}

export type PersonIdentityDto = {
  kind: 'phone' | 'email' | 'external'
  value: string
  sourceSystem?: string
  isPrimary: boolean
}

export type GetPersonRequest = { personId: string }
export type FindPersonByIdentityRequest = { identity: PersonIdentityDto }
export type SetPersonDisplayNameRequest = { personId: string; displayName: string }
export type AttachPersonIdentityRequest = { personId: string; identity: PersonIdentityDto }

export const PERSON_OPERATIONS = {
  GET: 'person.get',
  FIND_BY_IDENTITY: 'person.findByIdentity',
  SET_DISPLAY_NAME: 'person.setDisplayName',
  ATTACH_IDENTITY: 'person.attachIdentity',
} as const

export type PersonOperationMap = {
  'person.get': { request: GetPersonRequest; response: PersonDto | null }
  'person.findByIdentity': { request: FindPersonByIdentityRequest; response: PersonDto | null }
  'person.setDisplayName': { request: SetPersonDisplayNameRequest; response: PersonDto }
  'person.attachIdentity': { request: AttachPersonIdentityRequest; response: PersonIdentityDto }
}

export type PersonOperationName = ServiceOperationName<PersonOperationMap>
export type PersonEnvelope<K extends PersonOperationName = PersonOperationName> =
  ServiceEnvelopeFor<PersonOperationMap, K>
