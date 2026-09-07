import { randomUUID } from 'node:crypto'

import { SqlContractRepository } from '@/db/contract-service-repository'
import { SqlFirmRepository } from '@/db/firm-service-repository'
import { SqlPersonRepository } from '@/db/person-service-repository'
import { SqlPropertyRepository } from '@/db/property-service-repository'
import { SqlSecurityRepository } from '@/db/security-service-repository'
import { SqlShowingRepository } from '@/db/showing-service-repository'
import { composeCoreServices } from '@/services/composition'
import { AuthorizationService } from '@/services/entitlement'
import { SqlAuthorizationPolicyProvider } from '@/services/entitlement/db-authorization-policy-provider'
import { SECURITY_OPERATIONS, type SecurityIdentityResolution } from '@/services/security'

const authEntitlements = new AuthorizationService(new SqlAuthorizationPolicyProvider())

/**
 * Server-side application-security composition used by the login seam.
 * Auth.js proves the provider identity; SecurityService owns its application
 * mapping. The Security service comes from the single shared kernel composition
 * (composeCoreServices), so there is exactly one place the kernel is built.
 * EntitlementService is an explicit open stub until the entitlement story
 * defines fine-grained policy.
 */
export const applicationSecurityService = composeCoreServices(
  {
    person: new SqlPersonRepository(),
    firm: new SqlFirmRepository(),
    property: new SqlPropertyRepository(),
    contract: new SqlContractRepository(),
    showing: new SqlShowingRepository(),
    security: new SqlSecurityRepository(),
  },
  { authorization: authEntitlements },
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
