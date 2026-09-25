// AUTH-02 serializable actor snapshot for client components.
//
// Rust already resolved the security level. This projection only serializes it
// for cosmetic UI hiding; it does not infer roles or make an authorization
// decision.

import type { ActingUser, PortalActorSnapshot } from './types'

export function toPortalActorSnapshot(
  actor: ActingUser,
): PortalActorSnapshot {
  return {
    displayName: actor.displayName,
    accountType: actor.accountType,
    securityLevel: actor.securityLevel ?? 'GUEST',
    authorityCodes: actor.authorityCodes,
    entitlementCodes: actor.entitlementCodes ?? [],
  }
}
