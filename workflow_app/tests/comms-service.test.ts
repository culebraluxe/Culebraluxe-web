import assert from 'node:assert/strict'
import test from 'node:test'

import { SqlCommsRepository } from '../../db/comms-service-repository'
import type { QueryExecutor, QueryRow } from '../../db/query-executor'
import type { RelationshipEvidenceForContext } from '../../lib/relationship-intel/relationship-context'
import { CommsService } from '../../services/comms'
import {
  AuthorizationService,
  StaticAuthorizationPolicyProvider,
} from '../../services/entitlement'
import type {
  CommsMomentPage,
  CommsMomentRecord,
  CommsRepository,
  CommsSourceRecord,
} from '../../services/comms'

const SOURCE = (source: string, totalCount: number): CommsSourceRecord => ({
  source,
  firstObservedAt: '2025-01-01T00:00:00.000Z',
  lastContactAt: '2026-09-10T00:00:00.000Z',
  lastInboundAt: '2026-09-10T00:00:00.000Z',
  lastOutboundAt: null,
  inboundCount: totalCount,
  outboundCount: 0,
  totalCount,
  twoWay: false,
  lastDirection: 'inbound',
  lastContext: 'Airport pickup',
  lastContextAt: '2026-09-10T00:00:00.000Z',
  lastContextType: 'title',
  lastContextDirection: 'inbound',
})

const MOMENT = (id: string, sourceSystem: string): CommsMomentRecord => ({
  id,
  sourceSystem,
  direction: 'inbound',
  occurredAt: '2026-09-10T12:00:00.000Z',
  title: 'Airport pickup',
  summary: null,
})

/** Records the paging it was asked for, so clamping is observable. */
function harness(overrides: Partial<CommsRepository> = {}) {
  const calls: Array<{ limit: number; offset: number }> = []
  const repository: CommsRepository = {
    sources: async () => [SOURCE('apple_calls', 7), SOURCE('icloud_mail', 4), SOURCE('apple_facetime', 2)],
    evidence: async (): Promise<RelationshipEvidenceForContext[]> => [
      {
        source: 'apple_calls',
        inboundCount: 5,
        outboundCount: 2,
        lastObservedAt: '2026-09-10T00:00:00.000Z',
        lastInboundAt: '2026-09-10T00:00:00.000Z',
        firstObservedAt: '2025-01-01T00:00:00.000Z',
        isTwoWay: true,
      },
      {
        source: 'icloud_mail',
        inboundCount: 3,
        outboundCount: 1,
        lastObservedAt: '2026-09-01T00:00:00.000Z',
        firstObservedAt: '2026-08-01T00:00:00.000Z',
      },
      {
        // Bulk mail must never refresh meaningful contact.
        source: 'gmail_contacts',
        inboundCount: 900,
        outboundCount: 900,
        lastObservedAt: '2026-09-11T00:00:00.000Z',
        isAutomatedOrBulk: true,
      },
    ],
    lastContact: async () => ({ at: '2026-09-10T00:00:00.000Z', label: 'Sep 10, 2026' }),
    moments: async (_personId, limit, offset): Promise<CommsMomentPage> => {
      calls.push({ limit, offset })
      return { moments: [MOMENT('m1', 'icloud_mail'), MOMENT('m2', 'apple_messages')], total: 42 }
    },
    ...overrides,
  }
  return {
    service: new CommsService(repository, {
      authorization: new AuthorizationService(new StaticAuthorizationPolicyProvider()),
    }),
    repository,
    calls,
  }
}

const context = { actor: { id: 'u-1', kind: 'user' as const }, correlationId: 'corr-comms' }

test('comms.panel maps every raw source to a canonical channel the pane can render', async () => {
  const { service } = harness()
  const res = await service.execute({ operation: 'comms.panel', payload: { personId: 'p1' }, context })
  assert.equal(res.ok, true)
  if (!res.ok) return

  const bySource = new Map(res.value.sources.map((source) => [source.source, source]))
  // The bug this service exists to fix: the read model emits apple_calls and
  // apple_facetime, which the pane's icon map does not know.
  assert.equal(bySource.get('apple_calls')?.channel, 'call')
  assert.equal(bySource.get('apple_calls')?.label, 'Call')
  assert.equal(bySource.get('apple_facetime')?.channel, 'meeting')
  assert.equal(bySource.get('apple_facetime')?.label, 'Meeting')
  assert.equal(bySource.get('icloud_mail')?.channel, 'email')
  // Canonical channel order, so Call is above Email above Meeting.
  assert.deepEqual(
    res.value.sources.map((source) => source.channel),
    ['call', 'email', 'meeting'],
  )
})

