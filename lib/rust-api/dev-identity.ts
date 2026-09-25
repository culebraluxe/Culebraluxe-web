import 'server-only'

import { isPortalAuthBypass, portalAuthBypassActor } from '@/lib/auth/dev-bypass'

import type { RustBridgeIdentity } from './contract'

/**
 * Provider identity used only when the deliberate local Portal auth bypass is
 * active.
 *
 * The bypass user is the configured break-glass application user. Break-glass
 * identities use the canonical deterministic subject
 * `break-glass:<app_user_id>`, so DEV can present that identity to the normal
 * Rust resolver without querying auth_identity from TypeScript.
 *
 * Rust still resolves the identity through SecurityService/auth_identity; this
 * does not bypass the Rust security boundary. And isPortalAuthBypass() is
 * fail-closed whenever NODE_ENV or APP_ENV is production.
 */
export async function bypassBridgeIdentity(): Promise<RustBridgeIdentity | null> {
  if (!isPortalAuthBypass()) return null

  const actor = portalAuthBypassActor()
  const identity = {
    provider: 'break-glass',
    providerSubject: `break-glass:${actor.appUserId}`,
  }

  console.warn(
    `[rust-api] PORTAL_AUTH_BYPASS is on: presenting canonical break-glass identity for app_user=${actor.appUserId} so Rust resolves the same application user through its normal security service.`,
  )

  return identity
}
