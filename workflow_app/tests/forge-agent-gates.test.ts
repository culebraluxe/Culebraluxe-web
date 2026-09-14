import assert from 'node:assert/strict'
import test from 'node:test'
import { assessArchitectHandoff } from '../forge/agents/architect/assess'
import { parseLeadRouting } from '../forge/forge-lead-routing'
import type { ArchitectHandoff } from '../forge/agents/architect-handoff'
import { adjudicateAssay, runAssay } from '../forge/agents/qa/run'
import { canMove, normalizeStoryBucket } from '../../lib/story-moves'

// ---------------------------------------------------------------------------
// The ARCHITECT handoff assessment is fail-closed: a plan that names a seam that
// does not exist on the pinned baseRef, or that cannot be executed as written,
// must NOT be believed. These are the checks that keep an invented file from
// being routed to Smith.
// ---------------------------------------------------------------------------

const finding = (over: Partial<ArchitectHandoff['findings'][number]> = {}) => ({
  id: 'F1',
  required: true,
  summary: 'one bounded change',
  preconditions: [],
  scope: ['lib/story-moves.ts'],
  postconditions: ['the gate is one way'],
  classes: [],
  risks: [],
  hint: 'SAME_UNIT' as const,
  ...over,
})

const handoff = (findings = [finding()], baseRef = 'a1b2c3d4e5f6'): ArchitectHandoff => ({
  version: 1,
  baseRef,
  findings,
})

test('architect: a clean handoff passes when every seam exists on baseRef', () => {
  const result = assessArchitectHandoff(handoff(), { existsOnBaseRef: () => true })
  assert.equal(result.ok, true)
})

test('architect: a seam that does not exist on baseRef is refused WITH the reason', () => {
  const result = assessArchitectHandoff(handoff(), { existsOnBaseRef: () => false })
  assert.equal(result.ok, false)
  assert.ok(!result.ok && result.reasons.join(' ').includes('does not exist on'))
})

test('architect: duplicate finding ids are refused', () => {
  const result = assessArchitectHandoff(handoff([finding(), finding()]))
  assert.equal(result.ok, false)
  assert.ok(!result.ok && result.reasons.some((r) => r.includes('Duplicate finding ids')))
})

test('architect: an empty scope is refused', () => {
  const result = assessArchitectHandoff(handoff([finding({ scope: [] })]))
  assert.equal(result.ok, false)
  assert.ok(!result.ok && result.reasons.some((r) => r.includes('scope is empty')))
})

test('architect: a plan with no required findings is refused, never silently empty', () => {
  const result = assessArchitectHandoff(handoff([finding({ required: false })]))
  assert.equal(result.ok, false)
  assert.ok(!result.ok && result.reasons.some((r) => r.includes('No required findings')))
})

test('architect: a required HOLD with no named risk is refused', () => {
  const result = assessArchitectHandoff(handoff([finding({ hint: 'HOLD', risks: [] })]))
  assert.equal(result.ok, false)
  assert.ok(!result.ok && result.reasons.some((r) => r.includes('must name a concrete risk')))
})

test('architect: an empty baseRef is refused when existence is enforced', () => {
  const result = assessArchitectHandoff(handoff([finding()], ''), { existsOnBaseRef: () => true })
  assert.equal(result.ok, false)
  assert.ok(!result.ok && result.reasons.some((r) => r.includes('baseRef is required')))
})

// ---------------------------------------------------------------------------
// The ASSAY adjudicator is the QA verdict: no model. An empty plan is never a
// pass, and PASS binds exactly one SHA.
// ---------------------------------------------------------------------------

test('lead routing: the JSON without the marker prefix is accepted (observed live)', () => {
  // The Lead emitted a correct proposal with no `LEAD_ROUTING:` in front of it and
  // the run held, discarding a decision the engine already had in hand.
  const reply =
    'HOLD. The frozen contract is unsatisfiable.\n\n' +
    '{"version":1,"decision":"HOLD","size":"SMALL","sizeReason":"one file","reason":"no legal surface","assignments":[],"mergeChecks":[]}\n\n' +
    'FORGE_PASS_STOP: NEEDS_REVIEW'
  const parsed = parseLeadRouting(reply) as { decision?: string } | null
  assert.equal(parsed?.decision, 'HOLD')
})

test('lead routing: two marker lines stay ambiguous and are refused', () => {
  const reply =
    'LEAD_ROUTING: {"version":1,"decision":"HOLD"}\nLEAD_ROUTING: {"version":1,"decision":"SOLO"}'
  assert.equal(parseLeadRouting(reply), null)
})

test('lead routing: a non-routing object is never mistaken for a proposal', () => {
  const architectHandoff =
    'FORGE_ARCHITECT_HANDOFF: {"version":1,"baseRef":"abc123","findings":[{"id":"F1"}]}'
  assert.equal(parseLeadRouting(architectHandoff), null)

  const wrongVersion = '{"version":2,"decision":"HOLD"}'
  assert.equal(parseLeadRouting(wrongVersion), null)
})

const cmd = (command: string, exitCode: number) => ({
  command,
  exitCode,
  passed: exitCode === 0,
  excerpt: '',
})

