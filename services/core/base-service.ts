import { ServiceError } from './service-error'
import { InMemoryServiceQueue } from './service-queue'
import type {
  AuthorizationDecision,
  ServiceCapability,
  ServiceContext,
  ServiceDescriptor,
  ServiceDomainEvent,
  ServiceEndpoint,
  ServiceEnvelope,
  ServiceEnvelopeFor,
  ServiceInfrastructure,
  ServiceOperationDefinition,
  ServiceOperationDefinitions,
  ServiceOperationMap,
  ServiceOperationName,
  ServiceOperationRequest,
  ServiceOperationResponse,
  ServiceQueue,
  ServiceResult,
} from './types'

/**
 * Common CulebraLuxe service runtime.
 *
 * Concrete services contribute only operation contracts/handlers, dependencies,
 * and invariants. This parent owns the stable ingress, mailbox/queue, auth,
 * audit, correlation, events, errors, discovery, and cross-service routing.
 */
export abstract class BaseService<TMap extends ServiceOperationMap>
  implements ServiceEndpoint
{
  abstract readonly domain: string
  abstract readonly version: string
  abstract readonly description: string
  protected abstract readonly operations: ServiceOperationDefinitions<TMap>

  private readonly queue: ServiceQueue

  protected constructor(protected readonly infrastructure: ServiceInfrastructure = {}) {
    // One mailbox per service instance by default. Replace with any ServiceQueue
    // implementation (durable/distributed) without changing the service contract.
    this.queue = infrastructure.queue ?? new InMemoryServiceQueue()
  }

  dependencies(): readonly string[] {
    return []
  }

  invariants(): readonly string[] {
    return []
  }

  capabilities(): readonly ServiceCapability[] {
    const definitions = this.operations as Record<
      string,
      ServiceOperationDefinition<unknown, unknown>
    >
    return Object.entries(definitions).map(([name, definition]) => {
      const { handle: _handle, ...metadata } = definition
      return { name, ...metadata }
    })
  }

  describe(): ServiceDescriptor {
    return {
      domain: this.domain,
      version: this.version,
      description: this.description,
      capabilities: this.capabilities(),
      dependencies: this.dependencies(),
      invariants: this.invariants(),
    }
  }

  /** Strongly typed domain ingress used by callers that know the service contract. */
  async execute<K extends ServiceOperationName<TMap>>(
    envelope: ServiceEnvelopeFor<TMap, K>,
  ): Promise<ServiceResult<ServiceOperationResponse<TMap, K>>> {
    return this.dispatch(envelope) as Promise<
      ServiceResult<ServiceOperationResponse<TMap, K>>
    >
  }

  /** Untyped transport edge used by ServiceRouter/queues/adapters. */
  async dispatch(envelope: ServiceEnvelope): Promise<ServiceResult<unknown>> {
    const definitions = this.operations as Record<
      string,
      ServiceOperationDefinition<unknown, unknown>
    >
    const definition = definitions[envelope.operation]
    const execution = definition?.execution ?? { mode: 'inline' as const }
    const partitionKey = this.resolvePartitionKey(execution.partitionBy, envelope.payload)

    return this.queue.submit(
      {
        domain: this.domain,
        envelope,
        execution,
        partitionKey,
      },
      () => this.runAuthorized(definition, envelope),
    )
  }

  /**
   * Typed synchronous service-to-service request/reply.
   * Domain services depend on another service's contract only — never its
   * concrete class, repository, or transport.
   */
  protected async callService<
    TTargetMap extends ServiceOperationMap,
    K extends ServiceOperationName<TTargetMap>,
  >(
    domain: string,
    operation: K,
    payload: ServiceOperationRequest<TTargetMap, K>,
    context: ServiceContext,
  ): Promise<ServiceOperationResponse<TTargetMap, K>> {
    const router = this.infrastructure.router
    if (!router) {
      throw new ServiceError(
        'SERVICE_ROUTER_UNAVAILABLE',
        `${this.domain} cannot call ${domain}.${operation}: no ServiceRouter is configured.`,
        true,
      )
    }

    const result = await router.dispatch<ServiceOperationResponse<TTargetMap, K>>(domain, {
      operation,
      payload,
      context,
    })

    if (!result.ok) {
      throw new ServiceError(
        result.error.code,
        `${domain}.${operation}: ${result.error.message}`,
        result.error.retryable,
      )
    }

    return result.value
  }

  protected async emit(
    event: Omit<ServiceDomainEvent, 'correlationId' | 'causationId'>,
    context: ServiceContext,
  ): Promise<void> {
    if (!this.infrastructure.events) return
    await this.infrastructure.events.emit({
      ...event,
      correlationId: context.correlationId,
      causationId: context.causationId,
    })
  }

  protected fail(code: string, message: string, retryable = false, cause?: unknown): never {
    throw new ServiceError(code, message, retryable, cause)
  }

  private async runAuthorized(
    definition: ServiceOperationDefinition<unknown, unknown> | undefined,
    envelope: ServiceEnvelope,
  ): Promise<ServiceResult<unknown>> {
    const operation = envelope.operation
    const context = envelope.context
    let decision: AuthorizationDecision | undefined

    try {
      if (!definition) {
        throw new ServiceError(
          'UNKNOWN_OPERATION',
          `Unknown ${this.domain} operation: ${operation}`,
        )
      }

      decision = await this.authorize(definition, operation, context)
      const value = await definition.handle(envelope.payload, context)
      await this.audit(operation, context, 'success', undefined, decision)
      return { ok: true, value, correlationId: context.correlationId }
    } catch (cause) {
      const error = ServiceError.from(cause)
      const stamp =
        error instanceof ServiceError
          ? (error.authorization ?? decision)
          : decision
      await this.audit(operation, context, 'failure', error.code, stamp)
      return {
        ok: false,
        error: error.toShape(),
        correlationId: context.correlationId,
      }
    }
  }

  /**
   * Authorization always runs for a known operation. A missing authorization
   * port is a boot-configuration error, never a silent allow.
   */
  private async authorize(
    definition: ServiceOperationDefinition<unknown, unknown>,
    operation: string,
    context: ServiceContext,
  ): Promise<AuthorizationDecision> {
    const port = this.infrastructure.authorization
    if (!port) {
      throw new ServiceError(
        'AUTHORIZATION_UNAVAILABLE',
        `${this.domain}.${operation} requires an authorization port; none is configured.`,
        false,
      )
    }

    const decision = await port.authorize({
      domain: this.domain,
      action: definition.authorization ?? operation,
      operation,
      kind: definition.kind ?? 'command',
      actor: context.actor,
      principal: context.principal,
    })

    if (!decision.allowed) {
      throw new ServiceError(
        'FORBIDDEN',
        `${this.domain}.${operation} is not authorized: ${decision.reason}`,
        false,
        undefined,
        decision,
      )
    }
    return decision
  }

  private async audit(
    operation: string,
    context: ServiceContext,
    outcome: 'success' | 'failure',
    errorCode?: string,
    authorization?: AuthorizationDecision,
  ): Promise<void> {
    if (!this.infrastructure.audit) return
    await this.infrastructure.audit.record({
      domain: this.domain,
      operation,
      actor: context.actor,
      correlationId: context.correlationId,
      causationId: context.causationId,
      outcome,
      errorCode,
      authorization,
    })
  }

  private resolvePartitionKey(partitionBy: string | undefined, payload: unknown): string | undefined {
    if (!partitionBy || payload === null || typeof payload !== 'object') return undefined
    const value = (payload as Record<string, unknown>)[partitionBy]
    return value === undefined || value === null ? undefined : String(value)
  }
}
