// AUTH-01 concrete Auth.js session adapter factory.
//
// Bridges the Auth.js JWT session to the provider-neutral application adapter
// contract (AuthenticatedIdentity). Provider cookies/tokens stay inside Auth.js;
// business services only ever consume getActingUser(adapter) downstream.
//
// The provider `sub` (stable, provider-verified) is the identity key — email is
// informational only. `auth` is injectable for targeted tests; the production
// path always uses the @/auth instance.

import { auth } from '@/auth'
import type { SessionAdapter } from './session-adapter'
import type { AuthenticatedIdentity } from './types'

type AuthFn = typeof auth

export function createAuthJsSessionAdapter(
  deps: { auth: AuthFn } = { auth },
): SessionAdapter {
  const getSession = deps.auth
  return {
    async getSession(): Promise<AuthenticatedIdentity | null> {
      const session = await getSession()
      const sub = session?.user?.sub
      const provider = session?.user?.provider
      if (!sub || !provider) return null
      return {
        provider,
        providerSubject: sub,
        providerEmail: session.user.email ?? null,
      }
    },
  }
}
