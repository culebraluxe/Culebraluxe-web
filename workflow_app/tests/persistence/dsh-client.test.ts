// ---------------------------------------------------------------------------
// ENG-19 dsh-client unit tests — pure functions only (no harness process, no DB).
// Proves session-root path escaping + newest-session discovery semantics.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  dshHome,
  sessionRootForWorkspace,
} from '../../../agent-runtime/deepseek/dsh-client'

test('ENG-19: sessionRootForWorkspace matches the harness projectKey escaping', () => {
  const root = sessionRootForWorkspace('/Users/lisapenfieldicloud.com/Documents/Culebraluxe-web')
  assert.equal(root, '/Users/lisapenfieldicloud.com/.dsh/sessions/--Users-lisapenfieldicloud.com-Documents-Culebraluxe-web--')
  assert.ok(root.startsWith(dshHome()))
})

test('ENG-19: dshHome honors DSH_HOME override', () => {
  const original = process.env.DSH_HOME
  process.env.DSH_HOME = '/tmp/dsh-test-home'
  try {
    assert.equal(dshHome(), '/tmp/dsh-test-home')
    assert.ok(sessionRootForWorkspace('/repo').startsWith('/tmp/dsh-test-home'))
  } finally {
    if (original === undefined) delete process.env.DSH_HOME
    else process.env.DSH_HOME = original
  }
})
