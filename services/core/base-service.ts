import type {
  ServiceCapability,
  ServiceContext,
  ServiceDescriptor,
  ServiceDomainEvent,
  ServiceResult,
} from './types'
import { ServiceError } from './service-error'

/**
 * Shared mechanics for CulebraLuxe domain services.
 *
 * Business APIs stay on concrete services. This class owns only mechanics that
 * should behave consistently across every service: execution envelope,
 * authorization, audit, events, errors, correlation, and discovery metadata.
 */
export abstract class BaseService {
  abstract readonly domain: string
  abstract readonly version: string
  abstract readonly description: string

  protected constructor(protected readonly context: ServiceContext) {}

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

  protected async execute<T>(operation: string, work: () => Promise<T>): Promise<ServiceResult<T>> {
    try {
      const value = await work()
      await this.audit(operation, 'success')
      return { ok: true, value, correlationId: this.context.correlationId }
    } catch (cause) {
      const error = ServiceError.from(cause)
      await this.audit(operation, 'failure', error.code)
      return {
        ok: false,
        error: error.toShape(),
        correlationId: this.context.correlationId,
      }
    }
  }

  protected async authorize(action: string): Promise<void> {
    if (!this.context.authorization) return
    const allowed = await this.context.authorization.authorize({
      domain: this.domain,
      action,
      actor: this.context.actor,
    })
    if (!allowed) {
      throw new ServiceError('FORBIDDEN', `${this.domain}.${action} is not authorized`, false)
    }
  }

  protected async emit(event: Omit<ServiceDomainEvent, 'correlationId' | 'causationId'>): Promise<void> {
    if (!this.context.events) return
    await this.context.events.emit({
      ...event,
      correlationId: this.context.correlationId,
      causationId: this.context.causationId,
    })
  }

  protected fail(code: string, message: string, retryable = false, cause?: unknown): never {
    throw new ServiceError(code, message, retryable, cause)
  }

  private async audit(
    operation: string,
    outcome: 'success' | 'failure',
    errorCode?: string,
  ): Promise<void> {
    if (!this.context.audit) return
    await this.context.audit.record({
      domain: this.domain,
      operation,
      actor: this.context.actor,
      correlationId: this.context.correlationId,
      causationId: this.context.causationId,
      outcome,
      errorCode,
    })
  }
}
