import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import {
  DECISION_INJECTION_CAP,
  decisionDomainForStory,
  decisionMirrorPath,
  decisionWritePolicy,
  isValidDecisionKey,
  laneNeedsDecisions,
  parseDecisionFile,
  renderDecisionBlock,
  renderDecisionFile,
  validateStatement,
  type ForgeDecision,
} from '@/lib/forge-decision'
import {
  DECISION_STORE_UNAVAILABLE,
  withDecisionContext,
} from '@/agent-runtime/repo-context'
import {
  DecisionError,
  assertDecisionWrite,
  insertDecisionCandidate,
  listActiveDecisions,
  promoteDecision,
} from '@/legacy/db/forge-decision'
import { checkMirror } from '@/scripts/forge-decision'
import type { QueryExecutor, QueryRow } from '@/legacy/db/query-executor'

// ---------------------------------------------------------------------------
// ENG-FORGE-FACTORY-01 PHASE 2 — the decision institution.
//
// The packet's rules are about ROLES, so most of this file is about refusals: who may insert, who may
// promote, who may only read. The other half is the injector, and its acceptance test is specific -
// "the injector reads forge_decision status=active and not MEMORY.md" - which is asserted twice: the SQL
// it sends, and the text it produces.
// ---------------------------------------------------------------------------

const ACTIVE_ROW = {
  id: 'd-1',
  key: 'batch-table-is-job-stream',
  statement: 'The ENGINE BATCH table is the job stream: if a row is in it, it goes.',
  status: 'active',
  owner: 'captain',
  evidence_sha: 'probe-batch-sync',
  supersedes_id: null,
  source: 'captain',
  domain: 'forge',
  created_at: '2026-09-01T00:00:00.000Z',
  promoted_at: '2026-09-15T00:00:00.000Z',
  superseded_at: null,
} as unknown as QueryRow

function recorder(rows: QueryRow[] = []): { tx: QueryExecutor; sql: string[] } {
  const sql: string[] = []
  const tx: QueryExecutor = (strings, ...params) => {
    const text = strings.reduce((acc, part, i) => acc + part + (i < params.length ? `$${i + 1}` : ''), '')
    sql.push(text.replace(/\s+/g, ' ').trim())
    return Promise.resolve(rows)
  }
  return { tx, sql }
}

// --- the write policy: who may do what ---------------------------------------

test('Scout inserts candidates and nothing else', () => {
  assert.equal(decisionWritePolicy('scout', 'insert-candidate').allowed, true)
  for (const action of ['promote', 'supersede', 'flag-stale'] as const) {
    const verdict = decisionWritePolicy('scout', action)
    assert.equal(verdict.allowed, false, `Scout must not ${action}`)
    assert.ok(verdict.reason.length > 10, 'a refusal must explain itself')
  }
})

test('Smith may not write a decision at all — the implementer does not author the rule', () => {
  for (const action of ['insert-candidate', 'promote', 'supersede', 'flag-stale'] as const) {
    const verdict = decisionWritePolicy('smith', action)
    assert.equal(verdict.allowed, false, `Smith must not ${action}`)
  }
  assert.match(decisionWritePolicy('smith', 'promote').reason, /promoted by its implementer/)
})

test('Architect and the captain promote; Inspector flags but never promotes', () => {
  assert.equal(decisionWritePolicy('architect', 'promote').allowed, true)
  assert.equal(decisionWritePolicy('architect', 'supersede').allowed, true)
  assert.equal(decisionWritePolicy('architect', 'flag-stale').allowed, false)
  assert.equal(decisionWritePolicy('inspector', 'flag-stale').allowed, true)
  assert.equal(decisionWritePolicy('inspector', 'promote').allowed, false)
  assert.equal(decisionWritePolicy('captain', 'promote').allowed, true)
  assert.equal(decisionWritePolicy('human', 'supersede').allowed, true)
  // Lead reads decisions; it does not write them, and the reason says so.
  assert.equal(decisionWritePolicy('lead', 'promote').allowed, false)
  assert.match(decisionWritePolicy('lead', 'insert-candidate').reason, /reads decisions/)
})

