import type { BaseService } from './base-service'
import type { ServiceDescriptor } from './types'

/** Discovery is generic; business invocation remains strongly typed. */
export class ServiceRegistry {
  private readonly services = new Map<string, BaseService>()

  register<T extends BaseService>(service: T): T {
    if (this.services.has(service.domain)) {
      throw new Error(`Service already registered for domain: ${service.domain}`)
    }
    this.services.set(service.domain, service)
    return service
  }

  get<T extends BaseService>(domain: string): T | undefined {
    return this.services.get(domain) as T | undefined
  }

  require<T extends BaseService>(domain: string): T {
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
}
