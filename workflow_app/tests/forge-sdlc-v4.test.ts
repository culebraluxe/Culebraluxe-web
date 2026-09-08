import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  FORGE_SDLC_VERSION,
  parseForgeSdlc,
  parseForgeSdlcV4,
} from '../definitions/forge-sdlc'

// Scope C activation anchor: after flipping, the ACTIVE definition is v4 and
// carries the FAST lane + Architect review park while preserving v3 behavior.

test('Scope C: active definition is now v4 and parses/validates', () => {
  assert.equal(FORGE_SDLC_VERSION, 4)
  assert.doesNotThrow(() => parseForgeSdlc())
  const nodes = Object.keys(parseForgeSdlc().graph.nodes)
  // FAST lane present.
  for (const id of ['fast_lane_entry', 'fast_smith', 'fast_qa_verify', 'fast_publish']) {
    assert.ok(nodes.includes(id), `missing ${id}`)
  }
  // Architect review park present (reuses hold; no new end state).
  assert.ok(nodes.includes('architect_review'))
  // v4 loader agrees with the active loader.
  assert.equal(Object.keys(parseForgeSdlcV4().graph.nodes).length, nodes.length)
})