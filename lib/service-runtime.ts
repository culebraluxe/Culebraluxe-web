import 'server-only'

import { SqlContractRepository } from '@/legacy/db/contract-service-repository'
import { SqlFirmRepository } from '@/legacy/db/firm-service-repository'
import { SqlPersonRepository } from '@/legacy/db/person-service-repository'
import { SqlPropertyRepository } from '@/legacy/db/property-service-repository'
import { SqlSecurityRepository } from '@/legacy/db/security-service-repository'
import { SqlShowingRepository } from '@/legacy/db/showing-service-repository'
import { SqlWbsRepository } from '@/legacy/db/wbs-service-repository'
import { SqlProjectRepository } from '@/legacy/db/project-service-repository'
import { SqlFormInstanceRepository } from '@/legacy/db/form-service-repository'
import { SqlVaultRepository } from '@/legacy/db/vault-service-repository'
import { SqlCommsRepository } from '@/legacy/db/comms-service-repository'
import { composeCoreServices } from '@/legacy/services/composition'
import { AuthorizationService } from '@/legacy/services/entitlement'
import { SqlAuthorizationPolicyProvider } from '@/legacy/services/entitlement/db-authorization-policy-provider'
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
    comms: new SqlCommsRepository(),
    form: new SqlFormInstanceRepository(),
    vault: new SqlVaultRepository(),
  },
  { authorization: coreEntitlements, errors: appServiceErrorSink() },
)
