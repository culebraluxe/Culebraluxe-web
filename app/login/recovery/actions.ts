'use server'

import { authenticateBreakGlass } from '@/lib/auth/break-glass-authenticate'
import { recordSecurityAuditEvent } from '@/db/security-audit'

export type BreakGlassLoginResult = { ok: boolean }

// Break-glass application-root login. Verification + resolution + audit happen
// here; establishing the session cookie is the Auth.js credentials-provider
// integration point (see docs/authjs-adapter.md). Generic failure only — no
// root-identifier enumeration, no reason surfaced.
export async function breakGlassLoginAction(
  secret: string,
): Promise<BreakGlassLoginResult> {
  if (typeof secret !== 'string' || secret.length === 0) {
    return { ok: false }
  }

  const result = await authenticateBreakGlass(secret)
  if (!result.ok) {
    return { ok: false }
  }

  await recordSecurityAuditEvent({
    appUserId: result.actingUser.appUserId,
    eventType: 'BREAK_GLASS_LOGIN_SUCCESS',
    authenticationMethod: 'break-glass',
    metadata: { accountType: result.actingUser.accountType },
  })

  return { ok: true }
}
