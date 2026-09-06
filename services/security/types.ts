import type { ActingUser } from '@/lib/auth/types'
import type {
  ServiceEnvelopeFor,
  ServiceOperationName,
} from '../core'
import type { SecurityLevel } from './level'

export type SecurityPrincipal = {
  actingUser: ActingUser
  level: SecurityLevel
}

export type SecurityIdentityResolution =
  | { kind: 'known'; principal: SecurityPrincipal }
  | { kind: 'unmapped' }
  | { kind: 'inactive' }

export type ResolveSecurityIdentityRequest = {
  provider: string
  providerSubject: string
}

export type GetSecurityPrincipalRequest = {
  appUserId: string
}

export const SECURITY_OPERATIONS = {
  RESOLVE_IDENTITY: 'security.resolveIdentity',
  GET_PRINCIPAL: 'security.getPrincipal',
} as const

export type SecurityOperationMap = {
  'security.resolveIdentity': {
    request: ResolveSecurityIdentityRequest
    response: SecurityIdentityResolution
  }
  'security.getPrincipal': {
    request: GetSecurityPrincipalRequest
    response: SecurityPrincipal | null
  }
}

export type SecurityOperationName = ServiceOperationName<SecurityOperationMap>
export type SecurityEnvelope<
  K extends SecurityOperationName = SecurityOperationName,
> = ServiceEnvelopeFor<SecurityOperationMap, K>
