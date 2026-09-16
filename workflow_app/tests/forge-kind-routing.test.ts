import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  FORGE_KINDS,
  FORGE_MODEL_POLICIES,
  KIND_ROUTING,
  MODEL_FOR_POLICY,
  asForgeKind,
  asModelPolicy,
  describeRouting,
  modelForPolicy,
  policyForBatch,
  routingFor,
  summarizeKinds,
} from '../../lib/forge-kind'
import { fireForgeBatch } from '../../db/forge-batch'
import { setWorkItemRouting } from '../../db/agent-work'
import type { QueryExecutor, QueryRow } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// ENG-FORGE-FACTORY-01 PHASE 1 — the kind router.
//
// Two halves, because two different things can be wrong:
//   1. the routing TABLE (six kinds, two policies, one model mapping) — pure, asserted directly;
//   2. the COPY AT DISPATCH — `fireForgeBatch` must stamp kind + policy onto the work item the
//      dispatch trigger just created, which is the packet's acceptance test and the part that
//      cannot be verified by reading the code, because the trigger sees only the story row.
// The fake below is a recording executor: it answers the shapes the fire path asks for and keeps
// every statement, so the assertions can be about what was WRITTEN, not about what was intended.
// ---------------------------------------------------------------------------

type Recorded = { sql: string; params: unknown[] }

function fakeFireDb(options: {
  members: Array<{ story_id: string; kind: string }>
  batchPolicy?: string | null
  /** What the `update agent_work_item` returns — [] models a story whose item was not `Ready`. */
  stampRows?: QueryRow[]
}): { tx: QueryExecutor; recorded: Recorded[] } {
  const recorded: Recorded[] = []
  const tx: QueryExecutor = (strings, ...params) => {
    const sql = strings.reduce((acc, part, i) => acc + part + (i < params.length ? `$${i + 1}` : ''), '')
    recorded.push({ sql, params })
    const norm = sql.replace(/\s+/g, ' ').toLowerCase()

    if (norm.includes('select story_id, kind from forge_batch_item')) {
      return Promise.resolve(options.members as unknown as QueryRow[])
    }
    if (norm.includes('from forge_batch b')) {
      return Promise.resolve([
        {
          id: 'batch-1',
          label: 'staging',
          status: 'Staged',
          scheduled_for: null,
          fired_at: null,
          created_at: '2026-09-15T00:00:00.000Z',
          created_by: null,
          note: null,
          model_policy: options.batchPolicy ?? null,
          story_count: options.members.length,
          queued_count: 0,
          skipped_count: 0,
        },
      ])
    }
    if (norm.includes('update storyboard_story')) {
      return Promise.resolve([
        {
          id: (params[1] as string) ?? 'story',
          status: params[0],
          priority: 'Medium',
          completion: 0,
          title: 'fixture story',
          workstream: 'ENGINEERING',
          notes: null,
        } as unknown as QueryRow,
      ])
    }
    if (norm.includes('update agent_work_item')) {
      return Promise.resolve(options.stampRows ?? ([{ id: 'work-item-1' }] as unknown as QueryRow[]))
    }
    return Promise.resolve([] as QueryRow[])
  }
  return { tx, recorded }
}

const stampedCalls = (recorded: Recorded[]) => recorded.filter((r) => /update agent_work_item/.test(r.sql))

test('every kind routes to a policy and a starting lane', () => {
  for (const kind of FORGE_KINDS) {
    const routing = KIND_ROUTING[kind]
    assert.ok(routing, `${kind} has no routing`)
    assert.ok(FORGE_MODEL_POLICIES.includes(routing.policy), `${kind} routes to an unknown policy`)
    assert.ok(['Scout', 'Architect', 'Assay'].includes(routing.laneStart), `${kind} starts in an unknown lane`)
    assert.ok(routing.label.length > 0 && routing.example.length > 0)
  }
  assert.equal(FORGE_KINDS.length, 6, 'the packet allows six kinds; adding one is a HOLD')
})

test('there are exactly two policies and two model names — not a model picker', () => {
  assert.deepEqual([...FORGE_MODEL_POLICIES], ['cheap', 'judgment'])
  assert.equal(Object.keys(MODEL_FOR_POLICY).length, 2)
  assert.equal(modelForPolicy('cheap'), 'deepseek/deepseek-v4-flash')
  assert.equal(modelForPolicy('judgment'), 'deepseek/deepseek-chat')
  // The names must be the ones the price table already knows, or the cost lens cannot price them.
  assert.ok(MODEL_FOR_POLICY.cheap.model.includes('deepseek-v4-flash'))
  assert.ok(MODEL_FOR_POLICY.judgment.model.includes('deepseek-chat'))
})

