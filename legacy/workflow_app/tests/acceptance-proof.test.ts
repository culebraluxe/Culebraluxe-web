import assert from 'node:assert/strict'
import test from 'node:test'

import { collectAssayEvidence } from '@/legacy/workflow_app/forge/agents/assay-collect'
import type { RoleEffectPorts } from '@/legacy/workflow_app/forge/agents/ports'
import { adjudicateAssay, runAssayCommands } from '@/legacy/workflow_app/forge/agents/qa/run'
import {
  acceptanceMapChanged,
  buildAcceptanceMap,
  type AcceptanceCondition,
  type CommandResult,
} from '@/legacy/workflow_app/forge/agents/qa/types'
import { storyReadyToRunReasons } from '@/legacy/workflow_app/forge/forge-ready-gate'
import { buildStoryAcceptanceMap } from '@/legacy/workflow_app/forge/lead-routing-context'

// ---------------------------------------------------------------------------
// ENG-FORGE-ACCEPTANCE-PROOF-01 — THE PROOF ASSERTS THE ACCEPTANCE.
//
// Measured 2026-09-16 on WORKSHOP-TOOLS-01: qaPassed=true, story Complete 100, and the
// acceptance was NOT met. The lane authored the proof, so the proof tested the cheaper
// reading, and QA honestly ruled that the tests passed. QA measures what it is given;
// nothing checked that the given thing was the acceptance.
//
// These lock the fix at the QA seam: a clause with no assertion behind it is UNPROVEN,
// the clause is NAMED, the mapping is frozen before the work, and a lane that changes it
// is recorded as having done so. The proof must FAIL at the base ref — the base
// adjudicator knew only PASS/FAIL and would have returned PASS for the first case below.
// ---------------------------------------------------------------------------

const COMMAND = 'node --import tsx --test workflow_app/tests/acceptance-proof.test.ts'

// The proof's EXECUTED OUTPUT. A mapped assertion is satisfied only when it actually RAN, so the fixture
// carries the pass line for the ref the mapped clauses declare — a bare `excerpt: 'ok'` names no assertion
// and would read as UNPROVEN.
const ok = (command: string): CommandResult => ({
  command,
  exitCode: 0,
  passed: true,
  excerpt: 'ok',
  output: '\u2714 asserts-wired-flag (0.3ms)\n\u2139 tests 1\n\u2139 pass 1\n',
})

const unmapped = (id: string, text: string): AcceptanceCondition => ({ id, text, assertions: [] })
const mapped = (id: string, text: string, assertion: string): AcceptanceCondition => ({
  id,
  text,
  assertions: [assertion],
})

// ---------------------------------------------------------------------------
// 1. THE CASE THAT BIT: a proof that passes while an acceptance condition is unmet
//    comes back UNPROVEN, not PASS — and the condition is named.
// ---------------------------------------------------------------------------
test('a passing proof with an acceptance condition that has no assertion is UNPROVEN, not PASS', () => {
  const conditions = [unmapped('wired-flag', 'a tool whose wired flag is false must become wired')]
  const plan = { commands: [COMMAND], conditions }
  const results = runAssayCommands(plan, ok)

  // The proof itself is green. That is exactly the trap: green is not the acceptance.
  assert.equal(results[0]?.passed, true, 'the frozen proof passes')

  const report = adjudicateAssay({ plan, commands: results })
  assert.equal(report.verdict, 'UNPROVEN', 'a green proof with an unmapped clause is not a PASS')
  assert.ok(report.unproven.includes('wired-flag'), 'the untested condition is named')
  assert.ok(
    report.blockers.includes('UNPROVEN wired-flag'),
    'the report names the condition on the blockers',
  )
})

// ---------------------------------------------------------------------------
// 2. The same proof, fully mapped, is a PASS. The verdict is not a blanket refusal.
// ---------------------------------------------------------------------------
test('a fully-mapped acceptance with passing commands is a PASS', () => {
  const conditions = [mapped('wired-flag', 'the wired flag must become wired', 'asserts-wired-flag')]
  const plan = { commands: [COMMAND], conditions }
  const results = runAssayCommands(plan, ok)
  const report = adjudicateAssay({ plan, commands: results })
  assert.equal(report.verdict, 'PASS')
  assert.deepEqual(report.unproven, [])
  assert.deepEqual(report.blockers, [])
})

// ---------------------------------------------------------------------------
// 3. A real command failure still outranks an unmapped clause, but the untested
//    clause is NOT hidden behind the failure — both are recorded.
// ---------------------------------------------------------------------------
test('a failed command is FAIL, and an unmapped clause is still named in the blockers', () => {
  const conditions = [unmapped('wired-flag', 'the wired flag must become wired')]
  const plan = { commands: [COMMAND], conditions }
  const results = runAssayCommands(plan, (command) => ({ ...ok(command), exitCode: 1, passed: false }))
  const report = adjudicateAssay({ plan, commands: results })
  assert.equal(report.verdict, 'FAIL')
  assert.ok(report.blockers.includes(`CMD_FAIL ${COMMAND}`))
  assert.ok(report.blockers.includes('UNPROVEN wired-flag'), 'the gap is not hidden by the failure')
})

