// AUTH-03 single Portal session adapter instance + action/route guards.
//
// One reusable enforcement seam for every authenticated Portal server action
// and upload route:
//
//   server action → getPortalSessionAdapter() → runAuthorized → business service
//   upload route  → guardPortalUpload()       → multipart/write work
//
// Production code calls these with no adapter argument and always resolves
// through the ONE portalSessionAdapter instance (Auth.js). The optional
// adapter parameter is the same dependency-injection affordance as
// createAuthJsSessionAdapter({ auth }) — it exists so targeted tests can stub
// the session without touching the singleton.
//
// IMPORTANT: a Portal action/route must resolve the actor BEFORE any write.
// Client-side button hiding is cosmetic; this server-side resolution +
// exact-authority assertion is the actual security boundary.

import { createAuthJsSessionAdapter } from './authjs-session-adapter'
import { getActingUser } from './get-acting-user'
import { requireAuthority } from './authority'
import { AuthError } from './errors'
import type { SessionAdapter } from './session-adapter'
import type { ActingUser, AuthorityCode } from './types'

const portalSessionAdapter: SessionAdapter = createAuthJsSessionAdapter()

// The single Portal session adapter instance. runAuthorized(adapter, ...)
// consumes the adapter and resolves the actor internally, so this is what
// server actions should pass to the enforcement seam.
export function getPortalSessionAdapter(): SessionAdapter {
  return portalSessionAdapter
}

export function getPortalActingUser(
  adapter: SessionAdapter = portalSessionAdapter,
): Promise<ActingUser> {
  return getActingUser(adapter)
}

export type PortalUploadGuardResult =
  | { ok: true; actor: ActingUser }
  | { ok: false; status: 401 | 403; error: string }

// Fail-closed guard for Route Handlers (media upload endpoints). Must run
// before any multipart parsing or write work. AuthError is mapped to a
// deterministic JSON status (401 unauthenticated, 403 everything else);
// any other failure is rethrown so the route can fail closed with a 500.
export async function guardPortalUpload(
  authority: AuthorityCode,
  adapter: SessionAdapter = portalSessionAdapter,
): Promise<PortalUploadGuardResult> {
  try {
    const actor = await getActingUser(adapter)
    requireAuthority(actor, authority)
    return { ok: true, actor }
  } catch (error) {
    if (error instanceof AuthError) {
      return {
        ok: false,
        status: error.code === 'unauthenticated' ? 401 : 403,
        error: 'Unauthorized.',
      }
    }
    throw error
  }
}
