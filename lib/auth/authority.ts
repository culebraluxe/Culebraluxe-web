// AUTH-03 canonical authority helpers.
//
// Legacy authority codes remain as a compatibility boundary for older Portal
// routes while the service layer uses the skinny EntitlementService. ROOT is
// the deliberate break-glass/operator level in the four-level SecurityService
// model and therefore always passes these legacy guards.

import type { ActingUser, AuthorityCode } from './types'
import { MissingAuthorityError } from './errors'
import { resolveSecurityLevel } from '@/services/security/level'

export function hasAuthority(
  actor: ActingUser,
  authority: AuthorityCode,
): boolean {
  if (resolveSecurityLevel(actor.roleCodes) === 'ROOT') return true
  return actor.authorityCodes.includes(authority)
}

export function requireAuthority(
  actor: ActingUser,
  authority: AuthorityCode,
): void {
  if (!hasAuthority(actor, authority)) {
    throw new MissingAuthorityError(authority)
  }
}