test('an unknown kind or policy reads as the default rather than throwing', () => {
  assert.equal(asForgeKind('quantum'), 'fix')
  assert.equal(asForgeKind(null), 'fix')
  assert.equal(asForgeKind(undefined), 'fix')
  assert.equal(asModelPolicy('premium'), 'cheap')
  assert.equal(asModelPolicy(null), 'cheap')
  assert.equal(policyForBatch(undefined), 'cheap')
  assert.equal(policyForBatch('judgment'), 'judgment')
  assert.equal(routingFor('judgment').laneStart, 'Architect')
  assert.equal(routingFor('learn').laneStart, 'Assay')
})

test('describeRouting names the kind, the policy, the model and the lane', () => {
  assert.equal(describeRouting('fix'), 'fix/cheap → deepseek/deepseek-v4-flash · starts Scout')
  assert.equal(describeRouting('judgment'), 'judgment/dear → deepseek/deepseek-chat · starts Architect')
  // An explicit policy wins over the kind's default: that is what the batch row decides.
  assert.equal(describeRouting('qa', 'judgment'), 'qa/dear → deepseek/deepseek-chat · starts Scout')
})

test('the kind mix is deterministic, so a 30-second refresh cannot look like a change', () => {
  assert.equal(summarizeKinds([]), '')
  assert.equal(summarizeKinds(['fix']), 'fix')
  assert.equal(summarizeKinds(['judgment', 'fix', 'fix']), 'fix ×2, judgment')
  assert.equal(summarizeKinds(['fix', 'judgment']), summarizeKinds(['judgment', 'fix']))
  // Declaration order, not input order: the string is compared by eye on screen.
  assert.equal(summarizeKinds(['learn', 'qa', 'crm']), 'qa, crm, learn')
})

// --- the copy at dispatch: the packet's acceptance test ---------------------

test('firing a batch copies each item kind and the batch policy onto the work item', async () => {
  const { tx, recorded } = fakeFireDb({
    members: [
      { story_id: 'ENG-A', kind: 'fix' },
      { story_id: 'ENG-B', kind: 'judgment' },
    ],
    batchPolicy: 'cheap',
  })

  const result = await fireForgeBatch('batch-1', tx)

  assert.equal(result.queued, 2)
  assert.equal(result.stamped, 2, 'both queued items must receive their routing')
  assert.deepEqual(result.failed, [])

  const calls = stampedCalls(recorded)
  assert.equal(calls.length, 2)
  // params are [kind, model_policy, story_id] — assert the values, not the SQL text.
  assert.deepEqual(calls.map((c) => c.params), [
    ['fix', 'cheap', 'ENG-A'],
    ['judgment', 'cheap', 'ENG-B'],
  ])
})

test('one policy per batch: a judgment batch does not run its cheap kinds cheaply', async () => {
  // The packet: "Night / fireDueForgeBatches defaults model_policy=cheap unless the row says
  // judgment". The row is the BATCH, so a `qa` member inside a judgment batch runs judgment.
  const { tx, recorded } = fakeFireDb({
    members: [{ story_id: 'ENG-C', kind: 'qa' }],
    batchPolicy: 'judgment',
  })
  await fireForgeBatch('batch-1', tx)
  assert.deepEqual(stampedCalls(recorded)[0].params, ['qa', 'judgment', 'ENG-C'])
})

test('a batch with no policy on the row fires cheap — the night-run default', async () => {
  const { tx, recorded } = fakeFireDb({ members: [{ story_id: 'ENG-D', kind: 'feature' }], batchPolicy: null })
  await fireForgeBatch('batch-1', tx)
  assert.deepEqual(stampedCalls(recorded)[0].params, ['feature', 'cheap', 'ENG-D'])
})

test('a queued item that was not Ready is not stamped, and the count says so', async () => {
  // The dispatch trigger's conflict clause keeps an existing claimed/running row, so there is
  // nothing to stamp. The dispatch still happened — this must be visible, not silently a no-op.
  const { tx } = fakeFireDb({ members: [{ story_id: 'ENG-E', kind: 'fix' }], stampRows: [] })
  const result = await fireForgeBatch('batch-1', tx)
  assert.equal(result.queued, 1)
  assert.equal(result.stamped, 0)
})

test('setWorkItemRouting touches only the story’s Ready item and reports the row count', async () => {
  const seen: Recorded[] = []
  const tx: QueryExecutor = (strings, ...params) => {
    const sql = strings.reduce((acc, part, i) => acc + part + (i < params.length ? `$${i + 1}` : ''), '')
    seen.push({ sql, params })
    return Promise.resolve([{ id: 'work-item-9' }] as unknown as QueryRow[])
  }
  const rows = await setWorkItemRouting('ENG-F', { kind: 'learn', modelPolicy: 'cheap' }, tx)
  assert.equal(rows, 1)
  assert.match(seen[0].sql.replace(/\s+/g, ' '), /where story_id = \$3 and state = 'Ready'/)
  assert.deepEqual(seen[0].params, ['learn', 'cheap', 'ENG-F'])
})
