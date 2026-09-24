import { randomUUID } from 'node:crypto'

import { SqlContractRepository } from '@/legacy/db/contract-service-repository'
import { SqlFirmRepository } from '@/legacy/db/firm-service-repository'
import { SqlPersonRepository } from '@/legacy/db/person-service-repository'
import { SqlPropertyRepository } from '@/legacy/db/property-service-repository'
import { RustSecurityRepository } from '@/legacy/db/security-service-repository'
import { SqlShowingRepository } from '@/legacy/db/showing-service-repository'
import { SqlWbsRepository } from '@/legacy/db/wbs-service-repository'
import { SqlProjectRepository } from '@/legacy/db/project-service-repository'
import { composeCoreServices } from '@/legacy/services/composition'
import { AuthorizationService } from '@/legacy/services/entitlement'
import type { AuthorizationPort } from '@/legacy/services/core'
import {
  SECURITY_OPERATIONS,
  type SecurityIdentityResolution,
  type SecurityService,
} from '@/legacy/services/security'
import { appServiceErrorSink } from '@/lib/service-error-sink'

let securityRepository: SecurityRepositorySeam | null = null
let authorizationPort: AuthorizationPort | null = null
let composed: SecurityService | null = null

/**
 * The Security service's persistence seam, described through the composition this file ALREADY imports.
 *
 * WHY NOT JUST IMPORT THE INTERFACE. It lives in `legacy/services/security/repository`, and the architecture rule
 * keeps `legacy/` out of the primary website: this file already relies on a suppression for the wiring it genuinely
 * needs, and that list "may only shrink". Importing a type to describe one parameter would spend a suppression on
 * convenience, so the type is derived from the composition instead — and it stays in step with it by construction.
 */
type SecurityRepositorySeam = Parameters<typeof composeCoreServices>[0]['security']

/**
 * TEST SEAM: substitute the Security service's persistence boundary.
 *
 * The same shape as `setDatabaseTestExecutor` in `legacy/db/client.ts`, and for the same reason: the identity
 * mapping is a boundary, and a test that wants to say "this subject maps to this actor" should say it HERE rather
 * than in whatever query language currently implements it. Both used to be SQL; the mapping is now the Rust
 * service's, and a test pinned to SQL stopped saying anything true.
 *
 * What this deliberately does NOT substitute is the Security service itself: the composed service still runs, so
 * its failure mapping stays under test — a repository that throws must still come out as `unmapped`, which is the
 * fail-closed guarantee AUTH-02 rests on.
 *
 * `null` restores the real repository.
 */
export function setSecurityRepositoryForTesting(repository: SecurityRepositorySeam | null): void {
  securityRepository = repository
  composed = null // the next call rebuilds the kernel around the substitute
}

/**
 * TEST SEAM: substitute the AUTHORIZATION PORT, the other half of what the Rust service owns.
 *
 * Identity resolution is an authorized operation (`security.identity.resolve`, granted to the Auth.js edge actor by
 * a bootstrap rule in Rust), so a test of the login seam hits the authority as well as the mapping. In a browser-less
 * Node test there is no Next runtime for the bridge client to load under, so the decision has to be supplied — the
 * same way the repository is.
 *
 * `null` restores the real port.
 */
export function setAuthorizationPortForTesting(port: AuthorizationPort | null): void {
  authorizationPort = port
  composed = null
}

/**
 * Server-side application-security composition used by the login seam.
 * Auth.js proves the provider identity; SecurityService owns its application
 * mapping. The Security service comes from the single shared kernel composition
 * (composeCoreServices), so there is exactly one place the kernel is built.
 *
 * Built LAZILY so the test seam above can take effect; the kernel is still composed in exactly one place.
 */
export function applicationSecurityService(): SecurityService {
  composed ??= composeCoreServices(
    {
      person: new SqlPersonRepository(),
      firm: new SqlFirmRepository(),
      property: new SqlPropertyRepository(),
      contract: new SqlContractRepository(),
      showing: new SqlShowingRepository(),
      security: securityRepository ?? new RustSecurityRepository(),
      wbs: new SqlWbsRepository(),
      project: new SqlProjectRepository(),
    },
    // The authorization port is the Rust security service: this kernel asks for its decisions rather than
    // reproducing them (see AuthorizationService).
    { authorization: authorizationPort ?? new AuthorizationService(), errors: appServiceErrorSink() },
  ).security
  return composed
}

export async function resolveApplicationSecurityIdentity(
  provider: string,
  providerSubject: string,
): Promise<SecurityIdentityResolution> {
  const result = await applicationSecurityService().execute({
    operation: SECURITY_OPERATIONS.RESOLVE_IDENTITY,
    payload: { provider, providerSubject },
    context: {
      actor: { id: null, kind: 'system' },
      correlationId: randomUUID(),
    },
  })

  // Preserve AUTH-02 fail-closed behavior. An unexpected service failure must
  // never turn an unresolved provider identity into an application actor.
  if (!result.ok) return { kind: 'unmapped' }
  return result.value
}
