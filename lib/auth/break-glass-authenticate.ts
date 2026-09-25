// Break-glass authentication at the Auth.js edge.
//
// The secret hash remains server-side configuration. Identity/role resolution is
// delegated to the Rust SecurityService, so recovery follows the same
// auth_identity -> app_user -> roles/entitlements path as Google sign-in.

import { randomUUID } from 'node:crypto'

import {
  buildRustBridgeHeaders,
  resolveInternalApiKey,
  resolveRustApiBaseUrl,
} from '@/lib/rust-api/contract'
import { getBreakGlassConfig } from './break-glass-config'
import { verifyBreakGlassSecret } from './break-glass-secret'
import type { ActingUser } from './types'

export type BreakGlassResult =
  | { ok: true; actingUser: ActingUser }
  | { ok: false; reason: 'disabled' | 'invalid' | 'not-owner' | 'unavailable' }

type RustIdentityResponse =
  | {
      ok: true
      value: {
        kind: 'known'
        actingUser: {
          appUserId: string
          displayName: string
          email: string | null
          accountType: string
          roleCodes: string[]
          authorityCodes: string[]
          entitlementCodes: string[]
          personId: string | null
        }
        securityLevel: string
      } | { kind: 'unmapped' } | { kind: 'inactive' }
    }
  | { ok: false }

function breakGlassSubject(appUserId: string): string {
  return `break-glass:${appUserId}`
}

async function resolveBreakGlassPrincipal(appUserId: string): Promise<RustIdentityResponse> {
  const baseUrl = resolveRustApiBaseUrl(process.env.RUST_API_URL, process.env.NODE_ENV)
  const internalApiKey = resolveInternalApiKey(
    process.env.CULEBRA_INTERNAL_API_KEY,
    process.env.AUTH_SECRET,
  )
  if (!baseUrl || !internalApiKey) return { ok: false }

  const headers = buildRustBridgeHeaders({
    identity: {
      provider: 'break-glass',
      providerSubject: breakGlassSubject(appUserId),
    },
    internalApiKey,
    correlationId: randomUUID(),
  })

  try {
    const response = await fetch(`${baseUrl}/v1/security/identity`, {
      method: 'GET',
      headers,
      cache: 'no-store',
    })
    if (!response.ok) return { ok: false }
    return (await response.json()) as RustIdentityResponse
  } catch {
    return { ok: false }
  }
}

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

  const response = await resolveBreakGlassPrincipal(config.appUserId)
  if (!response.ok || response.value.kind !== 'known') {
    return { ok: false, reason: 'unavailable' }
  }

  const principal = response.value
  if (
    principal.actingUser.appUserId !== config.appUserId ||
    principal.actingUser.accountType !== 'internal' ||
    principal.securityLevel !== 'ROOT'
  ) {
    return { ok: false, reason: 'not-owner' }
  }

  return {
    ok: true,
    actingUser: {
      appUserId: principal.actingUser.appUserId,
      displayName: principal.actingUser.displayName,
      email: principal.actingUser.email,
      accountType: principal.actingUser.accountType,
      roleCodes: principal.actingUser.roleCodes,
      authorityCodes: principal.actingUser.authorityCodes,
      entitlementCodes: principal.actingUser.entitlementCodes,
      personId: principal.actingUser.personId,
    },
  }
}
