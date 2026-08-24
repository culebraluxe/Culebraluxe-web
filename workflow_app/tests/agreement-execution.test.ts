import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  evaluateAgreementExecution,
  agreementExecutionTransition,
  resolveRequiredExecutionRoles,
  AGREEMENT_FULLY_EXECUTED,
} from '../../lib/agreements/execution'

const ROLES = ['BUYER', 'SELLER', 'SELLER_BROKER']

test('CRM-27: partial required signatures are NOT fully executed', () => {
  const verdict = evaluateAgreementExecution({
    documentVersion: 'PR-PNS-v1',
    requiredRoles: ROLES,
    satisfiedRoles: ['BUYER'],
    manuallyExecuted: false,
  })
  assert.equal(verdict.fullyExecuted, false)
  assert.equal(verdict.reason, 'missing_required_roles')
  assert.deepEqual(verdict.missingRoles, ['SELLER', 'SELLER_BROKER'])
})

test('CRM-27: final required evidence -> fully executed', () => {
  const verdict = evaluateAgreementExecution({
    documentVersion: 'PR-PNS-v1',
    requiredRoles: ROLES,
    satisfiedRoles: ['BUYER', 'SELLER', 'SELLER_BROKER'],
    manuallyExecuted: false,
  })
  assert.equal(verdict.fullyExecuted, true)
  assert.equal(verdict.reason, 'all_required_roles_satisfied')
  assert.deepEqual(verdict.missingRoles, [])
})

test('CRM-27: authorized manual/external evidence satisfies the predicate', () => {
  const verdict = evaluateAgreementExecution({
    documentVersion: 'PR-PNS-v1',
    requiredRoles: ROLES,
    satisfiedRoles: [],
    manuallyExecuted: true,
  })
  assert.equal(verdict.fullyExecuted, true)
  assert.equal(verdict.reason, 'manual_execution')
})

test('CRM-27: optional vs required role policy is respected', () => {
  const requiredRoles = resolveRequiredExecutionRoles('PR-PNS', ROLES)
  assert.deepEqual([...requiredRoles].sort(), [...ROLES].sort())
  // A policy that marks SELLER_BROKER optional -> satisfied without it.
  const verdict = evaluateAgreementExecution({
    documentVersion: 'PR-PNS-v1',
    requiredRoles: ['BUYER', 'SELLER'],
    satisfiedRoles: ['BUYER', 'SELLER'],
    manuallyExecuted: false,
  })
  assert.equal(verdict.fullyExecuted, true)
})

test('CRM-27: transition is idempotent — becameFullyExecuted exactly once', () => {
  const partial = evaluateAgreementExecution({
    documentVersion: 'PR-PNS-v1', requiredRoles: ROLES, satisfiedRoles: ['BUYER'], manuallyExecuted: false,
  })
  const full = evaluateAgreementExecution({
    documentVersion: 'PR-PNS-v1', requiredRoles: ROLES, satisfiedRoles: ROLES, manuallyExecuted: false,
  })
  const first = agreementExecutionTransition(partial, full)
  assert.equal(first.becameFullyExecuted, true)
  const replay = agreementExecutionTransition(full, full)
  assert.equal(replay.becameFullyExecuted, false)
})

test('CRM-27: AGREEMENT_FULLY_EXECUTED is a neutral domain event type', () => {
  assert.equal(AGREEMENT_FULLY_EXECUTED, 'AGREEMENT_FULLY_EXECUTED')
})

// ---------------------------------------------------------------------------
// Completion evaluator (evidence assembly + exactly-once marker)
// ---------------------------------------------------------------------------

import {
  evaluateAgreementCompletion,
} from '../../lib/agreements/completion'
import type { QueryExecutor } from '../../db/query-executor'
import type { TxRunner } from '../../db/tx'

function makeExecute(opts: {
  templateId?: string
  issuedVersion?: number
  satisfiedRoles?: string[]
  id?: string
  dealId?: string | null
  documentType?: string
}) {
  const executor: QueryExecutor = async (strings) => {
    const sql = strings.join('?')
    if (sql.includes('from transaction_document')) {
      return [
        {
          id: opts.id ?? 'doc-1',
          template_id: opts.templateId ?? 'PR-PNS',
          issued_version: opts.issuedVersion ?? 1,
          deal_id: opts.dealId ?? 'deal-1',
          document_type: opts.documentType ?? 'agreement',
        },
      ]
    }
    if (sql.includes('from signature_request')) {
      return (opts.satisfiedRoles ?? []).map((role) => ({ execution_role: role }))
    }
    return []
  }
  return executor
}

function makeRun() {
  const seen = new Set<string>()
  const runner: TxRunner = (cb) =>
    cb(async (strings, ...params) => {
      const sql = strings.join('?')
      if (sql.includes('insert into agreement_execution')) {
        const key = `${String(params[0])}:${String(params[1])}`
        if (seen.has(key)) return []
        seen.add(key)
        return [{ id: '1' }]
      }
      return []
    })
  return { runner }
}

test('CRM-27: completion — partial roles never emits', async () => {
  const execute = makeExecute({ templateId: 'PR-PNS', issuedVersion: 1, satisfiedRoles: ['BUYER'] })
  const { runner } = makeRun()
  const result = await evaluateAgreementCompletion('doc-1', 'evt-1', { execute, run: runner })
  assert.equal(result.shouldEmit, false)
  assert.equal(result.verdict.fullyExecuted, false)
  assert.equal(result.document?.issuedVersion, 1)
})

test('CRM-27: completion — fully executed emits exactly once, replay is a no-op', async () => {
  const execute = makeExecute({ templateId: 'PR-PNS', issuedVersion: 1, satisfiedRoles: ROLES })
  const { runner } = makeRun()
  const first = await evaluateAgreementCompletion('doc-1', 'evt-1', { execute, run: runner })
  assert.equal(first.shouldEmit, true)
  assert.equal(first.verdict.fullyExecuted, true)
  const replay = await evaluateAgreementCompletion('doc-1', 'evt-2', { execute, run: runner })
  assert.equal(replay.shouldEmit, false)
})

test('CRM-27: completion — wrong document/version is independently scoped', async () => {
  const executeA = makeExecute({ templateId: 'PR-PNS', issuedVersion: 1, satisfiedRoles: ROLES })
  const { runner } = makeRun()
  const a = await evaluateAgreementCompletion('doc-A', 'evt-A', { execute: executeA, run: runner })
  assert.equal(a.shouldEmit, true)
  const executeB = makeExecute({ templateId: 'PR-PNS', issuedVersion: 1, satisfiedRoles: ['BUYER'] })
  const b = await evaluateAgreementCompletion('doc-B', 'evt-B', { execute: executeB, run: runner })
  assert.equal(b.shouldEmit, false)
  assert.equal(b.verdict.fullyExecuted, false)
})

