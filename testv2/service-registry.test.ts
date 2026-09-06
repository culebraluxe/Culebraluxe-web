import { test } from 'node:test'
import assert from 'node:assert/strict'
import { ServiceRegistry } from '../services/core/service-registry'
import type { ServiceDescriptor, ServiceEndpoint, ServiceEnvelope, ServiceResult } from '../services/core/types'

// ---------------------------------------------------------------------------
// TESTV2 — ServiceRegistry (the ServiceRouter): registration, discovery, and
// envelope dispatch routing. DB-free; a stub ServiceEndpoint stands in.
// ---------------------------------------------------------------------------

function stubEndpoint(domain: string, value: unknown = { n: 1 }): ServiceEndpoint {
  return {
    domain,
    describe: (): ServiceDescriptor => ({
      domain,
      version: '1',
      description: 'stub',
      capabilities: [],
      dependencies: [],
      invariants: [],
    }),
    dispatch: async (envelope: ServiceEnvelope): Promise<ServiceResult<unknown>> => ({
      ok: true,
      value,
      correlationId: envelope.context.correlationId,
    }),
  }
}

const env = { context: { actor: { id: 'sys', kind: 'system' as const }, correlationId: 'c' } }

test('register/get/require expose a service under its domain', () => {
  const registry = new ServiceRegistry()
  const person = stubEndpoint('person')
  registry.register(person)
  assert.equal(registry.get('person'), person)
  assert.equal(registry.require('person'), person)
})

test('register refuses a duplicate domain', () => {
  const registry = new ServiceRegistry()
  registry.register(stubEndpoint('person'))
  assert.throws(() => registry.register(stubEndpoint('person')), /already registered/)
})

test('require throws for an unknown domain', () => {
  const registry = new ServiceRegistry()
  assert.throws(() => registry.require('ghost'), /not registered/)
})

test('list() returns descriptors sorted by domain', () => {
  const registry = new ServiceRegistry()
  registry.register(stubEndpoint('firm'))
  registry.register(stubEndpoint('person'))
  assert.deepEqual(registry.list().map((d) => d.domain), ['firm', 'person'])
})

test('dispatch routes an envelope to the owning service and echoes correlationId', async () => {
  const registry = new ServiceRegistry()
  registry.register(stubEndpoint('person', { n: 7 }))
  const res = await registry.dispatch('person', { operation: 'person.get', payload: {}, ...env })
  assert.equal(res.ok, true)
  if (res.ok) {
    assert.deepEqual(res.value, { n: 7 })
    assert.equal(res.correlationId, 'c')
  }
})

test('dispatch for an unregistered domain returns SERVICE_NOT_FOUND', async () => {
  const registry = new ServiceRegistry()
  const res = await registry.dispatch('ghost', { operation: 'ghost.get', payload: {}, ...env })
  assert.equal(res.ok, false)
  if (!res.ok) assert.equal(res.error.code, 'SERVICE_NOT_FOUND')
})
