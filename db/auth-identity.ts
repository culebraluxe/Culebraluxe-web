import { sql } from './client'
import type { ActingUser } from '@/lib/auth/types'
import { getSecurityPrincipal } from './auth-user'
import { PortalWriteError } from '@/lib/portal-write-error'

// AUTH-02 provider-subject resolution service.
//
// (provider, providerSubject) → auth_identity → app_user → ActingUser.
// No fallback email lookup; no fuzzy matching; ambiguity is structurally
// impossible via UNIQUE(provider, provider_subject).

export type IdentityResolution =
  | { kind: 'known'; actingUser: ActingUser }
  | { kind: 'unmapped' }
  | { kind: 'inactive' }

export async function resolveProviderSubject(
  provider: string,
  providerSubject: string,
): Promise<IdentityResolution> {
  const rows = await sql`
    select ai.app_user_id
    from auth_identity ai
    where ai.provider = ${provider}
      and ai.provider_subject = ${providerSubject}
    limit 1
  `

  const row = rows[0] as { app_user_id: string } | undefined
  if (!row) return { kind: 'unmapped' }

  const actingUser = await getSecurityPrincipal(row.app_user_id)
  if (!actingUser) return { kind: 'inactive' }

  return { kind: 'known', actingUser }
}

// AUTH-02 successful-mapped-login timestamp update. Exact provider + subject
// match only; UPDATE (no insert), so it can never accidentally create an
// identity. Does not touch canonical app_user email. Server-side only.
// NOTE: this is a persistent write — do not execute during validation.
export async function touchAuthIdentityLastLogin(
  provider: string,
  providerSubject: string,
): Promise<void> {
  const rows = await sql`
    update auth_identity
    set
      last_login_at = now(),
      updated_at = now()
    where provider = ${provider}
      and provider_subject = ${providerSubject}
    returning id
  `
  if (rows.length === 0) {
    throw new PortalWriteError(
      'not-found',
      'No auth identity mapped for this provider subject.',
    )
  }
}
