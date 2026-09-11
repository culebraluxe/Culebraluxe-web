import 'server-only'

import { SqlContractRepository } from '@/db/contract-service-repository'
import { SqlFirmRepository } from '@/db/firm-service-repository'
import { SqlPersonRepository } from '@/db/person-service-repository'
import { SqlPropertyRepository } from '@/db/property-service-repository'
import { SqlSecurityRepository } from '@/db/security-service-repository'
import { SqlShowingRepository } from '@/db/showing-service-repository'
import { SqlWbsRepository } from '@/db/wbs-service-repository'
import { SqlProjectRepository } from '@/db/project-service-repository'
import { SqlFormInstanceRepository } from '@/db/form-service-repository'
import { SqlVaultRepository } from '@/db/vault-service-repository'
import { composeCoreServices } from '@/services/composition'
import { AuthorizationService } from '@/services/entitlement'
import { SqlAuthorizationPolicyProvider } from '@/services/entitlement/db-authorization-policy-provider'
import { appServiceErrorSink } from '@/lib/service-error-sink'

/**
 * The ONE production kernel composition: every domain, one place.
 *
 * Server-side surfaces — the public site, forms, and portal routes — reach
 * business data through these services. Nothing outside a repository should
 * query the `property` / `person` tables directly; a screen that does is a
 * surface that missed the service-layer refactor.
 */
export const coreEntitlements = new AuthorizationService(new SqlAuthorizationPolicyProvider())

export const coreServices = composeCoreServices(
  {
    person: new SqlPersonRepository(),
    firm: new SqlFirmRepository(),
    property: new SqlPropertyRepository(),
    contract: new SqlContractRepository(),
    showing: new SqlShowingRepository(),
    security: new SqlSecurityRepository(),
    wbs: new SqlWbsRepository(),
    project: new SqlProjectRepository(),
    form: new SqlFormInstanceRepository(),
    vault: new SqlVaultRepository(),
  },
  { authorization: coreEntitlements, errors: appServiceErrorSink() },
)
