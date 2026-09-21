import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createForgeReleaseOperations, type ForgeReleasePool } from '@/legacy/workflow_app/forge/release-operations'

// ---------------------------------------------------------------------------
// ENG-FORGE-VERIFY-IDENTITY-01 — a verification names the exact object and the
// exact attempt it verified.
//
// Two defects are reproduced first, then closed:
//   1. the verifier joined `pg_matviews` on `matviewname` ALONE, so a same-named
//      materialized view in another schema was accepted as the verified object;
//   2. the refresh receipt was read by story/target/model only, so a receipt
//      minted by an earlier release attempt satisfied a later verification.
//
// The pool below models the two durable rows `verifyDerived` reads — the
// populated materialized views and the refresh receipts — and answers three
// query shapes: the verifier's schema+name+attempt lookup, a name-only probe
// (the old match), and an attempt-blind receipt probe (the old lookup).
// ---------------------------------------------------------------------------

type Receipt = {
  story: string
  target: string
  model: string
  commandId: string
  success: boolean
}

function identityPool(fixture: { populated: string[]; receipts: Receipt[] }): ForgeReleasePool {
  const populated = new Set(fixture.populated)
  return {
    async query(text: string, params: unknown[] = []) {
      if (/pg_matviews/i.test(text)) {
        if (/schemaname\s*=\s*\$/i.test(text)) {
          const [story, target, model, name, schema, commandId] = params as [
            string,
            string,
            string,
            string,
            string,
            string,
          ]
          const view = populated.has(`${schema}.${name}`)
          const receipt = fixture.receipts.some(
            (r) =>
              r.story === story &&
              r.target === target &&
              r.model === model &&
              r.commandId === commandId &&
              r.success,
          )
          const hit = view && receipt
          return { rows: hit ? [{}] : [], rowCount: hit ? 1 : 0 }
        }
        // The old name-only match: schema is not part of the predicate.
        const name = String(params[0])
        const hit = [...populated].some((key) => key.split('.')[1] === name)
        return { rows: hit ? [{}] : [], rowCount: hit ? 1 : 0 }
      }
      if (/forge_derived_refresh_execution/i.test(text)) {
        // The old attempt-blind lookup: story/target/model and success only.
        const [story, target, model] = params as [string, string, string]
        const hit = fixture.receipts.some(
          (r) => r.story === story && r.target === target && r.model === model && r.success,
        )
        return { rows: hit ? [{}] : [], rowCount: hit ? 1 : 0 }
      }
      return { rows: [], rowCount: 0 }
    },
    async end() {},
  }
}

const ops = (pool: ForgeReleasePool) => createForgeReleaseOperations({ poolForTarget: () => pool })

const staleReceipt: Receipt = {
  story: 's1',
  target: 'prod',
  model: 'public.mv_clients',
  commandId: 'attempt-1',
  success: true,
}

test('verify-identity: the duplicate-name case is confirmed', async () => {
  const pool = identityPool({ populated: ['staging.mv_clients'], receipts: [staleReceipt] })
  const nameOnly = await pool.query(
    'select 1 from pg_matviews where matviewname = $1 and ispopulated = true',
    ['mv_clients'],
  )
  assert.equal(
    nameOnly.rowCount,
    1,
    'a name-only match accepts the same-named view in another schema — the defect',
  )
})

test('verify-identity: a duplicate-named materialized view in another schema is refused', async () => {
  const pool = identityPool({ populated: ['staging.mv_clients'], receipts: [staleReceipt] })
  const result = await ops(pool).verifyDerived({
    storyId: 's1',
    target: 'prod',
    models: ['public.mv_clients'],
    attemptCommandId: 'attempt-1',
  })
  assert.equal(result.success, false, result.detail)
})

test('verify-identity: a materialized view in the declared schema is accepted', async () => {
  const pool = identityPool({ populated: ['public.mv_clients'], receipts: [staleReceipt] })
  const result = await ops(pool).verifyDerived({
    storyId: 's1',
    target: 'prod',
    models: ['public.mv_clients'],
    attemptCommandId: 'attempt-1',
  })
  assert.equal(result.success, true, result.detail)
})

test('verify-identity: the stale-receipt case is confirmed', async () => {
  const pool = identityPool({ populated: ['public.mv_clients'], receipts: [staleReceipt] })
  const attemptBlind = await pool.query(
    'select 1 from forge_derived_refresh_execution where story_id = $1 and target = $2 and model_name = $3 and success = true',
    ['s1', 'prod', 'public.mv_clients'],
  )
  assert.equal(
    attemptBlind.rowCount,
    1,
    'an attempt-blind lookup accepts a receipt minted by an earlier attempt — the defect',
  )
})

test('verify-identity: a refresh receipt minted by an earlier attempt is refused', async () => {
  const pool = identityPool({ populated: ['public.mv_clients'], receipts: [staleReceipt] })
  const result = await ops(pool).verifyDerived({
    storyId: 's1',
    target: 'prod',
    models: ['public.mv_clients'],
    attemptCommandId: 'attempt-2',
  })
  assert.equal(result.success, false, result.detail)
})

test('verify-identity: a refresh receipt minted for the current attempt is accepted', async () => {
  const pool = identityPool({
    populated: ['public.mv_clients'],
    receipts: [{ ...staleReceipt, commandId: 'attempt-2' }],
  })
  const result = await ops(pool).verifyDerived({
    storyId: 's1',
    target: 'prod',
    models: ['public.mv_clients'],
    attemptCommandId: 'attempt-2',
  })
  assert.equal(result.success, true, result.detail)
})
