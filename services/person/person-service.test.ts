import { test } from 'node:test'
import assert from 'node:assert/strict'
import { makePersonHarness, newContext } from '../test-support'
import type { PersonDto } from './types'

// ---------------------------------------------------------------------------
// CORE service envelope tests (Person domain). Proves the typed envelope
// boundary: queries, commands, events, authorization, audit, and errors all
// flow through BaseService's single ingress without a database.
// ---------------------------------------------------------------------------

const actor = { id: 'u-1', kind: 'user' as const }
const person: PersonDto = { id: 'p1', displayName: 'Dana', status: 'active', archivedAt: null }

test('person.get returns the canonical DTO for a known id', async () => {
  const { service, repository } = makePersonHarness()
  repository.seed(person)
  const res = await service.execute({
    operation: 'person.get',
    payload: { personId: 'p1' },
    context: newContext(actor),
  })
  assert.equal(res.ok, true)
  if (res.ok) assert.deepEqual(res.value, person)
})

test('person.get for an unknown id returns ok:true, value:null', async () => {
  const { service } = makePersonHarness()
  const res = await service.execute({
    operation: 'person.get',
    payload: { personId: 'missing' },
    context: newContext(actor),
  })
  assert.equal(res.ok, true)
  if (res.ok) assert.equal(res.value, null)
})

test('person.setDisplayName is a command that updates + emits a domain event', async () => {
  const { service, repository, infra } = makePersonHarness()
  repository.seed(person)
  const res = await service.execute({
    operation: 'person.setDisplayName',
    payload: { personId: 'p1', displayName: 'Dana R.' },
    context: { actor, correlationId: 'corr-x' },
  })
  assert.equal(res.ok, true)
  assert.equal(res.ok && res.correlationId, 'corr-x')
  if (res.ok) assert.equal(res.value.displayName, 'Dana R.')
  assert.ok(
    infra.events.some((e) => e.type === 'person.display_name_changed' && e.aggregateId === 'p1'),
    'a display_name_changed event was emitted',
  )
  assert.ok(infra.audits.some((a) => a.operation === 'person.setDisplayName' && a.outcome === 'success'))
})

test('person.attachIdentity makes findByIdentity resolve the owner', async () => {
  const { service, repository } = makePersonHarness()
  repository.seed(person)
  const attached = await service.execute({
    operation: 'person.attachIdentity',
    payload: { personId: 'p1', identity: { kind: 'email', value: 'dana@culebraluxe.com', isPrimary: true } },
    context: newContext(actor),
  })
  assert.equal(attached.ok, true)
  const found = await service.execute({
    operation: 'person.findByIdentity',
    payload: { identity: { kind: 'email', value: 'dana@culebraluxe.com', isPrimary: true } },
    context: newContext(actor),
  })
  assert.equal(found.ok, true)
  if (found.ok) assert.equal(found.value?.id, 'p1')
})

test('a denied authorization returns a FORBIDDEN failure (never calls the handler)', async () => {
  const { service, repository, infra } = makePersonHarness()
  repository.seed(person)
  infra.setAuthorized(false)
  const res = await service.execute({
    operation: 'person.setDisplayName',
    payload: { personId: 'p1', displayName: 'X' },
    context: newContext(actor),
  })
  assert.equal(res.ok, false)
  if (!res.ok) {
    assert.equal(res.error.code, 'FORBIDDEN')
    assert.ok(infra.audits.some((a) => a.outcome === 'failure' && a.errorCode === 'FORBIDDEN'))
  }
})

test('an unknown operation returns UNKNOWN_OPERATION failure', async () => {
  const { service } = makePersonHarness()
  const res = await service.execute({ operation: 'person.nope', payload: {}, context: newContext(actor) } as never)
  assert.equal(res.ok, false)
  if (!res.ok) assert.equal(res.error.code, 'UNKNOWN_OPERATION')
})
