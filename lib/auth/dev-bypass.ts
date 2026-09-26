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

// WHO THE DEV BYPASS ACTS AS — flippable, and the SAME user on both sides.
//
// `PORTAL_AUTH_BYPASS_AS=root` (the default) acts as the DEV database's ROOT user; `business_power` acts as the
// business account. The app user id is what Rust resolves (through the break-glass identity, see
// lib/rust-api/dev-identity.ts), and the role here is what the nav snapshot claims, so the two can no longer disagree —
// they did: the snapshot said ROOT while Rust resolved a business_power user, so TECH showed in the nav and then
// answered FORBIDDEN. Override either account per machine with PORTAL_AUTH_BYPASS_ROOT_USER_ID /
// PORTAL_AUTH_BYPASS_BUSINESS_USER_ID. Each needs a `break-glass:<id>` row in the DEV auth_identity table.
type BypassPersona = { appUserId: string; roleCodes: string[]; securityLevel: 'ROOT' | 'BUSINESS_POWER_USER' }

function bypassPersona(): BypassPersona {
  if (process.env['PORTAL_AUTH_BYPASS_AS'] === 'business_power') {
    return {
      appUserId:
        process.env['PORTAL_AUTH_BYPASS_BUSINESS_USER_ID'] ??
        process.env['AUTH_BREAK_GLASS_APP_USER_ID'] ??
        'aa06d089-162c-4bef-84ec-a76ee38cc8ad',
      roleCodes: ['business_power'],
      securityLevel: 'BUSINESS_POWER_USER',
    }
  }
  return {
    appUserId: process.env['PORTAL_AUTH_BYPASS_ROOT_USER_ID'] ?? '1fc6dc61-d842-4d29-a20b-93c79e07c718',
    roleCodes: ['root'],
    securityLevel: 'ROOT',
  }
}

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
  const persona = bypassPersona()
  return {
    appUserId: persona.appUserId,
    // Cosmetic display name only — the obsolete dev label was removed from the
    // user-facing surface. The bypass gate, identity, and authorities are unchanged.
    displayName: 'CulebraLuxe Portal',
    email: 'lisa@culebraluxe.com',
    accountType: 'internal',
    roleCodes: persona.roleCodes,
    authorityCodes: [...TEMP_BYPASS_ALL_AUTHORITIES],
    personId: null,
    securityLevel: persona.securityLevel,
  }
}
