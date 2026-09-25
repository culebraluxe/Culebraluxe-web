'use server'

import { signIn } from '@/auth'
import { authenticateBreakGlass } from '@/lib/auth/break-glass-authenticate'

export type BreakGlassLoginResult = { ok: boolean }

// Break-glass application-root login. Verifies the submitted secret through the
// canonical authenticateBreakGlass() projection, then establishes an Auth.js
// Credentials session via signIn('break-glass', ...) so the rest of the
// application sees the SAME AuthenticatedIdentity → getActingUser pipeline as a
// normal provider login. The Rust identity resolution is durably audited by the
// production AuditPort; failures stay generic — no root-identifier enumeration.
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

  return { ok: true }
}