test('assay: an empty plan is INCOMPLETE, never PASS', () => {
  const report = adjudicateAssay({ plan: { candidateSha: 'abc123', commands: [] }, commands: [] })
  assert.equal(report.verdict, 'INCOMPLETE')
  assert.equal(report.verifiedSha, null)
})

test('assay: a non-zero command is FAIL and certifies no SHA', () => {
  const command = 'node --test x.test.ts'
  const report = adjudicateAssay({
    plan: { candidateSha: 'abc123', commands: [command] },
    commands: [cmd(command, 1)],
  })
  assert.equal(report.verdict, 'FAIL')
  assert.equal(report.verifiedSha, null)
})

test('assay: PASS binds verifiedSha to the candidate SHA', () => {
  const command = 'node --test x.test.ts'
  const report = runAssay({
    plan: { candidateSha: 'abc123', commands: [command] },
    runCommand: () => cmd(command, 0),
  })
  assert.equal(report.verdict, 'PASS')
  assert.equal(report.verifiedSha, 'abc123')
})

test('assay: the architecture gate fails the story; a skipped arch gate does not', () => {
  const command = 'node --test x.test.ts'
  const failed = adjudicateAssay({
    plan: { candidateSha: 'abc123', commands: [command] },
    commands: [cmd(command, 0)],
    staticGate: { archRan: true, archOk: false, archErrors: ['cycle'] },
  })
  assert.equal(failed.verdict, 'FAIL')

  const skipped = adjudicateAssay({
    plan: { candidateSha: 'abc123', commands: [command] },
    commands: [cmd(command, 0)],
    staticGate: { archRan: false, archOk: false, archErrors: [] },
  })
  assert.equal(skipped.verdict, 'PASS')
})

// ---------------------------------------------------------------------------
// NO GATE (the captain, 2026-09-14).
//
// These tests used to assert the OPPOSITE: that ENGINE could not be left and that
// closed stories could not be dispatched. Those assertions were correct about the
// rule and about what the rule cost - "i dont want any rules ... these are just
// sticky notes on a kahnban in real life i can just pick a sticky note off the white
// board kahnban and move it where ever i want ... this is so annoying". The gate is
// gone, and these now lock the ABSENCE, so nobody quietly reintroduces a table of
// forbidden pairs.
//
// What replaced the rules is information, not permission: a move's CONSEQUENCE is
// still published (`bucketSideEffect`), and pulling a story out of ENGINE RUN Q
// withdraws the queue entry it created (see `withdrawQueuedAgentWork`).
// ---------------------------------------------------------------------------
test('no gate: every column may move to every other column', () => {
  const buckets = ['backlog', 'open', 'bench', 'batch', 'engine', 'closed', 'next'] as const
  for (const from of buckets) {
    for (const to of buckets) {
      assert.equal(canMove(from, to), from !== to, `${from} -> ${to}`)
    }
  }
})

test('the engine column is not a one-way door any more', () => {
  assert.equal(canMove('engine', 'open'), true)
  assert.equal(canMove('engine', 'bench'), true)
  assert.equal(canMove('engine', 'batch'), true)
  // A no-op move is still refused, because there is nothing to do.
  assert.equal(canMove('engine', 'engine'), false)
})

test('a finished or deferred story can be sent straight to the engine', () => {
  assert.equal(canMove('closed', 'engine'), true)
  assert.equal(canMove('closed', 'batch'), true)
  assert.equal(canMove('next', 'engine'), true)
  assert.equal(canMove('backlog', 'closed'), true)
})

// ---------------------------------------------------------------------------
// THE SORTER'S BOUNDARY, where two vocabularies meet.
//
// The rules are keyed by BUCKET (`next`); the screen names its column by
// LIFECYCLE (`next-version`). A cast hid the difference, `MOVES['next-version']`
// was `undefined`, and every drag out of NEXT VERSION was refused by a rule that
// read as arbitrary. These lock the translation and the captain's stated rule:
// any story sitting in BACKLOG, OPEN, WORK BENCH or NEXT VERSION may be handed to
// the engine - staged (ENGINE BATCH) or run (ENGINE RUN Q).
// ---------------------------------------------------------------------------
test('sorter boundary: the view column name normalizes to the rule bucket', () => {
  assert.equal(normalizeStoryBucket('next-version'), 'next')
  assert.equal(normalizeStoryBucket('NEXT VERSION'), 'next')
  assert.equal(normalizeStoryBucket('work bench'), 'bench')
  assert.equal(normalizeStoryBucket('engine-queue'), 'engine')
  assert.equal(normalizeStoryBucket('bench'), 'bench')
  // Unrecognized names are refused BY NAME, never quietly accepted as a move.
  assert.equal(normalizeStoryBucket('nonsense'), null)
  assert.equal(normalizeStoryBucket(''), null)
})

test('the captain gets to the engine from every column he named', () => {
  for (const from of ['backlog', 'open', 'bench', 'next', 'closed'] as const) {
    assert.equal(canMove(from, 'batch'), true, `${from} -> batch`)
    assert.equal(canMove(from, 'engine'), true, `${from} -> engine`)
  }
  // Staging is not dispatch: the batch can be pulled back, and so can the run queue now.
  assert.equal(canMove('batch', 'open'), true)
  assert.equal(canMove('engine', 'open'), true)
})
