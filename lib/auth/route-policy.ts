// AUTH-03 centralized coarse Portal route policy. PREPARED, not activated —
// no middleware applies this until provider auth + owner bootstrap exist.
//
// Rules are coarse by design. External client accounts do not gain portal.read
// merely by being authenticated.

import type { AuthorityCode } from './types'

export type RoutePolicy = {
  pathPrefix: string
  authorities: AuthorityCode[]
}

// Most-specific prefix first.
export const PORTAL_ROUTE_POLICY: RoutePolicy[] = [
  { pathPrefix: '/portal/settings', authorities: ['settings.read'] },
  { pathPrefix: '/portal', authorities: ['portal.read'] },
]

// Paths that must remain public regardless of portal protection (login and
// auth callback routes). PREPARED, not activated.
const PUBLIC_AUTH_PATH_PREFIXES = ['/login', '/api/auth']

export function isPublicAuthPath(pathname: string): boolean {
  return PUBLIC_AUTH_PATH_PREFIXES.some(
    (prefix) => pathname === prefix || pathname.startsWith(`${prefix}/`),
  )
}

export function authoritiesForPath(pathname: string): AuthorityCode[] | null {
  for (const policy of PORTAL_ROUTE_POLICY) {
    if (pathname === policy.pathPrefix || pathname.startsWith(`${policy.pathPrefix}/`)) {
      return policy.authorities
    }
  }
  return null
}
