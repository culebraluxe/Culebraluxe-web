import test from 'node:test'
import assert from 'node:assert/strict'

import { resetGoldenData } from '../../lib/qa-reset'
import { neonTx } from '../../db/tx'
import { interactiveSql } from '../../lib/neon-interactive'
import type { QueryExecutor, QueryRow } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// QA-RESET — Golden fixture teardown is FK-safe, atomic, and bounded.
//
// 1. DB-free ordering test (runs anywhere): proves resetGoldenData issues the
//    canonical FK-safe delete order and NEVER manually deletes `tokens` (which
//    would 23503 against tasks/jobs whose task.token_id/job.token_id have no
//    cascade).
// 2. DB-gated integration test (skips when the DEV DB is unreachable): creates
//    a real Golden workflow (instance + token + task→token + job→token + trace
//    + process_events), resets it, and asserts full cleanup with no FK error,
//    idempotent second reset, and unrelated DEV rows untouched.
// ---------------------------------------------------------------------------

// --- Fake tagged-template executor for the ordering test -------------------

function makeFakeTx() {
  const calls: string[] = []
  const tx = ((strings: TemplateStringsArray): Promise<QueryRow[]> => {
    const sqlText = strings.join('$n')
    calls.push(sqlText)
    // The instance-id lookup feeds the per-instance evidence cleanup loop.
    const isInstanceSelect = /select\s+id\s+from\s+process_instances/.test(sqlText)
    // The QA-identity lookup feeds the Golden-owned person cleanup loop.
    const isPersonIdentitySelect = /from\s+person_identity/.test(sqlText)
    return Promise.resolve(
      isInstanceSelect
        ? ([{ id: 'inst-1' }] as QueryRow[])
        : isPersonIdentitySelect
          ? ([{ id: 'person-1' }] as QueryRow[])
          : [],
    )
  }) as unknown as QueryExecutor
  return { tx, calls }
}

const sqlHas = (sqlText: string, needle: string) => sqlText.toLowerCase().includes(needle)

test('qa-reset: delete order is FK-safe — no manual token delete; evidence before process_instances', async () => {
  const { tx, calls } = makeFakeTx()
  await resetGoldenData(tx, { dealId: 'deal-1', propertyId: 'prop-1' })

  const joined = calls.join('\n')
  // process_instances deletion is present (canonical cascade removes tokens/tasks/jobs).
  assert.ok(sqlHas(joined, 'delete from process_instances'), 'process_instances delete must be issued')
  // Never manually delete `tokens` — that violates task.token_id / job.token_id (no cascade).
  assert.ok(!sqlHas(joined, 'delete from tokens'), 'must NOT manually delete tokens')
  // Evidence tables not covered by the cascade are cleaned first.
  const iTrace = calls.findIndex((c) => sqlHas(c, 'delete from workflow_execution_trace_event'))
  const iEvents = calls.findIndex((c) => sqlHas(c, 'delete from process_events'))
  const iInstances = calls.findIndex((c) => sqlHas(c, 'delete from process_instances'))
  assert.ok(iTrace >= 0, 'trace evidence delete must be issued')
  assert.ok(iEvents >= 0, 'process_events delete must be issued')
  assert.ok(iTrace < iInstances, 'trace deleted before process_instances')
  assert.ok(iEvents < iInstances, 'process_events deleted before process_instances')
  // Bounded teardown of the deal / participants / persons / property.
  assert.ok(sqlHas(joined, 'delete from deal_participant'), 'deal_participant delete must be issued')
  assert.ok(sqlHas(joined, 'delete from deal'), 'deal delete must be issued')
  assert.ok(sqlHas(joined, 'delete from person'), 'person delete must be issued')
  assert.ok(sqlHas(joined, 'delete from property'), 'property delete must be issued')
})

// --- DB-gated integration test ---------------------------------------------

async function dbAvailable(): Promise<boolean> {
  try {
    const rows = await interactiveSql`select to_regclass('public.process_instances') as pi`
    return rows[0]?.pi != null
  } catch {
    return false
  }
}

