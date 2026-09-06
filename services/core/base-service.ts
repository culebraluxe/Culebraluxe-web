import { ServiceError } from './service-error'
import { InMemoryServiceQueue } from './service-queue'
import type {
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
      () =>
        this.run(envelope.operation, envelope.context, async () => {
          if (!definition) {
            this.fail('UNKNOWN_OPERATION', `Unknown ${this.domain} operation: ${envelope.operation}`)
          }

          await this.authorize(
            definition.authorization ?? envelope.operation,
            envelope.context,
          )

          return definition.handle(envelope.payload, envelope.context)
        }),
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
        result.error.cause,
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

  private async run<T>(
    operation: string,
    context: ServiceContext,
    work: () => Promise<T>,
  ): Promise<ServiceResult<T>> {
    try {
      const value = await work()
      await this.audit(operation, context, 'success')
      return { ok: true, value, correlationId: context.correlationId }
    } catch (cause) {
      const error = ServiceError.from(cause)
      await this.audit(operation, context, 'failure', error.code)
      return {
        ok: false,
        error: error.toShape(),
        correlationId: context.correlationId,
      }
    }
  }

  private async authorize(action: string, context: ServiceContext): Promise<void> {
    if (!this.infrastructure.authorization) return
    const allowed = await this.infrastructure.authorization.authorize({
      domain: this.domain,
      action,
      actor: context.actor,
    })
    if (!allowed) {
      throw new ServiceError('FORBIDDEN', `${this.domain}.${action} is not authorized`, false)
    }
  }

  private async audit(
    operation: string,
    context: ServiceContext,
    outcome: 'success' | 'failure',
    errorCode?: string,
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
    })
  }

  private resolvePartitionKey(partitionBy: string | undefined, payload: unknown): string | undefined {
    if (!partitionBy || payload === null || typeof payload !== 'object') return undefined
    const value = (payload as Record<string, unknown>)[partitionBy]
    return value === undefined || value === null ? undefined : String(value)
  }
}
