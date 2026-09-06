import { ServiceRegistry, type ServiceInfrastructure } from './core'
import { PersonService, type PersonRepository } from './person'
import { FirmService, type FirmRepository } from './firm'
import { PropertyService, type PropertyRepository } from './property'
import { ContractService, type ContractRepository } from './contract'
import { SecurityService, type SecurityRepository } from './security'

export type CoreServiceRepositories = {
  person: PersonRepository
  firm: FirmRepository
  property: PropertyRepository
  contract: ContractRepository
  /** Optional during migration; production composition supplies Security. */
  security?: SecurityRepository
}

export type CoreServiceComposition = {
  registry: ServiceRegistry
  person: PersonService
  firm: FirmService
  property: PropertyService
  contract: ContractService
  security: SecurityService | null
}

/**
 * Composition root for the CulebraLuxe business-service kernel.
 * The registry is injected back as ServiceRouter, so services communicate only
 * through envelopes and owning service contracts.
 */
export function composeCoreServices(
  repositories: CoreServiceRepositories,
  infrastructure: Omit<ServiceInfrastructure, 'router'> = {},
): CoreServiceComposition {
  const registry = new ServiceRegistry()
  const serviceInfrastructure: ServiceInfrastructure = {
    ...infrastructure,
    router: registry,
  }

  const person = registry.register(new PersonService(repositories.person, serviceInfrastructure))
  const firm = registry.register(new FirmService(repositories.firm, serviceInfrastructure))
  const property = registry.register(new PropertyService(repositories.property, serviceInfrastructure))
  const contract = registry.register(new ContractService(repositories.contract, serviceInfrastructure))
  const security = repositories.security
    ? registry.register(new SecurityService(repositories.security, serviceInfrastructure))
    : null

  return { registry, person, firm, property, contract, security }
}
