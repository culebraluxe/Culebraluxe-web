import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  BaseService,
  ServiceError,
  type ServiceErrorRecord,
  type ServiceErrorSink,
  type ServiceInfrastructure,
} from '../services/core'

// ---------------------------------------------------------------------------
// TESTV2 — BaseService durable-error seam. Unhandled (non-ServiceError) exceptions
// must reach the injected errors sink; intended domain failures (fail()/authorization
// denials) are audited control flow and must NOT be captured as error noise.
// ---------------------------------------------------------------------------

type ProbeMap = { ping: { request: { boom?: boolean; serviceError?: boolean }; response: string } }

const context = { actor: { id: 'u-1', kind: 'user' as const }, correlationId: 'c-1' }

class ProbeService extends BaseService<ProbeMap> {
  readonly domain = 'probe'
  readonly version = '1'
  readonly description = 'probe'
  protected readonly operations = {
    ping: {
      kind: 'query' as const,
      description: 'probe op',
      handle: async (request: { boom?: boolean; serviceError?: boolean }) => {
        if (request.serviceError) {
          throw new ServiceError('BUSINESS', 'intended domain outcome')
        }
        if (request.boom) {
          throw new Error('boom')
        }
        return 'pong'
      },
    },
  }

  constructor(infrastructure: ServiceInfrastructure) {
    super(infrastructure)
  }
}

function makeSink(): { sink: ServiceErrorSink; records: ServiceErrorRecord[] } {
  const records: ServiceErrorRecord[] = []
  const sink: ServiceErrorSink = {
    record: async (failure) => {
      records.push(failure)
    },
  }
  return { sink, records }
}

function infra(records: ServiceErrorRecord[]): ServiceInfrastructure {
  return {
    errors: { record: async (f) => records.push(f) },
    authorization: {
      authorize: async () => ({ allowed: true, reason: 'ok', policyId: 'p', mode: 'enforced' as const }),
    },
  }
}

test('unhandled exception is captured through the errors sink at error level fields', async () => {
  const records: ServiceErrorRecord[] = []
  const service = new ProbeService(infra(records))
  const result = await service.execute({ operation: 'ping', payload: { boom: true }, context })
  assert.equal(result.ok, false)
  if (result.ok) return
  assert.equal(result.error.code, 'UNEXPECTED')
  assert.equal(records.length, 1)
  assert.equal(records[0]!.domain, 'probe')
  assert.equal(records[0]!.operation, 'ping')
  assert.equal(records[0]!.message, 'boom')
  assert.equal(records[0]!.correlationId, 'c-1')
})

test('intended ServiceError (fail-style) is NOT captured as error noise', async () => {
  const records: ServiceErrorRecord[] = []
  const service = new ProbeService(infra(records))
  const result = await service.execute({ operation: 'ping', payload: { serviceError: true }, context })
  assert.equal(result.ok, false)
  if (result.ok) return
  assert.equal(result.error.code, 'BUSINESS')
  assert.equal(records.length, 0)
})

test('authorization denial (FORBIDDEN) is NOT captured as error noise', async () => {
  const records: ServiceErrorRecord[] = []
  const service = new ProbeService({
    errors: { record: async (f) => records.push(f) },
    authorization: {
      authorize: async () => ({ allowed: false, reason: 'denied', policyId: 'p', mode: 'enforced' as const }),
    },
  })
  const result = await service.execute({ operation: 'ping', payload: {}, context })
  assert.equal(result.ok, false)
  if (result.ok) return
  assert.equal(result.error.code, 'FORBIDDEN')
  assert.equal(records.length, 0)
})

test('capture sink throwing never breaks the service result (best-effort)', async () => {
  const service = new ProbeService({
    errors: { record: async () => { throw new Error('capture down') } },
    authorization: {
      authorize: async () => ({ allowed: true, reason: 'ok', policyId: 'p', mode: 'enforced' as const }),
    },
  })
  const result = await service.execute({ operation: 'ping', payload: { boom: true }, context })
  assert.equal(result.ok, false)
  if (result.ok) return
  assert.equal(result.error.code, 'UNEXPECTED')
})

test('no errors sink configured => no throw, result intact', async () => {
  const service = new ProbeService({
    authorization: {
      authorize: async () => ({ allowed: true, reason: 'ok', policyId: 'p', mode: 'enforced' as const }),
    },
  })
  const result = await service.execute({ operation: 'ping', payload: { boom: true }, context })
  assert.equal(result.ok, false)
  if (result.ok) return
  assert.equal(result.error.code, 'UNEXPECTED')
})
