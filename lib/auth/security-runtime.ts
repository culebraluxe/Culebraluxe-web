import { randomUUID } from 'node:crypto'

import { SqlSecurityRepository } from '@/db/security-service-repository'
import { EntitlementService } from '@/services/entitlement'
import {
  SECURITY_OPERATIONS,
  SecurityService,
  type SecurityIdentityResolution,
} from '@/services/security'

const authEntitlements = new EntitlementService()

/**
 * Server-side application-security composition used by the login seam.
 * Auth.js proves the provider identity; SecurityService owns its application
 * mapping. EntitlementService is an explicit open stub until the entitlement
 * story defines fine-grained policy.
 */
export const applicationSecurityService = new SecurityService(
  new SqlSecurityRepository(),
  { authorization: authEntitlements },
)

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
