// AUTH-02 authoritative Portal access guard (server-side).
//
// ONE authoritative seam for Portal route protection: resolve the acting user
// through the canonical projection and require the given authority. Failure is
// mapped to a deterministic redirect target:
//   - unauthenticated            → /login
//   - unmapped / inactive /      → /login/unauthorized
//     missing-authority
//
// The layout/page turns the result into a Next redirect; this module stays
// framework-free so the decision logic is unit-testable with any SessionAdapter
// stub. Business pages must never re-implement this mapping.

import { getActingUser } from './get-acting-user'
import { requireAuthority } from './authority'
import { AuthError } from './errors'
import { devAuthLog } from './dev-auth-log'
import type { SessionAdapter } from './session-adapter'
import type { ActingUser, AuthorityCode } from './types'
import { isPortalAuthBypass, portalAuthBypassActor } from './dev-bypass'

export type PortalAccessResult =
  | { ok: true; actor: ActingUser }
  | { ok: false; redirectTo: '/login' | '/login/unauthorized' }

export async function resolvePortalAccess(
  adapter: SessionAdapter,
  authority: AuthorityCode,
): Promise<PortalAccessResult> {
  // ⚠️ TEMP STARTUP AUTH BYPASS — REMOVE ME. See note above.
  if (isPortalAuthBypass()) {
    return { ok: true, actor: portalAuthBypassActor() }
  }

  try {
    const actor = await getActingUser(adapter)
    requireAuthority(actor, authority)
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
