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
  const nodes = (g: { graph: { nodes: Record<string, unknown> } }) => Object.keys(g.graph.nodes)
  const v3Nodes = nodes(v3)
  const v4Nodes = nodes(v4)
  // v4 = v3 superset: FAST lane nodes added, nothing removed.
  assert.ok(v4Nodes.length > v3Nodes.length)
  for (const id of ['fast_lane_entry', 'fast_smith', 'fast_qa_verify', 'fast_qa_result', 'fast_repair_smith', 'fast_publish', 'fast_publish_result']) {
    assert.ok(v4Nodes.includes(id), `missing ${id}`)
  }
  for (const id of v3Nodes) assert.ok(v4Nodes.includes(id), `v4 dropped v3 node ${id}`)
  assert.equal(v4.version, 4)
})
