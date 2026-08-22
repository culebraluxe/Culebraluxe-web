// AUTH-02 authoritative Portal access guard (server-side).
//
// ONE authoritative seam for Portal route protection: resolve the acting user
// through the canonical projection and require the given authority. Failure is
// mapped to a deterministic redirect target:
//   - unauthenticated            → /login
//   - unmapped / inactive /      → /login/unauthorized
//     missing-authority
//
// The layout/page turns the result into a Next redirect; this module stays
// framework-free so the decision logic is unit-testable with any SessionAdapter
// stub. Business pages must never re-implement this mapping.

import { getActingUser } from './get-acting-user'
import { requireAuthority } from './authority'
import { AuthError } from './errors'
import type { SessionAdapter } from './session-adapter'
import type { ActingUser, AuthorityCode } from './types'

export type PortalAccessResult =
  | { ok: true; actor: ActingUser }
  | { ok: false; redirectTo: '/login' | '/login/unauthorized' }

export async function resolvePortalAccess(
  adapter: SessionAdapter,
  authority: AuthorityCode,
): Promise<PortalAccessResult> {
  try {
    const actor = await getActingUser(adapter)
    requireAuthority(actor, authority)
    return { ok: true, actor }
  } catch (error) {
    if (error instanceof AuthError) {
      return {
        ok: false,
        redirectTo:
          error.code === 'unauthenticated' ? '/login' : '/login/unauthorized',
      }
    }
    throw error
  }
}
