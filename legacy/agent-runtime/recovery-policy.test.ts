import assert from 'node:assert/strict'
import test from 'node:test'

import { decideRuntimeRecovery } from './recovery-policy'

for (const role of ['verifier', 'reviewer']) {
  test(`${role} interruption always requires human even with retry budget remaining`, () => {
    const decision = decideRuntimeRecovery({
      role,
      attempts: 1,
      maxAttempts: 99,
      reason: 'process interrupted',
    })
    assert.equal(decision.action, 'hold-human')
    assert.equal(decision.humanRequired, true)
    assert.match(decision.reason, /no automatic Assay retry/i)
    assert.match(decision.reason, /no Smith restart/i)
  })
}

test('Smith may retry infrastructure failure only while budget remains', () => {
  assert.equal(
    decideRuntimeRecovery({
      role: 'builder',
      attempts: 1,
      maxAttempts: 3,
      reason: 'transient transport failure',
    }).action,
    'retry',
  )
  assert.equal(
    decideRuntimeRecovery({
      role: 'builder',
      attempts: 3,
      maxAttempts: 3,
      reason: 'still failing',
    }).action,
    'hold-human',
  )
})
