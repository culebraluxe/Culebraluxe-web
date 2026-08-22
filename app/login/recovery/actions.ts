'use server'

import { signIn } from '@/auth'
import { authenticateBreakGlass } from '@/lib/auth/break-glass-authenticate'
import { recordSecurityAuditEvent } from '@/db/security-audit'

export type BreakGlassLoginResult = { ok: boolean }

// Break-glass application-root login. Verifies the submitted secret through the
// canonical authenticateBreakGlass() projection, then establishes an Auth.js
// Credentials session via signIn('break-glass', ...) so the rest of the
// application sees the SAME AuthenticatedIdentity → getActingUser pipeline as a
// normal provider login. Success is audited; failures stay generic — no
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

  try {
    await signIn('break-glass', { secret, redirect: false })
  } catch {
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
