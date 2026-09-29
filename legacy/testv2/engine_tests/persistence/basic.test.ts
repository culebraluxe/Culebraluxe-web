import { test } from 'node:test'
import assert from 'node:assert/strict'

import { withTransaction } from '../../../lib/neon-interactive'
import {
  assertEngineSchema,
  ensureProbeTable,
  PersistenceFixture,
} from './harness'

const LINEAR = {
  startNodeId: 'start',
  nodes: {
    start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'end' }] },
    end: { id: 'end', type: 'end' },
  },
}

test('ENG-04: a real engine step against DEV persists exact state (setup -> execute -> assert -> teardown)', async () => {
  await assertEngineSchema()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition('tunit_linear', 1, LINEAR)
    const engine = f.makeEngine()
    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_linear',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })

    const inst = await f.instance(processInstanceId)
    assert.ok(inst, 'instance row persisted')
    assert.equal(inst.status, 'completed')

    const tokens = await f.tokens(processInstanceId)
    assert.equal(tokens.length, 1, 'exactly one token')
    assert.equal(tokens[0].node_id, 'end')
    assert.equal(tokens[0].status, 'completed')

    const events = await f.events(processInstanceId)
    assert.ok(events.some((e: any) => e.event_type === 'process.started'))
    assert.ok(events.some((e: any) => e.event_type === 'process.completed'))
  } finally {
    await f.cleanup()
  }
})

test('ENG-04: two interactive transactions can be intentionally overlapped (READ COMMITTED isolation observable)', async () => {
  await assertEngineSchema()
  await ensureProbeTable()
  const f = new PersistenceFixture()
  try {
    await f.rows`insert into tunit_probe (tenant_id, note) values (${f.tenantId}, 'baseline')`

    let signalT2 = () => {}
    let signalT1Commit = () => {}
    const t2MayRead = new Promise<void>((res) => {
      signalT2 = res
    })
    const t1MayCommit = new Promise<void>((res) => {
      signalT1Commit = res
    })

    const t1 = withTransaction(async (tx) => {
      await tx`insert into tunit_probe (tenant_id, note) values (${f.tenantId}, 't1-uncommitted')`
      signalT2() // t2 may now read; t1 has NOT committed
      await t1MayCommit // hold the transaction open until t2 has read
    })

    const t2 = withTransaction(async (tx) => {
      await t2MayRead
      const rows = await tx`select count(*)::int as c from tunit_probe where tenant_id = ${f.tenantId}`
      assert.equal(rows[0].c, 1, "t2 must NOT see t1's uncommitted row (isolation)")
      signalT1Commit() // t1 may now commit
    })

    await Promise.all([t1, t2])

    const after = await f.rows`select count(*)::int as c from tunit_probe where tenant_id = ${f.tenantId}`
    assert.equal(after[0].c, 2, "t1's commit became visible after t2's read")
  } finally {
    await f.cleanup()
  }
})
