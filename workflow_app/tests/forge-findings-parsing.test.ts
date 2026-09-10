import assert from 'node:assert/strict'
import test from 'node:test'

import { findingsFromArchitectEvidence } from '../forge/forge-shaping'

// REGRESSION (2026-09-10, observed live on ENG-FORGE-SPLIT-DOGFOOD-01):
// the architect emitted the findings marker followed by a FENCED, multiline JSON
// block. The old parser required `FORGE_FINDINGS_JSON:` to be followed immediately
// by `[` and to end at a line end, so it found NOTHING — the Lead's routing context
// then had zero findings, the validator refused every non-HOLD proposal ("No
// required findings supplied"), and the story parked at the human gate.
//
// A model-authored marker must be parsed in whatever shape the model chooses.

const FENCED = [
  'Some prose from the architect about the contract.',
  '',
  'FORGE_FINDINGS_JSON:',
  '```json',
  '[',
  '  {"id":"unit-a","summary":"sibling scope","required":true,"seams":["a.ts"],"hint":"SPLIT_CHILD"},',
  '  {"id":"unit-b","summary":"join reason","required":true,"seams":["b.ts"],"hint":"SPLIT_CHILD"}',
  ']',
  '```',
  '',
  'Trailing prose.',
].join('\n')

test('a fenced, multiline findings payload is parsed (the live failure shape)', () => {
  const findings = findingsFromArchitectEvidence(FENCED)
  assert.equal(findings.length, 2)
  assert.deepEqual(findings.map((f) => f.id), ['unit-a', 'unit-b'])
  assert.equal(findings[0].required, true)
})

test('an inline payload still parses (no regression for the old shape)', () => {
  const inline = 'FORGE_FINDINGS_JSON: [{"id":"x","summary":"s","required":true,"seams":["a.ts"]}]'
  assert.equal(findingsFromArchitectEvidence(inline).length, 1)
})

test('a payload whose strings contain brackets stays intact', () => {
  const tricky =
    'FORGE_FINDINGS_JSON: [{"id":"y","summary":"uses [x] and \\"quotes\\"","required":false,"seams":["a.ts"]}]'
  const findings = findingsFromArchitectEvidence(tricky)
  assert.equal(findings.length, 1)
  assert.match(findings[0].summary, /\[x\]/)
})

test('an echoed instruction line does not shadow the real payload', () => {
  const echoed = [
    'The role\'s required structured output is: FORGE_FINDINGS_JSON: <json array>',
    '',
    'FORGE_FINDINGS_JSON: [{"id":"real","summary":"s","required":true,"seams":["a.ts"]}]',
  ].join('\n')
  const findings = findingsFromArchitectEvidence(echoed)
  assert.equal(findings.length, 1)
  assert.equal(findings[0].id, 'real')
})

test('no marker yields no findings', () => {
  assert.deepEqual(findingsFromArchitectEvidence('just prose'), [])
  assert.deepEqual(findingsFromArchitectEvidence(null), [])
})