test('a denied write throws before any SQL is sent', async () => {
  const { tx, sql } = recorder()
  assert.throws(() => assertDecisionWrite('smith', 'promote'), (error: unknown) => {
    assert.ok(error instanceof DecisionError)
    assert.equal((error as DecisionError).code, 'policy-denied')
    return true
  })
  await assert.rejects(
    () => insertDecisionCandidate({ key: 'x-y', statement: 'Smith tries to write a rule.' }, { role: 'smith' }, tx),
    (error: unknown) => error instanceof DecisionError && (error as DecisionError).code === 'policy-denied',
  )
  assert.deepEqual(sql, [], 'a refused write must not reach the database')
})

// --- shape rules -------------------------------------------------------------

test('one sentence per row, enforced', () => {
  assert.deepEqual(validateStatement('The table is the job stream.'), [])
  assert.match(validateStatement('First sentence. Second sentence.').join(' '), /more than one sentence/)
  assert.match(validateStatement('x'.repeat(300)).join(' '), /cap is 240/)
  assert.match(validateStatement('   ').join(' '), /needs a statement/)
  assert.equal(isValidDecisionKey('batch-table-is-job-stream'), true)
  assert.equal(isValidDecisionKey('Has Spaces'), false)
  assert.equal(isValidDecisionKey('UPPER'), false)
})

test('promotion is refused when the row is already active or superseded', async () => {
  const active = recorder([ACTIVE_ROW])
  await assert.rejects(
    () => promoteDecision('batch-table-is-job-stream', { role: 'captain', name: 'captain' }, {}, active.tx),
    (error: unknown) => error instanceof DecisionError && (error as DecisionError).code === 'invalid',
  )
  const superseded = recorder([{ ...ACTIVE_ROW, status: 'superseded' } as QueryRow])
  await assert.rejects(
    () => promoteDecision('x', { role: 'architect', name: 'architect' }, {}, superseded.tx),
    (error: unknown) => error instanceof DecisionError && /write a new candidate/.test((error as Error).message),
  )
})

// --- the injector: the packet's acceptance test -------------------------------

test('the injector reads forge_decision status=active — and never MEMORY.md', async () => {
  const { tx, sql } = recorder([ACTIVE_ROW])
  const decisions = await listActiveDecisions('forge', { limit: DECISION_INJECTION_CAP }, tx)

  assert.equal(decisions.length, 1)
  assert.equal(decisions[0].key, 'batch-table-is-job-stream')
  assert.match(sql[0], /from forge_decision/)
  assert.match(sql[0], /status = 'active'/)
  assert.match(sql[0], /order by promoted_at desc/)
  // The acceptance line, mechanically: nothing in this read can be a memory file.
  assert.ok(!/memory/i.test(sql[0]), `the injector must not read MEMORY.md, sent: ${sql[0]}`)
})

test('the injected block is binding, ordered, and names where disagreement goes', () => {
  const block = renderDecisionBlock([
    { key: 'newer-rule', statement: 'Newer rules lead.', owner: 'captain', promotedAt: '2026-09-15' },
    { key: 'older-rule', statement: 'Older rules follow.', owner: null, promotedAt: '2026-09-01' },
  ])
  assert.match(block, /ACTIVE DECISIONS \(2, newest promoted first\)/)
  assert.match(block, /IN FORCE/)
  assert.ok(block.indexOf('newer-rule') < block.indexOf('older-rule'), 'the caller order is preserved')
  assert.ok(!block.includes('MEMORY.md'), 'the block does not point at the memory file as authority')
  assert.match(block, /open a learn item/)
  assert.equal(renderDecisionBlock([]), '')
})

test('a failed store read is visible in the prompt, not silent', () => {
  const text = withDecisionContext('Lane=smith', { readFailed: true })
  assert.match(text ?? '', /Lane=smith/)
  assert.match(text ?? '', new RegExp(DECISION_STORE_UNAVAILABLE.slice(0, 40)))
  assert.ok(!/MEMORY\.md/.test(text ?? ''))
  // ...and zero active decisions is a different fact from a failure: nothing is appended.
  assert.equal(withDecisionContext('Lane=smith', { decisions: [] }), 'Lane=smith')
})