test('qa-reset (DB): golden workflow fully removed, no FK error, unrelated untouched, idempotent', async (t) => {
  const available = await dbAvailable()
  if (!available) {
    t.skip('DEV DB unreachable — run on a machine with DATABASE_URL_DEV configured')
    return
  }

  const goldenDealId = crypto.randomUUID()
  const propertyId = crypto.randomUUID()
  const unrelatedSubject = crypto.randomUUID()
  const tenantId = crypto.randomUUID()

  let goldenInstanceId = ''
  let otherInstanceId = ''

  try {
    const defRows = await interactiveSql`
      insert into process_definitions (tenant_id, key, version, name, description, definition, status, created_by)
      values (${tenantId}, ${`qa-reset-${tenantId}`}, 1, 'qa-reset', null, '{"nodes":[],"edges":[]}'::jsonb, 'active', 'qa-reset-test')
      returning id
    `
    const definitionId = defRows[0].id as string

    // Golden instance: subject_type='deal', subject_id=goldenDealId.
    const golden = await interactiveSql`
      insert into process_instances (tenant_id, definition_id, business_key, status, started_by, subject_type, subject_id)
      values (${tenantId}, ${definitionId}, 'golden', 'active', 'qa-reset-test', 'deal', ${goldenDealId})
      returning id
    `
    goldenInstanceId = golden[0].id as string
    const tokenRows = await interactiveSql`
      insert into tokens (tenant_id, process_instance_id, node_id, status)
      values (${tenantId}, ${goldenInstanceId}, 'start', 'active')
      returning id
    `
    const tokenId = tokenRows[0].id as string
    // task + job both reference the token (no cascade) and the instance (cascade).
    await interactiveSql`
      insert into tasks (tenant_id, process_instance_id, token_id, name, status)
      values (${tenantId}, ${goldenInstanceId}, ${tokenId}, 'golden-task', 'created')
    `
    await interactiveSql`
      insert into jobs (tenant_id, process_instance_id, token_id, type, due_at, status)
      values (${tenantId}, ${goldenInstanceId}, ${tokenId}, 'timer', now(), 'pending')
    `
    await interactiveSql`
      insert into workflow_execution_trace_event (workflow_instance_id, event_type, system, occurred_at)
      values (${goldenInstanceId}, 'WORKFLOW_STARTED', 'workflow', now())
    `
    await interactiveSql`
      insert into process_events (tenant_id, process_instance_id, token_id, event_type)
      values (${tenantId}, ${goldenInstanceId}, ${tokenId}, 'START')
    `


    // Unrelated instance (different subject) that must survive.
    const other = await interactiveSql`
      insert into process_instances (tenant_id, definition_id, business_key, status, started_by, subject_type, subject_id)
      values (${tenantId}, ${definitionId}, 'other', 'active', 'qa-reset-test', 'deal', ${unrelatedSubject})
      returning id
    `
    otherInstanceId = other[0].id as string
    const otherTokenRows = await interactiveSql`
      insert into tokens (tenant_id, process_instance_id, node_id, status)
      values (${tenantId}, ${otherInstanceId}, 'start', 'active')
      returning id
    `
    const otherTokenId = otherTokenRows[0].id as string
    await interactiveSql`
      insert into tasks (tenant_id, process_instance_id, token_id, name, status)
      values (${tenantId}, ${otherInstanceId}, ${otherTokenId}, 'other-task', 'created')
    `
    await interactiveSql`
      insert into workflow_execution_trace_event (workflow_instance_id, event_type, system, occurred_at)
      values (${otherInstanceId}, 'WORKFLOW_STARTED', 'workflow', now())
    `
    await interactiveSql`
      insert into process_events (tenant_id, process_instance_id, token_id, event_type)
      values (${tenantId}, ${otherInstanceId}, ${otherTokenId}, 'START')
    `

    // Reset (atomic). Must NOT throw 23503.
    await neonTx((tx) => resetGoldenData(tx, { dealId: goldenDealId, propertyId }))

    // Golden instance + everything under it is gone.
    const goldenInstances = await interactiveSql`select id from process_instances where id = ${goldenInstanceId}`
    assert.equal(goldenInstances.length, 0, 'golden process_instances must be gone')
    const goldenTokens = await interactiveSql`select id from tokens where process_instance_id = ${goldenInstanceId}`
    assert.equal(goldenTokens.length, 0, 'golden tokens must be gone')
    const goldenTasks = await interactiveSql`select id from tasks where process_instance_id = ${goldenInstanceId}`
    assert.equal(goldenTasks.length, 0, 'golden tasks must be gone')
    const goldenJobs = await interactiveSql`select id from jobs where process_instance_id = ${goldenInstanceId}`
    assert.equal(goldenJobs.length, 0, 'golden jobs must be gone')
    const goldenTrace = await interactiveSql`select id from workflow_execution_trace_event where workflow_instance_id = ${goldenInstanceId}`
    assert.equal(goldenTrace.length, 0, 'golden trace rows must be gone')
    const goldenEvents = await interactiveSql`select id from process_events where process_instance_id = ${goldenInstanceId}`
    assert.equal(goldenEvents.length, 0, 'golden process_events must be gone')

    // Unrelated DEV rows untouched.
    const otherInstances = await interactiveSql`select id from process_instances where id = ${otherInstanceId}`
    assert.equal(otherInstances.length, 1, 'unrelated instance must survive')
    const otherTokens = await interactiveSql`select id from tokens where process_instance_id = ${otherInstanceId}`
    assert.equal(otherTokens.length, 1, 'unrelated token must survive')
    const otherTasks = await interactiveSql`select id from tasks where process_instance_id = ${otherInstanceId}`
    assert.equal(otherTasks.length, 1, 'unrelated task must survive')
    const otherTrace = await interactiveSql`select id from workflow_execution_trace_event where workflow_instance_id = ${otherInstanceId}`
    assert.equal(otherTrace.length, 1, 'unrelated trace must survive')

    // Second reset is a safe no-op (nothing to reset).
    await neonTx((tx) => resetGoldenData(tx, { dealId: goldenDealId, propertyId }))
  } finally {
    if (otherInstanceId) {
      await interactiveSql`delete from workflow_execution_trace_event where workflow_instance_id = ${otherInstanceId}`
    }
    await interactiveSql`delete from process_events where tenant_id = ${tenantId}`
    await interactiveSql`delete from process_instances where tenant_id = ${tenantId}`
    await interactiveSql`delete from process_definitions where tenant_id = ${tenantId}`
  }
})

