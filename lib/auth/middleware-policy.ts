// AUTH-02 pure Portal route-policy decision for the Edge middleware gate.
//
// The middleware is the CHEAP first gate only (no DB — the Edge runtime cannot
// reliably reach the Neon pool for the authority projection). The authoritative
// check is server-side: the Portal layout calls getActingUser + requireAuthority
// and redirects on AuthError.
//
// Decisions:
//   - non-Portal path            → pass through
//   - unauthenticated Portal     → /login
//   - authenticated but missing  → /login/unauthorized
//     the required authority
//   - authenticated, capability  → pass through
//     snapshot present and ok
//   - authenticated, NO snapshot → pass through (session minted before the
//     snapshot claim existed, or unmapped at sign-in). The authoritative
//     server-side guard still decides — never redirect on a missing claim.

import { authoritiesForPath } from './route-policy'

export type MiddlewareSessionSnapshot = {
  authenticated: boolean
  capabilities?: string[] | null
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

  // Only enforce when the session carries a capability snapshot. Absence means
  // "unknown to the cheap gate" — pass through and let the server-side guard
  // decide authoritatively.
  if (Array.isArray(session.capabilities)) {
    const missing = authorities.some(
      (authority) => !session.capabilities!.includes(authority),
    )
    if (missing) {
      return { kind: 'redirect', to: '/login/unauthorized' }
    }
  }

  return { kind: 'next' }
}
