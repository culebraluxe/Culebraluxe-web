import 'server-only'

import { SqlContractRepository } from '@/db/contract-service-repository'
import { SqlFirmRepository } from '@/db/firm-service-repository'
import { SqlPersonRepository } from '@/db/person-service-repository'
import { SqlPropertyRepository } from '@/db/property-service-repository'
import { SqlSecurityRepository } from '@/db/security-service-repository'
import { SqlShowingRepository } from '@/db/showing-service-repository'
import { composeCoreServices } from '@/services/composition'
import { AuthorizationService } from '@/services/entitlement'
import { SqlAuthorizationPolicyProvider } from '@/services/entitlement/db-authorization-policy-provider'

/**
 * Explicit entitlement port for the service kernel. It is deliberately open in
 * SECURITY-CORE-01; a later entitlement story replaces policy without changing
 * the business-service composition seam.
 */
export const formEntitlements = new AuthorizationService(new SqlAuthorizationPolicyProvider())

/** The one production kernel composition: all six domains, one place. */
export const formCoreServices = composeCoreServices(
  {
    person: new SqlPersonRepository(),
    firm: new SqlFirmRepository(),
    property: new SqlPropertyRepository(),
    contract: new SqlContractRepository(),
    showing: new SqlShowingRepository(),
    security: new SqlSecurityRepository(),
  },
  { authorization: formEntitlements },
)

/** Showing is a first-class composed domain now (kept as a stable alias). */
export const formShowingService = formCoreServices.showing
