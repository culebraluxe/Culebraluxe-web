import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  ARCHITECT_FINDINGS_MISSING,
  MAX_SEAMS_PER_FINDING,
  assessArchitectBrief,
  isRepoRelativeSeam,
} from '../forge/forge-shaping'

// ENG-FORGE-ARCHITECT-BRIEF-01 (slice 1) — the Architect brief is a contract.
//
// The failure this gate exists for: a brief that parsed to zero findings degraded
// quietly to [], so the failure surfaced one lane later as LEAD refusing to route
// ("No required findings supply") — blaming the wrong role with a message its author
// could not act on. These tests pin the fail-closed behaviour AND the diagnostics
// that make attempt 2 self-correct.

const line = (payload: string) => `Here is my plan.\n\nFORGE_FINDINGS_JSON: ${payload}`

const oneFinding = (overrides: Record<string, unknown> = {}) =>
  line(
    JSON.stringify([
      {
        id: 'reuse-shipped-engine',
        summary: 'reuse the shipped calendar engine rather than a second one',
        required: true,
        seams: ['components/portal/projects-workspace.tsx'],
        hint: 'SAME_UNIT',
        ...overrides,
      },
    ]),
  )

test('a complete brief is OK and reports its shape', () => {
  const verdict = assessArchitectBrief(oneFinding())
  assert.equal(verdict.verdict, 'OK')
  assert.equal(verdict.reasons.length, 0)
  assert.equal(verdict.findingCount, 1)
  assert.equal(verdict.seamCount, 1)
})

test('tolerant shapes still pass (fenced / inline) — no regression', () => {
  const fenced = line('[{"id":"a","summary":"s","required":true,"seams":["ui/projects/model.ts"]}]')
  const inline =
    'FORGE_FINDINGS_JSON: [{"id":"a","summary":"s","required":true,"seams":["ui/projects/model.ts"]}]'
  assert.equal(assessArchitectBrief(fenced).verdict, 'OK')
  assert.equal(assessArchitectBrief(inline).verdict, 'OK')
})

test('zero findings is a HOLD that names the exact machine line to emit', () => {
  const verdict = assessArchitectBrief('I looked at the code and it seems fine.')
  assert.equal(verdict.verdict, 'HOLD')
  assert.deepEqual(verdict.reasons, [ARCHITECT_FINDINGS_MISSING])
  assert.match(verdict.reasons[0], /FORGE_FINDINGS_JSON:/)
  assert.equal(verdict.findingCount, 0)
})

test('a truncated payload is a HOLD, not a silent empty brief', () => {
  const verdict = assessArchitectBrief('FORGE_FINDINGS_JSON: [{"id":"a","summary":"s"')
  assert.equal(verdict.verdict, 'HOLD')
  assert.deepEqual(verdict.reasons, [ARCHITECT_FINDINGS_MISSING])
})

test('a dropped row is reported (a partial payload must not shrink the finding set)', () => {
  const verdict = assessArchitectBrief(
    line(
      JSON.stringify([
        { id: 'good', summary: 'ok', required: true, seams: ['ui/projects/model.ts'] },
        { id: 'no-summary', required: true, seams: ['ui/projects/model.ts'] },
      ]),
    ),
  )
  assert.equal(verdict.verdict, 'HOLD')
  assert.match(verdict.reasons.join(' '), /1 of 2 finding rows were dropped/)
  assert.equal(verdict.findingCount, 1)
})

test('a required finding with no seam is a HOLD with an actionable reason', () => {
  const verdict = assessArchitectBrief(oneFinding({ seams: [] }))
  assert.equal(verdict.verdict, 'HOLD')
  assert.match(verdict.reasons.join(' '), /reuse-shipped-engine: declare at least one repository-relative seam/)
})

test('an invalid seam is named so the retry can fix it', () => {
  const verdict = assessArchitectBrief(oneFinding({ seams: ['/abs/path.ts', 'ui/projects/*.ts'] }))
  assert.equal(verdict.verdict, 'HOLD')
  assert.match(verdict.reasons.join(' '), /"\/abs\/path.ts"/)
  assert.match(verdict.reasons.join(' '), /"ui\/projects\/\*\.ts"/)
})

test('a REQUIRED finding over the chunk ceiling must be decomposed', () => {
  const tooMany = ['a.ts', 'b.ts', 'c.ts', 'd.ts']
  const verdict = assessArchitectBrief(oneFinding({ seams: tooMany }))
  assert.equal(verdict.verdict, 'HOLD')
  assert.match(
    verdict.reasons.join(' '),
    new RegExp(`4 seams exceeds the chunk ceiling of ${MAX_SEAMS_PER_FINDING}`),
  )
  assert.match(verdict.reasons.join(' '), /decompose it into separate findings/)
})

test('a NON-required finding may be wide (the ceiling applies to required work only)', () => {
  const verdict = assessArchitectBrief(
    oneFinding({ required: false, seams: ['a.ts', 'b.ts', 'c.ts', 'd.ts'] }),
  )
  assert.equal(verdict.verdict, 'OK')
})

test('path#symbol seams are accepted', () => {
  const verdict = assessArchitectBrief(oneFinding({ seams: ['ui/projects/model.ts#buildWorkPlan'] }))
  assert.equal(verdict.verdict, 'OK')
})

test('isRepoRelativeSeam mirrors the LEAD rules', () => {
  for (const ok of ['ui/projects/model.ts', './ui/projects/model.ts', 'ui/projects/model.ts#fn', 'components/x']) {
    assert.equal(isRepoRelativeSeam(ok), true, ok)
  }
  for (const bad of ['/abs/path.ts', 'ui/*.ts', 'ui/a b.ts', '', '../up.ts', 'ui/../../up.ts', 'C:/x.ts']) {
    assert.equal(isRepoRelativeSeam(bad), false, bad)
  }
})
