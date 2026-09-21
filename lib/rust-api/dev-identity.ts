import 'server-only'

import { isPortalAuthBypass, portalAuthBypassActor } from '@/lib/auth/dev-bypass'
import { sql } from '@/db/client'

import type { RustBridgeIdentity } from './contract'

/**
 * The provider identity to use when a developer is running with the auth bypass.
 *
 * WHY THIS EXISTS: the Rust API resolves the caller to an application user through `auth_identity` — the same table
 * production uses — and refuses anything unmapped with 403 AUTH_IDENTITY_UNMAPPED. A bypassed dev session has no
 * Auth.js provider identity, so the bridge would refuse with 401 before the Rust API was even called, and every
 * cut-over screen would be dead in dev. That is what happened.
 *
 * WHAT THIS IS NOT: a fabricated identity. It looks up the REAL `auth_identity` row for the bypass's application user
 * (`AUTH_BREAK_GLASS_APP_USER_ID`) and presents that, so the Rust side resolves it exactly as it resolves a
 * production caller, against the same table, through the same code. Nothing is bypassed on the Rust side — the
 * internal key is still required, the identity is still resolved, and an unmapped identity is still refused.
 *
 * It is guarded by `isPortalAuthBypass()`, which returns false whenever NODE_ENV or APP_ENV is production, so this
 * cannot run in production even if a caller is careless.
 */
export async function bypassBridgeIdentity(): Promise<RustBridgeIdentity | null> {
  if (!isPortalAuthBypass()) return null

  const actor = portalAuthBypassActor()
  const rows = (await sql`
    select provider, provider_subject
    from auth_identity
    where app_user_id = ${actor.appUserId}
      and provider_subject is not null
    order by provider asc
    limit 1
  `) as { provider: string; provider_subject: string }[]

  const identity = rows[0]
  if (!identity) return null

  console.warn(
    `[rust-api] PORTAL_AUTH_BYPASS is on: presenting the real auth_identity for app_user=${actor.appUserId} ` +
      `(provider=${identity.provider}) so the Rust API resolves the same user it would in a signed-in session.`,
  )

  return { provider: identity.provider, providerSubject: identity.provider_subject }
}
