import type {
  ServiceDescriptor,
  ServiceEndpoint,
  ServiceEnvelope,
  ServiceResult,
  ServiceRouter,
} from './types'

/**
 * Local discovery + routing table. Business services see only ServiceRouter.
 * A remote/router adapter can replace this later without changing callers.
 */
export class ServiceRegistry implements ServiceRouter {
  private readonly services = new Map<string, ServiceEndpoint>()

  register<T extends ServiceEndpoint>(service: T): T {
    if (this.services.has(service.domain)) {
      throw new Error(`Service already registered for domain: ${service.domain}`)
    }
    this.services.set(service.domain, service)
    return service
  }

  get<T extends ServiceEndpoint>(domain: string): T | undefined {
    return this.services.get(domain) as T | undefined
  }

  require<T extends ServiceEndpoint>(domain: string): T {
    const service = this.get<T>(domain)
    if (!service) throw new Error(`Service not registered for domain: ${domain}`)
    return service
  }

  describe(domain: string): ServiceDescriptor | undefined {
    return this.services.get(domain)?.describe()
  }

  list(): readonly ServiceDescriptor[] {
    return [...this.services.values()]
      .map((service) => service.describe())
      .sort((a, b) => a.domain.localeCompare(b.domain))
  }

  async dispatch<TResponse = unknown>(
    domain: string,
    envelope: ServiceEnvelope,
  ): Promise<ServiceResult<TResponse>> {
    const service = this.services.get(domain)
    if (!service) {
      return {
        ok: false,
        error: {
          code: 'SERVICE_NOT_FOUND',
          message: `Service not registered for domain: ${domain}`,
          retryable: false,
        },
        correlationId: envelope.context.correlationId,
      }
    }

    return service.dispatch(envelope) as Promise<ServiceResult<TResponse>>
  }
}
