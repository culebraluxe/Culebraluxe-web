// AUTH-02 authoritative Portal access guard.
//
// Authentication is Auth.js. Identity mapping and authorization are Rust-owned.
// TypeScript only translates the Rust decision into the existing redirect
// contract. DEV's deliberate local bypass remains separate and cannot run in
// production.

import { getActingUser } from './get-acting-user'
import { AuthError } from './errors'
import { devAuthLog } from './dev-auth-log'
import type { SessionAdapter } from './session-adapter'
import type { ActingUser, AuthorityCode } from './types'
import { isPortalAuthBypass, portalAuthBypassActor } from './dev-bypass'
import { authorizeApplicationAction } from './security-runtime'

export type PortalAccessResult =
  | { ok: true; actor: ActingUser }
  | { ok: false; redirectTo: '/login' | '/login/unauthorized' }

export async function resolvePortalAccess(
  adapter: SessionAdapter,
  authority: AuthorityCode,
): Promise<PortalAccessResult> {
  if (isPortalAuthBypass()) {
    return { ok: true, actor: portalAuthBypassActor() }
  }

  try {
    const actor = await getActingUser(adapter)
    const decision = await authorizeApplicationAction(authority)
    if (!decision.allowed) {
      devAuthLog('APPLICATION_AUTHORIZATION', 'missing-authority')
      return { ok: false, redirectTo: '/login/unauthorized' }
    }
    devAuthLog('AUTH_PORTAL_AUTHORIZED')
    return { ok: true, actor }
  } catch (error) {
    if (error instanceof AuthError) {
      devAuthLog('APPLICATION_AUTHORIZATION', error.code)
      return {
        ok: false,
        redirectTo:
          error.code === 'unauthenticated' ? '/login' : '/login/unauthorized',
      }
    }
    devAuthLog('APPLICATION_AUTHORIZATION', 'UNKNOWN')
    throw error
  }
}
