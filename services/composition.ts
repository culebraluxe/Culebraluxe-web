import {
  ServiceRegistry,
  type AuditPort,
  type DomainEventPort,
  type ServiceInfrastructure,
  type ServiceAuditEvent,
  type ServiceDomainEvent,
  type ServiceErrorRecord,
  type ServiceErrorSink,
} from './core'
import { PersonService, type PersonRepository } from './person'
import { FirmService, type FirmRepository } from './firm'
import { PropertyService, type PropertyRepository } from './property'
import { ContractService, type ContractRepository } from './contract'
import { SecurityService, type SecurityRepository } from './security'
import { ShowingService, type ShowingRepository } from './showing'
import { WbsService, type WbsRepository } from './wbs'
import { ProjectService, type ProjectRepository } from './project'
import { FormService, type FormRepository } from './forms'
import { VaultService, type VaultRepository } from './vault'
import {
  AuthorizationService,
  StaticAuthorizationPolicyProvider,
} from './entitlement'

/**
 * Repositories required to build the full service kernel. Security and Showing
 * are first-class here: there is no longer a "partial" kernel without them.
 */
export type CoreServiceRepositories = {
  person: PersonRepository
  firm: FirmRepository
  property: PropertyRepository
  contract: ContractRepository
  showing: ShowingRepository
  security: SecurityRepository
  wbs: WbsRepository
  project: ProjectRepository
  /**
   * Optional: kernels that never touch documents (security runtime, form
   * bindings, proofs) omit it. The full production runtime always provides it.
   */
  form?: FormRepository
  /** Optional for the same reason as `form`: only document-touching kernels need it. */
  vault?: VaultRepository
}

export type CoreServiceComposition = {
  registry: ServiceRegistry
  person: PersonService
  firm: FirmService
  property: PropertyService
  contract: ContractService
  showing: ShowingService
  security: SecurityService
  wbs: WbsService
  project: ProjectService
  /** Present when the composition was given a Form repository (the full runtime). */
  form?: FormService
  /** Present when the composition was given a Vault repository (the full runtime). */
  vault?: VaultService
}

/**
 * Default AuditPort used when the caller omits one. In-memory capturing so that
 * `audit()` never silently no-ops because the port is absent. A durable audit
 * adapter can replace this without changing any service.
 */
function defaultAuditPort(): AuditPort {
  const captured: ServiceAuditEvent[] = []
  return {
    record: async (event) => {
      captured.push(event)
    },
  }
}

/** Default DomainEventPort used when the caller omits one (in-memory capture). */
function defaultEventPort(): DomainEventPort {
  const captured: ServiceDomainEvent[] = []
  return {
    emit: async (event) => {
      captured.push(event)
    },
  }
}

/** Default ServiceErrorSink used when the caller omits one (in-memory capture). */
function defaultErrorSink(): ServiceErrorSink {
  const captured: ServiceErrorRecord[] = []
  return {
    record: async (failure) => {
      captured.push(failure)
    },
  }
}

/**
 * Composition root for the CulebraLuxe business-service kernel — the single
 * place the kernel is built. Envelope-only write path: new business writes for
 * these domains go through service envelopes, never raw repositories/commands.
 * The registry is injected back as ServiceRouter, so services communicate only
 * through envelopes and owning service contracts.
 *
 * Audit/events default to in-memory captures so nothing early-returns for a
 * missing port; entitlement is required infrastructure (supplied or explicit).
 */
export function composeCoreServices(
  repositories: CoreServiceRepositories,
  infrastructure: Omit<ServiceInfrastructure, 'router'> = {},
): CoreServiceComposition {
  const registry = new ServiceRegistry()
  const serviceInfrastructure: ServiceInfrastructure = {
    ...infrastructure,
    router: registry,
    audit: infrastructure.audit ?? defaultAuditPort(),
    events: infrastructure.events ?? defaultEventPort(),
    errors: infrastructure.errors ?? defaultErrorSink(),
    // Entitlement is required infrastructure: an enforced resolver by default so
    // a kernel is never built without an authorization decision source.
    authorization:
      infrastructure.authorization ?? new AuthorizationService(new StaticAuthorizationPolicyProvider()),
  }

  const person = registry.register(new PersonService(repositories.person, serviceInfrastructure))
  const firm = registry.register(new FirmService(repositories.firm, serviceInfrastructure))
  const property = registry.register(new PropertyService(repositories.property, serviceInfrastructure))
  const contract = registry.register(new ContractService(repositories.contract, serviceInfrastructure))
  const security = registry.register(new SecurityService(repositories.security, serviceInfrastructure))
  const showing = registry.register(new ShowingService(repositories.showing, serviceInfrastructure))
  const wbs = registry.register(new WbsService(repositories.wbs, serviceInfrastructure))
  const project = registry.register(new ProjectService(repositories.project, serviceInfrastructure))
  const form = repositories.form
    ? registry.register(new FormService(repositories.form, serviceInfrastructure))
    : undefined
  const vault = repositories.vault
    ? registry.register(new VaultService(repositories.vault, serviceInfrastructure))
    : undefined

  return {
    registry,
    person,
    firm,
    property,
    contract,
    showing,
    security,
    wbs,
    project,
    form,
    vault,
  }
}