test('the lanes that decide or obey receive decisions; the rest do not', () => {
  // architect writes the contract, inspector flags stale decisions: a rule neither of them can read is a
  // rule they will contradict in prose (Grok, 2026-09-15).
  for (const lane of ['lead', 'smith', 'night', 'architect', 'inspector']) {
    assert.equal(laneNeedsDecisions(lane), true, `${lane} must receive the decision block`)
  }
  for (const lane of ['scout', 'assay', 'dev_ops', null]) {
    assert.equal(laneNeedsDecisions(lane), false, `${lane} should not receive the decision block`)
  }
})

test('story workstreams map onto the four decision domains', () => {
  assert.equal(decisionDomainForStory({ workstream: 'ENGINEERING' }), 'forge')
  assert.equal(decisionDomainForStory({ workstream: 'CRM', operatingSurface: 'Deals' }), 'crm')
  assert.equal(decisionDomainForStory({ workstream: 'WEB' }), 'web')
  assert.equal(decisionDomainForStory({ workstream: 'OPS', operatingSurface: 'Accounting' }), 'ops')
  assert.equal(decisionDomainForStory({}), 'forge')
})

// --- the git mirror: the file and the row cannot disagree silently ------------

const asDecision = (over: Partial<ForgeDecision> = {}): ForgeDecision => ({
  id: 'd-1',
  key: 'intent-is-not-status',
  statement: 'Intent is recorded in its own column or table; a status is evidence of work that happened.',
  status: 'active',
  owner: 'captain',
  evidenceSha: 'board repair',
  supersedesId: null,
  source: 'captain',
  domain: 'forge',
  createdAt: '2026-09-15T00:00:00.000Z',
  promotedAt: '2026-09-15T00:00:00.000Z',
  supersededAt: null,
  ...over,
})

test('the mirror file round-trips through its parser', () => {
  const decision = asDecision()
  const parsed = parseDecisionFile(renderDecisionFile(decision))
  assert.equal(parsed.key, decision.key)
  assert.equal(parsed.status, 'active')
  assert.equal(parsed.domain, 'forge')
  assert.equal(parsed.owner, 'captain')
  assert.equal(parsed.statement, decision.statement)
})

test('checkMirror catches a missing file, a hand edit and an orphan file', () => {
  const root = mkdtempSync(join(tmpdir(), 'forge-decisions-'))
  const decision = asDecision()

  const missing = checkMirror(root, [decision])
  assert.equal(missing.length, 1)
  assert.match(missing[0].message, /is missing/)

  mkdirSync(join(root, 'docs/agent/decisions'), { recursive: true })
  writeFileSync(join(root, decisionMirrorPath(decision.key)), renderDecisionFile(decision), 'utf8')
  assert.deepEqual(checkMirror(root, [decision]), [])

  // A hand edit to the statement is the failure that matters: the file would read as authority.
  writeFileSync(
    join(root, decisionMirrorPath(decision.key)),
    renderDecisionFile({ ...decision, statement: 'Something the row does not say.' }),
    'utf8',
  )
  const edited = checkMirror(root, [decision])
  assert.equal(edited.length, 1)
  assert.match(edited[0].message, /statement differs/)

  // An orphan file — a decision that exists only in git — is the other half of the split brain.
  writeFileSync(join(root, 'docs/agent/decisions/never-promoted.md'), '# never-promoted\n', 'utf8')
  const orphans = checkMirror(root, [decision])
  assert.ok(orphans.some((finding) => /no row in forge_decision/.test(finding.message)))
})

test('a candidate does not need a mirror file (the mirror is written on promote)', () => {
  const root = mkdtempSync(join(tmpdir(), 'forge-decisions-'))
  assert.deepEqual(checkMirror(root, [asDecision({ status: 'candidate', promotedAt: null })]), [])
})
