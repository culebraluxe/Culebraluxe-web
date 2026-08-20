// AUTH-03 reusable server-side authorization boundary.
//
//   server action → resolve ActingUser → requireAuthority → call business service
//
// PREPARED, not activated — no existing server action is wired yet, because
// provider auth + owner bootstrap must exist first (otherwise the currently
// unprotected Portal would lock everyone out).

import type { SessionAdapter } from './session-adapter'
import type { ActingUser, AuthorityCode } from './types'
import { getActingUser } from './get-acting-user'
import { requireAuthority as assertAuthority } from './authority'

export type AuthorizedHandler<T> = (actor: ActingUser) => Promise<T> | T

export async function runAuthorized<T>(
  adapter: SessionAdapter,
  authority: AuthorityCode,
  handler: AuthorizedHandler<T>,
): Promise<T> {
  const actor = await getActingUser(adapter)
  assertAuthority(actor, authority)
  return handler(actor)
}
