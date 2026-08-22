// AUTH-02 serializable actor snapshot for client components.
//
// Server components pass this narrow projection (never the full ActingUser or
// the session) into client components purely so UI can hide controls. UI
// hiding is cosmetic — every server boundary still re-checks authorities.

import type { ActingUser, PortalActorSnapshot } from './types'

export function toPortalActorSnapshot(
  actor: ActingUser,
): PortalActorSnapshot {
  return {
    displayName: actor.displayName,
    accountType: actor.accountType,
    authorityCodes: actor.authorityCodes,
  }
}
