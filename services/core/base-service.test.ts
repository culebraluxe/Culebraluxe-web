import { describe, expect, it } from 'vitest'
import { BaseService, ServiceRegistry, type ServiceContext } from './index'

class TestService extends BaseService {
  readonly domain = 'test'
  readonly version = '1'
  readonly description = 'Service Core acceptance fixture'

  capabilities() {
    return [{ name: 'echo', description: 'Returns a value through the common execution envelope' }] as const
  }

  async echo(value: string) {
    return this.execute('echo', async () => value)
  }

  async denied() {
    return this.execute('denied', async () => {
      await this.authorize('denied')
      return 'should-not-run'
    })
  }
}

describe('Service Core', () => {
  it('registers, discovers, and executes a typed service', async () => {
    const audit: string[] = []
    const context: ServiceContext = {
      actor: { id: 'user-1', kind: 'user' },
      correlationId: 'corr-1',
      audit: {
        async record(event) {
          audit.push(`${event.domain}.${event.operation}:${event.outcome}`)
        },
      },
    }

    const registry = new ServiceRegistry()
    const service = registry.register(new TestService(context))

    expect(registry.describe('test')?.capabilities[0]?.name).toBe('echo')
    expect(registry.require<TestService>('test')).toBe(service)
    await expect(service.echo('hello')).resolves.toEqual({
      ok: true,
      value: 'hello',
      correlationId: 'corr-1',
    })
    expect(audit).toEqual(['test.echo:success'])
  })

  it('standardizes authorization failure and failure audit', async () => {
    const audit: string[] = []
    const service = new TestService({
      actor: { id: 'user-1', kind: 'user' },
      correlationId: 'corr-2',
      authorization: { async authorize() { return false } },
      audit: { async record(event) { audit.push(`${event.outcome}:${event.errorCode ?? ''}`) } },
    })

    const result = await service.denied()
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.error.code).toBe('FORBIDDEN')
    expect(audit).toEqual(['failure:FORBIDDEN'])
  })
})
