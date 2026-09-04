import assert from 'node:assert/strict'
import test from 'node:test'

import {
  assayCandidateFromInstructions,
  assayPlanFromInstructions,
  evaluateAssayEvidence,
  finalizeAssayEvidence,
  parseAssayTestCounters,
  withAssayCandidateDirective,
  withAssayPlanDirective,
  type AssayCommandResult,
} from './assay-evidence'

const CANDIDATE = 'b'.repeat(40)
const COMMAND = 'pnpm exec tsx --test agent-runtime/example.test.ts'

function result(overrides: Partial<AssayCommandResult> = {}): AssayCommandResult {
  return {
    command: COMMAND,
    exitCode: 0,
    signal: null,
    timedOut: false,
    durationMs: 25,
    tests: { total: 26, passed: 26, failed: 0 },
    stdoutTail: '26/26 pass; this prose may literally say failed commands and does not decide truth',
    stderrTail: '',
    ...overrides,
  }
}

function base() {
  return {
    version: 1 as const,
    candidateSha: CANDIDATE,
    verifiedSha: CANDIDATE,
    requiredCommands: [COMMAND],
    commandResults: [result()],
    policyViolations: [] as string[],
    startedAt: '2026-09-04T00:00:00.000Z',
    endedAt: '2026-09-04T00:00:01.000Z',
  }
}

test('26/26 + exit 0 + fail 0 is PASS regardless of prose words', () => {
  const verdict = evaluateAssayEvidence(base())
  assert.deepEqual(verdict, { pass: true, failureCode: null, detail: null })
  assert.equal(finalizeAssayEvidence(base()).verdict, 'PASS')
})

test('non-zero process exit is mathematical failure', () => {
  const evidence = base()
  evidence.commandResults = [result({ exitCode: 1 })]
  const verdict = evaluateAssayEvidence(evidence)
  assert.equal(verdict.pass, false)
  assert.equal(verdict.failureCode, 'ASSAY_TEST_FAILED')
})

test('positive failed count is mathematical failure', () => {
  const evidence = base()
  evidence.commandResults = [
    result({ tests: { total: 26, passed: 25, failed: 1 } }),
  ]
  assert.equal(evaluateAssayEvidence(evidence).pass, false)
})

test('passed must equal total when both counters exist', () => {
  const evidence = base()
  evidence.commandResults = [
    result({ tests: { total: 26, passed: 25, failed: 0 } }),
  ]
  assert.equal(evaluateAssayEvidence(evidence).pass, false)
})

test('candidate SHA mismatch fails before publication', () => {
  const evidence = base()
  evidence.verifiedSha = 'c'.repeat(40)
  const verdict = evaluateAssayEvidence(evidence)
  assert.equal(verdict.pass, false)
  assert.equal(verdict.failureCode, 'CANDIDATE_MISMATCH')
})

test('all required commands must execute in immutable order', () => {
  const evidence = base()
  evidence.requiredCommands = [COMMAND, 'pnpm exec tsc --noEmit']
  assert.equal(evaluateAssayEvidence(evidence).pass, false)

  const wrong = base()
  wrong.commandResults = [result({ command: 'different command' })]
  assert.equal(evaluateAssayEvidence(wrong).pass, false)
})

test('policy violation defeats otherwise clean arithmetic', () => {
  const evidence = base()
  evidence.policyViolations = ['SCOPED mode forbids pnpm test']
  const verdict = evaluateAssayEvidence(evidence)
  assert.equal(verdict.pass, false)
  assert.equal(verdict.failureCode, 'ASSAY_POLICY_FAILED')
})

test('runner counter parser extracts node-style and fraction-style numbers', () => {
  assert.deepEqual(
    parseAssayTestCounters('# tests 26\n# pass 26\n# fail 0'),
    { total: 26, passed: 26, failed: 0 },
  )
  assert.deepEqual(parseAssayTestCounters('25/25 pass, 0 fail'), {
    total: 25,
    passed: 25,
    failed: 0,
  })
})

test('Assay plan and candidate directives round-trip as machine data', () => {
  let instructions = withAssayPlanDirective('human explanation', {
    mode: 'SCOPED',
    commands: [COMMAND],
  })
  instructions = withAssayCandidateDirective(instructions, CANDIDATE)

  assert.deepEqual(assayPlanFromInstructions(instructions), {
    mode: 'SCOPED',
    commands: [COMMAND],
  })
  assert.equal(assayCandidateFromInstructions(instructions), CANDIDATE)
})
