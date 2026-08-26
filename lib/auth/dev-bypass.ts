// TEMP STARTUP AUTH BYPASS — local/dev only (PORTAL_AUTH_BYPASS=1).
// Shared so layout guards AND server actions (runAuthorized → getActingUser)
// see the same synthetic actor. Never enable on production.

import type { ActingUser, AuthorityCode } from './types'

const TEMP_BYPASS_ALL_AUTHORITIES: AuthorityCode[] = [
  'portal.read',
  'crm.write',
  'listing.write',
  'deal.read',
  'deal.write',
  'settings.read',
  'settings.manage',
  'tech.access',
  'external.properties.save',
  'external.deal.read_own',
]

const TEMP_BYPASS_APP_USER_ID =
  process.env.AUTH_BREAK_GLASS_APP_USER_ID ??
  'aa06d089-162c-4bef-84ec-a76ee38cc8ad'

export function isPortalAuthBypass(): boolean {
  // Dynamic lookup so Next does not compile this to an empty string.
  // node:test loads .env.local (which sets the flag); never bypass there.
  if (process.env['PORTAL_AUTH_BYPASS'] !== '1') return false
  if (process.env['NODE_TEST_CONTEXT']) return false
  // HARDEN-01/HARDEN-04 — fail closed on production: a production deployment
  // must NEVER honor the DEV bypass, even if the flag is accidentally set.
  // Production auth configuration mismatch must deny, not silently open access.
  if (process.env['NODE_ENV'] === 'production') return false
  if (process.env['APP_ENV'] === 'production') return false
  return true
}

export function portalAuthBypassActor(): ActingUser {
  return {
    appUserId: TEMP_BYPASS_APP_USER_ID,
    // Cosmetic display name only — the obsolete dev label was removed from the
    // user-facing surface. The bypass gate, identity, and authorities are unchanged.
    displayName: 'CulebraLuxe Portal',
    email: 'lisa@culebraluxe.com',
    accountType: 'internal',
    roleCodes: ['owner'],
    authorityCodes: [...TEMP_BYPASS_ALL_AUTHORITIES],
    personId: null,
  }
}
