import 'server-only'

import { SqlContractRepository } from '@/db/contract-service-repository'
import { SqlFirmRepository } from '@/db/firm-service-repository'
import { SqlPersonRepository } from '@/db/person-service-repository'
import { SqlPropertyRepository } from '@/db/property-service-repository'
import { SqlSecurityRepository } from '@/db/security-service-repository'
import { SqlShowingRepository } from '@/db/showing-service-repository'
import { composeCoreServices } from '@/services/composition'
import { EntitlementService } from '@/services/entitlement'
import { ShowingService } from '@/services/showing'

/**
 * Explicit entitlement port for the service kernel. It is deliberately open in
 * SECURITY-CORE-01; a later entitlement story replaces policy without changing
 * the business-service composition seam.
 */
export const formEntitlements = new EntitlementService()

export const formCoreServices = composeCoreServices(
  {
    person: new SqlPersonRepository(),
    firm: new SqlFirmRepository(),
    property: new SqlPropertyRepository(),
    contract: new SqlContractRepository(),
    security: new SqlSecurityRepository(),
  },
  { authorization: formEntitlements },
)

export const formShowingService = formCoreServices.registry.register(
  new ShowingService(new SqlShowingRepository(), {
    router: formCoreServices.registry,
    authorization: formEntitlements,
  }),
)
