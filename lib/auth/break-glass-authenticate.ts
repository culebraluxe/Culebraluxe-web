// AUTH-02 break-glass (application root) authentication.
//
// Provider-independent: verifies the submitted secret against the configured
// scrypt hash, then resolves the explicitly configured root app_user through
// the NORMAL role/authority projection. No wildcard bypass — the root user
// must hold the explicit owner role, and normal owner authorities still come
// from the seeded role→authority mapping.

import { getSecurityPrincipal } from '@/db/auth-user'
import { getBreakGlassConfig } from './break-glass-config'
import { verifyBreakGlassSecret } from './break-glass-secret'
import type { ActingUser } from './types'

export type BreakGlassResult =
  | { ok: true; actingUser: ActingUser }
  | { ok: false; reason: 'disabled' | 'invalid' | 'not-owner' | 'unavailable' }

export async function authenticateBreakGlass(
  submittedSecret: string,
): Promise<BreakGlassResult> {
  const config = getBreakGlassConfig()

  if (!config.enabled || !config.appUserId || !config.secretHash) {
    return { ok: false, reason: 'disabled' }
  }

  if (!verifyBreakGlassSecret(submittedSecret, config.secretHash)) {
    return { ok: false, reason: 'invalid' }
  }

  // Resolve through the canonical security projection. An inactive user
  // resolves to null → unavailable.
  const actingUser = await getSecurityPrincipal(config.appUserId)
  if (!actingUser) {
    return { ok: false, reason: 'unavailable' }
  }

  if (
    actingUser.accountType !== 'internal' ||
    !actingUser.roleCodes.includes('owner')
  ) {
    return { ok: false, reason: 'not-owner' }
  }

  return { ok: true, actingUser }
}
