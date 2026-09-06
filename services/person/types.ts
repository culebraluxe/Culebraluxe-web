import type {
  ServiceCapability,
  ServiceEnvelope,
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

export type GetPersonRequest = {
  personId: string
}

export type FindPersonByIdentityRequest = {
  identity: PersonIdentityDto
}

export type SetPersonDisplayNameRequest = {
  personId: string
  displayName: string
}

export type AttachPersonIdentityRequest = {
  personId: string
  identity: PersonIdentityDto
}

export const PERSON_OPERATIONS = {
  GET: {
    name: 'person.get',
    kind: 'query',
    description: 'Return one canonical person by id.',
    authorization: 'person.read',
    idempotent: true,
  },
  FIND_BY_IDENTITY: {
    name: 'person.findByIdentity',
    kind: 'query',
    description: 'Resolve a canonical person by one identity hint.',
    authorization: 'person.read',
    idempotent: true,
  },
  SET_DISPLAY_NAME: {
    name: 'person.setDisplayName',
    kind: 'command',
    description: 'Change the canonical display name for a person.',
    authorization: 'person.write',
  },
  ATTACH_IDENTITY: {
    name: 'person.attachIdentity',
    kind: 'command',
    description: 'Attach an identity to the canonical person.',
    authorization: 'person.write',
  },
} as const satisfies Readonly<Record<string, ServiceCapability>>

export type PersonOperationName =
  (typeof PERSON_OPERATIONS)[keyof typeof PERSON_OPERATIONS]['name']

export type PersonRequestByOperation = {
  'person.get': GetPersonRequest
  'person.findByIdentity': FindPersonByIdentityRequest
  'person.setDisplayName': SetPersonDisplayNameRequest
  'person.attachIdentity': AttachPersonIdentityRequest
}

export type PersonResponseByOperation = {
  'person.get': PersonDto | null
  'person.findByIdentity': PersonDto | null
  'person.setDisplayName': PersonDto
  'person.attachIdentity': PersonIdentityDto
}

export type PersonEnvelope<K extends PersonOperationName = PersonOperationName> =
  ServiceEnvelope<K, PersonRequestByOperation[K]>

export type PersonResponse<K extends PersonOperationName> = PersonResponseByOperation[K]
