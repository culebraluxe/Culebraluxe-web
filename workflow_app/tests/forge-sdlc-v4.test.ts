import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  FORGE_SDLC_VERSION,
  FORGE_SDLC_V4_VERSION,
  parseForgeSdlc,
  parseForgeSdlcV4,
} from '../definitions/forge-sdlc'

// Scope C baseline anchor: v4 parses/validates BEFORE it is ever activated.

test('Scope C: active definition remains v3 (v4 inactive until coordinated activation)', () => {
  assert.equal(FORGE_SDLC_VERSION, 3)
  assert.equal(FORGE_SDLC_V4_VERSION, 4)
})

test('Scope C: FORGE_SDLC-v4.xml is a valid, parseable workflow definition', () => {
  assert.doesNotThrow(() => parseForgeSdlcV4())
  const v3 = parseForgeSdlc()
  const v4 = parseForgeSdlcV4()
  // Baseline (v4 currently mirrors v3): identical node structure.
  assert.equal(v4.graph.nodes.length, v3.graph.nodes.length)
})
