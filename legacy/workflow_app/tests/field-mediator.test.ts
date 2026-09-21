import assert from 'node:assert/strict'
import test from 'node:test'

import { describeRefusal, mediateField, normalizeRaw } from '@/lib/field-mediator'

const SIZE = {
  field: 'size',
  kind: 'closed' as const,
  accepted: ['SOLO', 'SMITH', 'SPLIT', 'HOLD'],
  aliases: { single: 'SOLO', soloish: 'SOLO' },
  decision: true,
}

test('shape tolerance is mechanical, and never changes meaning', () => {
  assert.equal(normalizeRaw('```json\n  SOLO  \n```'), 'SOLO')
  // fenced, quoted, keyed and bare all land on the same value
  for (const raw of ['```\nRFC: SOLO\n```', 'RFC=solo', '"solo"', 'solo']) {
    const out = mediateField({ ...SIZE, field: 'rfc' }, raw)
    assert.equal(out.ok, true, `expected ${raw} to mediate`)
    if (out.ok) assert.equal(out.value, 'SOLO')
  }
  // a DECLARED alias is a synonym; case folds
  const aliased = mediateField({ ...SIZE, aliases: { single: 'SOLO' } }, 'Single')
  assert.equal(aliased.ok, true)
  if (aliased.ok) assert.equal(aliased.value, 'SOLO')
})

test('a decision is never defaulted, and never invented', () => {
  const missing = mediateField({ ...SIZE, default: 'SMITH' }, '   ')
  assert.equal(missing.ok, false)
  if (!missing.ok) assert.equal(missing.reason, 'DECISION_MISSING')
  // the declared default is honoured for a non-decision, and the success says where it came from
  const defaulted = mediateField({ field: 'lane', kind: 'text', default: 'night' }, '')
  assert.equal(defaulted.ok, true)
  if (defaulted.ok) assert.deepEqual([defaulted.value, defaulted.source], ['night', 'default'])
})

test('an undeclared value is refused naming the field and the accepted set', () => {
  const out = mediateField(SIZE, 'TEAM')
  assert.equal(out.ok, false)
  if (!out.ok) {
    assert.equal(out.field, 'size')
    assert.equal(out.reason, 'NOT_IN_SET')
    assert.ok(describeRefusal(out).includes('SOLO | SMITH | SPLIT | HOLD'), describeRefusal(out))
  }
})

test('two candidates in prose is AMBIGUOUS — the mediator never picks', () => {
  // A DECISION field is not mined from prose at all (that is the safety property), so the ambiguity case is
  // exercised on a descriptive closed field — the same code path, without pretending prose can decide.
  const out = mediateField({ ...SIZE, allowProse: true, decision: false }, 'the lead could go SOLO or SMITH')
  assert.equal(out.ok, false)
  if (!out.ok) assert.equal(out.reason, 'AMBIGUOUS')
  // one candidate, descriptive field, prose allowed: that is mechanical and lands
  const one = mediateField({ ...SIZE, allowProse: true, decision: false }, 'the lead chose SMITH today')
  assert.equal(one.ok, true)
  if (one.ok) assert.equal(one.value, 'SMITH')
})

test('typed kinds coerce declared spellings and refuse the rest', () => {
  assert.equal(mediateField({ field: 'n', kind: 'number' }, '7').ok, true)
  assert.equal(mediateField({ field: 'n', kind: 'number' }, 'about seven').ok, false)
  assert.equal(mediateField({ field: 'b', kind: 'boolean' }, 'Yes').ok, true)
  assert.equal(mediateField({ field: 'b', kind: 'boolean' }, 'maybe').ok, false)
  assert.equal(mediateField({ field: 'sha', kind: 'sha' }, '5addff16b722').ok, true)
  assert.equal(mediateField({ field: 'sha', kind: 'sha' }, 'HEAD~1').ok, false)
  const long = mediateField({ field: 't', kind: 'text', maxLength: 3 }, 'abcdef')
  assert.equal(long.ok, false)
  if (!long.ok) assert.equal(long.reason, 'TOO_LONG')
})

test('a non-string is empty, never stringified into a value', () => {
  for (const raw of [undefined, null, 42, {}, []]) {
    const out = mediateField({ field: 'x', kind: 'text' }, raw)
    assert.equal(out.ok, false, `${JSON.stringify(raw)} must not become a value`)
  }
})