// ---------------------------------------------------------------------------
// 4. THE COLLECTOR. A mapped story records qaPassed false and NAMES the condition;
//    an ABSENT mapping is UNPROVEN too — never a pass.
// ---------------------------------------------------------------------------
const collectorPorts = (over: Partial<RoleEffectPorts>): RoleEffectPorts =>
  ({ assayCommands: ['echo the frozen proof'], runCommand: ok, ...over }) as unknown as RoleEffectPorts

test('the collector records UNPROVEN and names the unmapped condition on the row', () => {
  const acceptanceMap = buildAcceptanceMap({ acceptance: ['a clause with no assertion'] })
  const evidence = collectAssayEvidence(
    {} as never,
    collectorPorts({ acceptanceMap }),
  )
  assert.equal(evidence.qaPassed, false, 'an unmapped clause is never a pass')
  assert.match(evidence.deliverableRejection ?? '', /UNPROVEN/)
  assert.match(evidence.deliverableRejection ?? '', /unproven=\[/)
  assert.match(evidence.deliverableRejection ?? '', /a-clause-with-no-assertion/)
})

test('an absent acceptance mapping is UNPROVEN, never a pass', () => {
  const evidence = collectAssayEvidence({} as never, collectorPorts({}))
  assert.equal(evidence.qaPassed, false, 'no mapping means no assertion behind any clause')
  assert.match(evidence.deliverableRejection ?? '', /acceptance-map-missing/)
})

// ---------------------------------------------------------------------------
// 5. THE MAPPING IS FROZEN BEFORE THE WORK. A lane that changes it is RECORDED,
//    never silently re-derived.
// ---------------------------------------------------------------------------
test('a mapping changed after it was frozen is recorded as changed', () => {
  const frozen = buildAcceptanceMap({
    acceptance: ['the wired flag must become wired'],
    assertions: { 'the wired flag must become wired': ['asserts-wired-flag'] },
  })
  assert.equal(frozen.conditions[0]?.assertions.length, 1)
  assert.equal(frozen.hash.length > 0, true, 'the frozen mapping carries its hash')

  const changed = [unmapped('the-wired-flag-must-become-wired', 'the wired flag must become wired')]
  assert.equal(acceptanceMapChanged(frozen, changed), true, 'the change is detected')

  const report = adjudicateAssay({
    plan: { commands: [COMMAND], conditions: changed },
    commands: [ok(COMMAND)],
    frozenMap: frozen,
  })
  assert.ok(report.blockers.includes('ACCEPTANCE_MAP_CHANGED'), 'the change is recorded')
  assert.equal(report.verdict, 'UNPROVEN')

  const unchanged = adjudicateAssay({
    plan: { commands: [COMMAND], conditions: frozen.conditions },
    commands: [ok(COMMAND)],
    frozenMap: frozen,
  })
  assert.equal(unchanged.verdict, 'PASS', 'an untouched mapping still passes')
})

// ---------------------------------------------------------------------------
// 6. THE MAPPING IS BUILT BEFORE THE WORK (Lead routing), from the story's own
//    acceptance and the Architect-declared assertions. A clause with no declared
//    assertion is carried unmapped — so it can only come back UNPROVEN.
// ---------------------------------------------------------------------------
test('the pre-work mapping carries a declared assertion per clause and leaves the rest unmapped', () => {
  const map = buildStoryAcceptanceMap({
    id: 'ENG-FORGE-ACCEPTANCE-PROOF-01',
    acceptanceCriteria: '- one clause\n- another clause',
    architectBrief:
      'FORGE_ARCHITECT_CONTRACT: {"filesInScope":["legacy/workflow_app/tests"],"acceptance":["one clause","another clause"],"acceptanceAssertions":{"one clause":["asserts-one"]},"leadHint":"SMITH"}',
  })
  assert.equal(map.conditions.length, 2)
  assert.deepEqual(map.conditions[0]?.assertions, ['asserts-one'])
  assert.deepEqual(map.conditions[1]?.assertions, [], 'a clause with no declared assertion is unmapped')
})

// ---------------------------------------------------------------------------
// 7. THE READY GATE refuses a story whose declared mapping leaves a clause
//    unmapped — it can only ever be UNPROVEN, so it must not start a run.
// ---------------------------------------------------------------------------
test('the ready gate refuses a declared mapping that leaves a clause unmapped', () => {
  const base = {
    workType: 'FEATURE',
    acceptanceCriteria: 'a',
    assayCommands: '- `t`',
  }
  assert.deepEqual(storyReadyToRunReasons({ ...base, acceptanceAssertions: { a: [] } }), [
    'ready-gate:unmapped-acceptance',
  ])
  assert.deepEqual(storyReadyToRunReasons({ ...base, acceptanceAssertions: { a: ['asserts-a'] } }), [])
  assert.deepEqual(storyReadyToRunReasons(base), [], 'an absent mapping is not gated here')
})
