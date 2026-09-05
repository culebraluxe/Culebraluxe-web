import assert from 'node:assert/strict'
import test from 'node:test'

import { FORGE_HOLD_RESUME_TARGETS, validResumeTarget } from '../forge/forge-hold-resolve'

// ---------------------------------------------------------------------------
// ENG-FORGE-V10 S4 — HOLD resumeTarget validation (pure).
// ---------------------------------------------------------------------------

test('ENG-FORGE-V10 S4: accepts every XML resumeTarget enum value', () => {
  for (const target of FORGE_HOLD_RESUME_TARGETS) {
    assert.equal(validResumeTarget(target), true, `expected valid: ${target}`)
  }
})

test('ENG-FORGE-V10 S4: rejects a prose/invalid resumeTarget', () => {
  assert.equal(validResumeTarget('back to smith please'), false)
  assert.equal(validResumeTarget(''), false)
  assert.equal(validResumeTarget('SMITH'), true)
})