test('comms.panel summarizes the header over evidence and keeps bulk out of meaningful contact', async () => {
  const { service } = harness()
  const res = await service.execute({ operation: 'comms.panel', payload: { personId: 'p1' }, context })
  assert.equal(res.ok, true)
  if (!res.ok) return

  const aggregate = res.value.aggregate
  assert.equal(aggregate.inboundCount, 5 + 3 + 900)
  assert.equal(aggregate.outboundCount, 2 + 1 + 900)
  assert.equal(aggregate.observedCount, 5 + 3 + 900 + 2 + 1 + 900)
  assert.equal(aggregate.twoWay, true)
  // The bulk row is newest, but meaningful contact stops at the calls.
  assert.equal(aggregate.lastContactAt, '2026-09-10T00:00:00.000Z')
  assert.equal(aggregate.lastContactLabel, 'Sep 10, 2026')
  assert.equal(aggregate.activeSourceCount, 3)
  assert.equal(aggregate.sourceCount, 8)
})

test('comms.panel maps each moment onto a canonical channel from its source system', async () => {
  const { service } = harness()
  const res = await service.execute({ operation: 'comms.panel', payload: { personId: 'p1' }, context })
  assert.equal(res.ok, true)
  if (!res.ok) return
  assert.deepEqual(
    res.value.moments.map((moment) => moment.channel),
    ['email', 'imessage'],
  )
  assert.equal(res.value.momentCount, 42)
})

test('comms.timeline clamps paging so the pane cannot ask for the whole archive', async () => {
  const { service, calls } = harness()
  const res = await service.execute({
    operation: 'comms.timeline',
    payload: { personId: 'p1', page: 0, pageSize: 5000 },
    context,
  })
  assert.equal(res.ok, true)
  if (!res.ok) return
  assert.equal(res.value.page, 1)
  assert.equal(res.value.pageSize, 100)
  assert.deepEqual(calls.at(-1), { limit: 100, offset: 0 })

  const page3 = await service.execute({
    operation: 'comms.timeline',
    payload: { personId: 'p1', page: 3, pageSize: 20 },
    context,
  })
  assert.equal(page3.ok, true)
  assert.deepEqual(calls.at(-1), { limit: 20, offset: 40 })
})

test('comms.timeline defaults to the panel page size and reports the total', async () => {
  const { service } = harness()
  const res = await service.execute({ operation: 'comms.timeline', payload: { personId: 'p1' }, context })
  assert.equal(res.ok, true)
  if (!res.ok) return
  assert.equal(res.value.pageSize, 20)
  assert.equal(res.value.total, 42)
})

/** A channel we cannot name is still a contact, so it must not vanish. */
test('an unknown source becomes other rather than being dropped', async () => {
  const { service } = harness({ sources: async () => [SOURCE('carrier_pigeon', 3)] })
  const res = await service.execute({ operation: 'comms.panel', payload: { personId: 'p1' }, context })
  assert.equal(res.ok, true)
  if (!res.ok) return
  assert.equal(res.value.sources.length, 1)
  assert.equal(res.value.sources[0].channel, 'other')
  assert.equal(res.value.sources[0].label, 'Other')
})

/** The architectural rule, enforced in a test: COMMS never reads an ODS table. */
test('the comms repository reads warehouse read models only, never an l_ table', async () => {
  const queries: string[] = []
  const execute = (async (strings: TemplateStringsArray) => {
    queries.push(strings.join(' ').replace(/\s+/g, ' ').trim())
    return [] as QueryRow[]
  }) as QueryExecutor

  const repository = new SqlCommsRepository(execute)
  await repository.sources('p1')
  await repository.evidence('p1')
  await repository.lastContact('p1')
  await repository.moments('p1', 10, 0)

  assert.ok(queries.length >= 5, 'expected the repository to issue its reads')
  for (const query of queries) {
    assert.ok(!/\bl_[a-z_]+/.test(query), `comms must not read an ODS table: ${query}`)
  }
  assert.ok(
    queries.some((query) => query.includes('mv_client_relationship_channels')),
    'sources come from the relationship-channel read model',
  )
  assert.ok(queries.some((query) => query.includes('interaction')), 'moments come from interactions')
  assert.ok(
    queries.some((query) => query.includes('mv_client_directory')),
    'the last-contact label comes from the client directory read model',
  )
})
