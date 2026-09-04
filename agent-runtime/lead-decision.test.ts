import assert from 'node:assert/strict'
import test from 'node:test'

import {
  leadRunPhaseFromInstructions,
  parseLeadDecision,
  withLeadRunPhaseDirective,
} from './lead-decision'

test('Lead phase directive is durable command metadata', () => {
  const text = withLeadRunPhaseDirective('work', 'post')
  assert.equal(leadRunPhaseFromInstructions(text), 'post')
})

test('Lead PRE decisions parse as structured machine facts', () => {
  assert.deepEqual(
    parseLeadDecision('notes\nLEAD_DECISION: SOLO\nLEAD_REASON: cheaper than delegation'),
    { decision: 'SOLO', splitCount: null, reason: 'cheaper than delegation' },
  )
  assert.deepEqual(
    parseLeadDecision('LEAD_DECISION: SPLIT:3\nLEAD_REASON: stable interfaces'),
    { decision: 'SPLIT', splitCount: 3, reason: 'stable interfaces' },
  )
})

test('Lead POST decisions are explicit and malformed split fails closed', () => {
  assert.deepEqual(parseLeadDecision('LEAD_DECISION: ASSAY'), {
    decision: 'ASSAY',
    splitCount: null,
    reason: null,
  })
  assert.equal(parseLeadDecision('LEAD_DECISION: SPLIT:1'), null)
  assert.equal(parseLeadDecision('looks good'), null)
})
