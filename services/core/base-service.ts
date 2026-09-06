import type {
  ServiceCapability,
  ServiceContext,
  ServiceDescriptor,
  ServiceDomainEvent,
  ServiceInfrastructure,
  ServiceResult,
} from './types'
import { ServiceError } from './service-error'

/**
 * Shared mechanics for CulebraLuxe domain services.
 *
 * Service instances are stateless with respect to actor/correlation context.
 * Per-call context travels with the invocation (an envelope today, another
 * transport tomorrow). The constructor only receives long-lived infrastructure.
 */
export abstract class BaseService {
  abstract readonly domain: string
  abstract readonly version: string
  abstract readonly description: string

  protected constructor(protected readonly infrastructure: ServiceInfrastructure = {}) {}

  abstract capabilities(): readonly ServiceCapability[]

  dependencies(): readonly string[] {
    return []
  }

  invariants(): readonly string[] {
    return []
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

  /** Common execution shell used by either envelope-style or method-style services. */
  protected async run<T>(
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

  protected async authorize(action: string, context: ServiceContext): Promise<void> {
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
}
