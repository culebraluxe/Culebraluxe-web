// AUTH-02 coarse authority snapshot for the middleware gate.
//
// Resolves the SAME canonical projection as getActingUser (auth_identity →
// app_user → roles → authorities) and returns only the authority codes. It is
// invoked exclusively at sign-in (Auth.js jwt callback, Node runtime) so the
// Edge middleware never touches the database; the snapshot rides in the JWT.
//
// The snapshot is a coarse, deny-by-default convenience for the cheap Edge
// gate. It is NEVER authoritative: revocations take effect immediately at the
// server-side layout/action guards (which re-resolve via getActingUser), and a
// stale snapshot can only over-deny until the next sign-in (7-day max session).

import { resolveProviderSubject } from '@/db/auth-identity'

export async function getSessionAuthoritySnapshot(
  provider: string,
  providerSubject: string,
): Promise<string[] | null> {
  if (!provider || !providerSubject) return null
  const resolution = await resolveProviderSubject(provider, providerSubject)
  if (resolution.kind !== 'known') return null
  return resolution.actingUser.authorityCodes
}
