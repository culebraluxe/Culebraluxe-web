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
import type { SessionAdapter } from './session-adapter'
import type { ActingUser, AuthorityCode } from './types'

// ═══════════════════════════════════════════════════════════════════════════
// ⚠️ TEMP STARTUP AUTH BYPASS — REMOVE ME.  (startup dev flow only)
//
// Lets the owner enter the portal immediately (no Google login) so UI/product
// work can continue. Active ONLY when the environment sets PORTAL_AUTH_BYPASS=1
// (set locally in .env.local; NEVER set on production). The authoritative auth
// implementation below is fully preserved and runs whenever the flag is off —
// this block just short-circuits the guard with a synthetic internal actor.
// Delete this block (and the middleware sibling) to restore real auth.
// ═══════════════════════════════════════════════════════════════════════════
const TEMP_BYPASS_ALL_AUTHORITIES: AuthorityCode[] = [
  'portal.read',
  'crm.write',
  'listing.write',
  'deal.read',
  'deal.write',
  'settings.read',
  'settings.manage',
  'external.properties.save',
  'external.deal.read_own',
]

// Real, active app_user so FK-safe writes keep working (break-glass id if set).
const TEMP_BYPASS_APP_USER_ID =
  process.env.AUTH_BREAK_GLASS_APP_USER_ID ??
  'aa06d089-162c-4bef-84ec-a76ee38cc8ad'

export type PortalAccessResult =
  | { ok: true; actor: ActingUser }
  | { ok: false; redirectTo: '/login' | '/login/unauthorized' }

export async function resolvePortalAccess(
  adapter: SessionAdapter,
  authority: AuthorityCode,
): Promise<PortalAccessResult> {
  // ⚠️ TEMP STARTUP AUTH BYPASS — REMOVE ME. See note above.
  if (process.env.PORTAL_AUTH_BYPASS === '1') {
    return {
      ok: true,
      actor: {
        appUserId: TEMP_BYPASS_APP_USER_ID,
        displayName: 'Portal Dev (TEMP BYPASS)',
        email: 'lisa@culebraluxe.com',
        accountType: 'internal',
        roleCodes: ['owner'],
        authorityCodes: [...TEMP_BYPASS_ALL_AUTHORITIES],
        personId: null,
      },
    }
  }

  try {
    const actor = await getActingUser(adapter)
    requireAuthority(actor, authority)
    return { ok: true, actor }
  } catch (error) {
    if (error instanceof AuthError) {
      return {
        ok: false,
        redirectTo:
          error.code === 'unauthenticated' ? '/login' : '/login/unauthorized',
      }
    }
    throw error
  }
}
