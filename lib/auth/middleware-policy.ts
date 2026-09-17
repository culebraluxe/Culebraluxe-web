// AUTH-02 pure Portal route-policy decision for the Edge middleware gate.
//
// The middleware is the CHEAP first gate only (no DB — the Edge runtime cannot
// reliably reach the Neon pool for the authority projection). The authoritative
// check is server-side: the Portal layout calls getActingUser + requireAuthority
// and redirects on AuthError.
//
// This function performs NO capability check: nothing stamps a capability claim
// into the JWT, so the gate decides on authentication alone. A capability check
// may only return once a writer exists (AUTH-CAPABILITIES-02).
//
// Decisions:
//   - non-Portal path        → pass through
//   - unauthenticated Portal → /login
//   - authenticated Portal   → pass through (authority is decided server-side)

import { authoritiesForPath } from './route-policy'

export type MiddlewareSessionSnapshot = {
  authenticated: boolean
}

export type MiddlewareDecision =
  | { kind: 'next' }
  | { kind: 'redirect'; to: '/login' | '/login/unauthorized' }

export function decidePortalMiddleware(
  pathname: string,
  session: MiddlewareSessionSnapshot,
): MiddlewareDecision {
  const authorities = authoritiesForPath(pathname)
  if (!authorities) return { kind: 'next' }

  if (!session.authenticated) {
    return { kind: 'redirect', to: '/login' }
  }

  return { kind: 'next' }
}
