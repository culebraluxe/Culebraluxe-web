import { ServiceRegistry, type ServiceInfrastructure } from './core'
import { PersonService, type PersonRepository } from './person'
import { FirmService, type FirmRepository } from './firm'
import { PropertyService, type PropertyRepository } from './property'
import { ContractService, type ContractRepository } from './contract'

export type CoreServiceRepositories = {
  person: PersonRepository
  firm: FirmRepository
  property: PropertyRepository
  contract: ContractRepository
}

export type CoreServiceComposition = {
  registry: ServiceRegistry
  person: PersonService
  firm: FirmService
  property: PropertyService
  contract: ContractService
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

  return { registry, person, firm, property, contract }
}
