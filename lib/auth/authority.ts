// AUTH-03 canonical authority helpers. Exact authority-code match, server-side,
// deterministic. No wildcard owner bypass — owner succeeds only because the
// owner role explicitly owns the seeded authorities. Inactive users/roles
// never authorize (they are already excluded at projection time).

import type { ActingUser, AuthorityCode } from './types'
import { MissingAuthorityError } from './errors'

export function hasAuthority(
  actor: ActingUser,
  authority: AuthorityCode,
): boolean {
  return actor.authorityCodes.includes(authority)
}

export function requireAuthority(
  actor: ActingUser,
  authority: AuthorityCode,
): void {
  if (!hasAuthority(actor, authority)) {
    throw new MissingAuthorityError(authority)
  }
}
