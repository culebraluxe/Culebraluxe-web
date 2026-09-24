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
import { SECURITY_OPERATIONS, type SecurityIdentityResolution } from '@/legacy/services/security'
import { appServiceErrorSink } from '@/lib/service-error-sink'

const authEntitlements = new AuthorizationService()

/**
 * Server-side application-security composition used by the login seam.
 * Auth.js proves the provider identity; SecurityService owns its application
 * mapping. The Security service comes from the single shared kernel composition
 * (composeCoreServices), so there is exactly one place the kernel is built.
 * The authorization port is supplied with the same role grants as the Rust kernel.
 */
export const applicationSecurityService = composeCoreServices(
  {
    person: new SqlPersonRepository(),
    firm: new SqlFirmRepository(),
    property: new SqlPropertyRepository(),
    contract: new SqlContractRepository(),
    showing: new SqlShowingRepository(),
    security: new RustSecurityRepository(),
    wbs: new SqlWbsRepository(),
    project: new SqlProjectRepository(),
  },
  { authorization: authEntitlements, errors: appServiceErrorSink() },
).security

export async function resolveApplicationSecurityIdentity(
  provider: string,
  providerSubject: string,
): Promise<SecurityIdentityResolution> {
  const result = await applicationSecurityService.execute({
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
