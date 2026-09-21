import assert from 'node:assert/strict'
import test from 'node:test'
import { FORGE_HOLD_UNKNOWN, forgeHoldLine } from '@/legacy/workflow_app/forge/forge-visibility'

test('an open hold names the reason and the originating node', () => {
  const line = forgeHoldLine({
    reason: 'gate refused: missing qa verdict',
    originatingNode: 'qa_verify',
    failureClass: 'evidence',
    resumeTarget: 'qa_verify',
    since: '2026-09-16T19:00:00.000Z',
    processInstanceId: 'pi-1',
  })
  assert.ok(line)
  assert.equal(line.reason, 'gate refused: missing qa verdict')
  assert.equal(line.originatingNode, 'qa_verify')
  assert.equal(line.failureClass, 'evidence')
  assert.equal(line.resumeTarget, 'qa_verify')
  assert.equal(line.since, '2026-09-16T19:00:00.000Z')
  assert.equal(line.processInstanceId, 'pi-1')
})

test('no hold record reports none', () => {
  assert.equal(forgeHoldLine(null), null)
  assert.equal(forgeHoldLine(undefined), null)
})

test('a hold record with no reason reads as unknown, never an empty string', () => {
  const nullReason = forgeHoldLine({
    reason: null,
    originatingNode: 'architect',
    failureClass: null,
    resumeTarget: null,
    since: null,
    processInstanceId: null,
  })
  assert.ok(nullReason)
  assert.equal(nullReason.reason, FORGE_HOLD_UNKNOWN)
  assert.notEqual(nullReason.reason, '')
  assert.equal(nullReason.originatingNode, 'architect')

  const blankReason = forgeHoldLine({
    reason: '   ',
    originatingNode: null,
    failureClass: null,
    resumeTarget: null,
    since: null,
    processInstanceId: null,
  })
  assert.ok(blankReason)
  assert.equal(blankReason.reason, FORGE_HOLD_UNKNOWN)
  assert.notEqual(blankReason.reason, '')
})
