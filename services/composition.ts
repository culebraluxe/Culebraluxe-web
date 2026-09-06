import { ServiceRegistry, type ServiceInfrastructure } from './core'
import { PersonService, type PersonRepository } from './person'
import { PropertyService, type PropertyRepository } from './property'
import { ContractService, type ContractRepository } from './contract'

export type CoreServiceRepositories = {
  person: PersonRepository
  property: PropertyRepository
  contract: ContractRepository
}

export type CoreServiceComposition = {
  registry: ServiceRegistry
  person: PersonService
  property: PropertyService
  contract: ContractService
}

/**
 * Composition root for the first CulebraLuxe business-service kernel.
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
  const property = registry.register(new PropertyService(repositories.property, serviceInfrastructure))
  const contract = registry.register(new ContractService(repositories.contract, serviceInfrastructure))

  return { registry, person, property, contract }
}
