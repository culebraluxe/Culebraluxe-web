// AUTH-03 reusable server-side authorization boundary.
//
// Auth.js resolves the actor; Rust SecurityService makes the authorization
// decision. TypeScript does not inspect role, authority, entitlement, or
// security-level data to decide whether the handler may run.

import type { SessionAdapter } from './session-adapter'
import type { ActingUser, AuthorityCode } from './types'
import { getActingUser } from './get-acting-user'
import { MissingAuthorityError } from './errors'
import { isPortalAuthBypass } from './dev-bypass'
import { authorizeApplicationAction } from './security-runtime'

export type AuthorizedHandler<T> = (actor: ActingUser) => Promise<T> | T

export async function runAuthorized<T>(
  adapter: SessionAdapter,
  authority: AuthorityCode,
  handler: AuthorizedHandler<T>,
): Promise<T> {
  const actor = await getActingUser(adapter)
  if (!isPortalAuthBypass()) {
    const decision = await authorizeApplicationAction(authority)
    if (!decision.allowed) throw new MissingAuthorityError(authority)
  }
  return handler(actor)
}
