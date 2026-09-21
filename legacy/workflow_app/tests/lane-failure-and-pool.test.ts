import assert from 'node:assert/strict'
import { test } from 'node:test'
import { runWaveBatch, type WaveLane } from '@/legacy/workflow_app/forge/forge-executor'
import { createForgeReleaseOperations, type ForgeReleasePool } from '@/legacy/workflow_app/forge/release-operations'

const delay = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms))

function lane(name: string): WaveLane<string> {
  return { lane: name, surface: null, task: name }
}

function sharedPool(): ForgeReleasePool & { closed: () => boolean } {
  let closed = false
  return {
    closed: () => closed,
    async query() {
      if (closed) throw new Error('shared pool is closed')
      return { rows: [{}], rowCount: 1 }
    },
    async end() {
      closed = true
    },
  }
}

test('reproduction: a rejecting lane rejects the batch before a slow sibling settles (confirmed)', async () => {
  let siblingSettled = false
  const sibling = (async () => {
    await delay(30)
    siblingSettled = true
  })()
  const failing = (async () => {
    throw new Error('lane failed')
  })()
  await assert.rejects(Promise.all([failing, sibling]))
  assert.equal(
    siblingSettled,
    false,
    'Promise.all surfaced the failure before the slow sibling settled',
  )
  await sibling
})

test('reproduction: an operation end closes the shared pool for a later caller (confirmed)', async () => {
  const pool = sharedPool()
  await pool.end()
  await assert.rejects(pool.query('select 1'), /shared pool is closed/)
})

test('lane failure: a rejecting lane reports only after every sibling has settled', async () => {
  const settled: string[] = []
  await assert.rejects(
    runWaveBatch([lane('a'), lane('b'), lane('c')], async (item) => {
      if (item.lane === 'a') throw new Error('a failed')
      await delay(item.lane === 'b' ? 20 : 40)
      settled.push(item.lane)
    }),
    /a failed/,
  )
  assert.deepEqual(settled, ['b', 'c'], 'every sibling settled before the failure surfaced')
})

test('lane failure: the records of siblings that did run are preserved', async () => {
  const records: string[] = []
  await assert.rejects(
    runWaveBatch([lane('a'), lane('b')], async (item) => {
      if (item.lane === 'a') throw new Error('a failed')
      await delay(10)
      records.push(item.lane)
    }),
  )
  assert.deepEqual(records, ['b'], 'the sibling that ran kept its record')
})

test('lane failure: no driver failure is returned while a sibling is still writing', async () => {
  let writing = false
  let surfacedWhileWriting = false
  try {
    await runWaveBatch([lane('a'), lane('b')], async (item) => {
      if (item.lane === 'a') throw new Error('a failed')
      writing = true
      await delay(20)
      writing = false
    })
  } catch {
    surfacedWhileWriting = writing
  }
  assert.equal(
    surfacedWhileWriting,
    false,
    'the failure surfaced while a sibling was still writing',
  )
})

test('shared pool: no operation closes the pool another caller is using', async () => {
  const pool = sharedPool()
  const ops = createForgeReleaseOperations({ poolForTarget: () => pool })
  const refreshed = await ops.refreshDerived({
    commandId: 'c1',
    storyId: 's1',
    target: 'dev',
    models: ['mv_one'],
  })
  assert.equal(refreshed.success, true)
  assert.equal(pool.closed(), false, 'an operation closed the shared pool')
})

test('shared pool: a caller can still run a query after another caller finished', async () => {
  const pool = sharedPool()
  const ops = createForgeReleaseOperations({ poolForTarget: () => pool })
  const verified = await ops.verifyDerived({
    storyId: 's1',
    target: 'dev',
    models: ['mv_one'],
    attemptCommandId: 'c1',
  })
  assert.equal(verified.success, true)
  const after = await pool.query('select 1')
  assert.equal(after.rowCount, 1, 'a later caller could still query the shared pool')
})

test('shared pool: the pool lifecycle is owned by process shutdown, not one operation', async () => {
  const pool = sharedPool()
  const ops = createForgeReleaseOperations({ poolForTarget: () => pool })
  await ops.verifyDerived({
    storyId: 's1',
    target: 'dev',
    models: ['mv_one'],
    attemptCommandId: 'c1',
  })
  await ops.refreshDerived({
    commandId: 'c2',
    storyId: 's1',
    target: 'dev',
    models: ['mv_one'],
  })
  assert.equal(pool.closed(), false, 'the pool outlived every operation')
  const after = await pool.query('select 1')
  assert.equal(after.rowCount, 1)
})
