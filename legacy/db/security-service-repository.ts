import { rustApiRead, rustApiResolveIdentity } from '@/lib/rust-api/client'
import type { ActingUser } from '@/lib/auth/types'
import type {
  SecurityRepository,
  SecurityRepositoryIdentityResolution,
} from '@/legacy/services/security'

/** The actor as `/v1/whoami` answers it — the TypeScript boundary's field names, from the Rust service. */
type RustActor = {
  appUserId: string
  displayName: string
  email: string | null
  accountType: string
  roleCodes: string[]
  authorityCodes: string[]
  entitlementCodes: string[]
  personId: string | null
}

function toActingUser(actor: RustActor): ActingUser {
  return {
    appUserId: actor.appUserId,
    displayName: actor.displayName,
    email: actor.email,
    // The boundary type narrows this to 'internal' | 'external'; the service is the authority on which it is, so
    // this trusts the answer rather than second-guessing it.
    accountType: actor.accountType as ActingUser['accountType'],
    roleCodes: actor.roleCodes,
    authorityCodes: actor.authorityCodes,
    entitlementCodes: actor.entitlementCodes,
    personId: actor.personId,
  }
}

/**
 * Security repository adapter over the Rust SecurityService.
 *
 * THE GOOGLE MAPPING LIVES IN RUST. Auth.js still proves the provider identity — it has to, it is the thing
 * holding the OAuth redirect — but the question "which application user is this Google subject, and what may they
 * do?" is answered by one implementation: Rust's `security.resolveIdentity`, which owns the identity lookup, the
 * role projection, the level hierarchy and the entitlement grants. This adapter is transport.
 *
 * WHY IT WAS NOT ALWAYS. It used to run the same two AUTH-02 queries itself (`auth_identity` → `app_user` →
 * `security_role`), which meant the mapping existed twice: a TS copy and a Rust copy that could disagree about
 * roles, levels, entitlements, or how quickly a revocation takes effect. The packet for this work said
 * "Auth.js proves the provider identity; SecurityService owns its application mapping" — this is that sentence
 * made true.
 *
 * NO DATABASE ACCESS HERE. `legacy/db/` is still where SQL belongs, but not for this: the projection is a service
 * decision, and a second copy of it is a second authority.
 */
export class RustSecurityRepository implements SecurityRepository {
  async resolveProviderSubject(
    provider: string,
    providerSubject: string,
  ): Promise<SecurityRepositoryIdentityResolution> {
    try {
      const resolution = await rustApiResolveIdentity(provider, providerSubject)
      if (resolution.kind === 'known') {
        return { kind: 'known', actingUser: toActingUser(resolution.actingUser) }
      }
      if (resolution.kind === 'inactive') return { kind: 'inactive' }
      return { kind: 'unmapped' }
    } catch {
      // FAIL CLOSED, as AUTH-02 has always failed closed: an authority that cannot answer has not mapped anybody.
      // "We could not check" is not "it is fine", and it is certainly not an application actor.
      return { kind: 'unmapped' }
    }
  }

  async getPrincipal(appUserId: string): Promise<ActingUser | null> {
    try {
      const result = await rustApiRead<RustActor>('/v1/whoami')
      const actor = toActingUser(result.value)
      // THE CALLER'S OWN PRINCIPAL ONLY. Every assertion in this adapter resolves the session identity it is called
      // with, so answering a request for somebody *else's* principal would be inventing an answer from a different
      // question. A mismatch is refused rather than approximated.
      return actor.appUserId === appUserId ? actor : null
    } catch {
      return null
    }
  }
}

