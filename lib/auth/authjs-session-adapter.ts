// AUTH-02 concrete Auth.js session adapter factory.
//
// Auth.js (next-auth) is NOT installed yet, so this module deliberately does
// NOT import it. It never fakes an authenticated session. Once
// `pnpm add next-auth@beta` is run, replace this stub with the import-ready
// implementation documented in docs/authjs-adapter.md.

import type { SessionAdapter } from './session-adapter'
import type { AuthenticatedIdentity } from './types'

export function createAuthJsSessionAdapter(): SessionAdapter {
  return {
    async getSession(): Promise<AuthenticatedIdentity | null> {
      throw new Error(
        'Auth.js is not installed. Run "pnpm add next-auth@beta" and implement docs/authjs-adapter.md.',
      )
    },
  }
}
