import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

import {
  buildGoldenEventSpecs,
  goldenSourceEventIds,
  goldenCauseGraph,
  GOLDEN_OFFSETS,
  QA_SOURCE_SYSTEM,
  type QaContext,
} from '../../lib/qa-golden'

// ---------------------------------------------------------------------------
// QA-GOLDEN — verifies the deterministic 18-event Golden QA narrative (structure
// only; no DB required).
// ---------------------------------------------------------------------------

const CTX: QaContext = {
  dealId: 'deal-x',
  propertyId: 'prop-x',
  propertyName: 'QA — 123 Ocean View Drive',
  mariaId: 'maria-x',
  juanId: 'juan-x',
  mariaName: 'QA Maria Rodriguez',
  juanName: 'QA Juan Rodriguez',
  workflowInstanceId: 'wf-x',
  workflowDefinitionKey: 'residential_transaction',
  workflowDefinitionVersion: 1,
}

test('QA-GOLDEN 1: exactly 18 event specs', () => {
  assert.equal(buildGoldenEventSpecs().length, 18)
})

test('QA-GOLDEN 3/4/18: offsets are non-decreasing, not all zero, span ~2870ms', () => {
  assert.equal(GOLDEN_OFFSETS.length, 18)
  assert.ok(GOLDEN_OFFSETS.some((o) => o > 0), 'not all zero')
  for (let i = 1; i < GOLDEN_OFFSETS.length; i++) {
    assert.ok(GOLDEN_OFFSETS[i] >= GOLDEN_OFFSETS[i - 1], `offset non-decreasing at ${i}`)
  }
  assert.equal(GOLDEN_OFFSETS[17], 2870, 'last event around 2870ms')
  // Distinct offsets produce distinct swimlane positions.
  assert.equal(new Set(GOLDEN_OFFSETS).size, GOLDEN_OFFSETS.length)
})

test('QA-GOLDEN 5/6: all six EventKind families and expected systems are represented', () => {
  const kinds = new Set(buildGoldenEventSpecs().map((s) => s.eventType))
  // Kind families are derived from eventType prefixes (adapter semantics).
  assert.ok([...kinds].some((k) => k.startsWith('COMMAND_')))
  assert.ok([...kinds].some((k) => k.startsWith('DOMAIN_EVENT')))
  assert.ok([...kinds].some((k) => k === 'WORKFLOW_STARTED' || k === 'TRANSITION_TAKEN'))
  assert.ok([...kinds].some((k) => k.startsWith('TASK_')))
  assert.ok([...kinds].some((k) => k.startsWith('SIGNATURE_')))
  assert.ok([...kinds].some((k) => k.startsWith('PERSISTENCE_')))
  const systems = new Set(buildGoldenEventSpecs().map((s) => s.system))
  for (const sys of ['command', 'domain', 'workflow', 'task', 'signature', 'postgres']) {
    assert.ok(systems.has(sys), `system ${sys} present`)
  }
})

test('QA-GOLDEN 7: every simulated external event is marked qa_simulation', () => {
  for (const s of buildGoldenEventSpecs()) {
    assert.equal(s.metadata(CTX).qa_simulation, true, `${s.sourceEventId} marked qa_simulation`)
  }
})

test('QA-GOLDEN 8/9: deterministic unique sourceEventIds', () => {
  const ids = goldenSourceEventIds()
  assert.equal(ids.length, 18)
  assert.equal(new Set(ids).size, 18, 'no duplicate sourceEventIds')
  for (const id of ids) assert.ok(id.startsWith('golden-'), `deterministic id ${id}`)
})

test('QA-GOLDEN 10/11: causal graph is intended and fully resolves within the set', () => {
  const graph = goldenCauseGraph()
  const ids = new Set(goldenSourceEventIds())
  const edges = Object.entries(graph).filter(([, cause]) => cause != null)
  assert.ok(edges.length >= 15, 'rich causal graph')
  for (const [, cause] of edges) assert.ok(ids.has(cause as string), `cause ${cause} resolves`)
})

test('QA-GOLDEN 14/15: real QA identities propagate; old fake IDs are absent', () => {
  const specs = buildGoldenEventSpecs()
  assert.equal(specs[0].metadata(CTX).dealId, 'deal-x')
  const wf = specs.find((s) => s.index === 4)!
  assert.equal(wf.metadata(CTX).workflowInstanceId, 'wf-x')
  assert.equal(wf.metadata(CTX).workflowDefinitionKey, 'residential_transaction')
  const created = specs.find((s) => s.index === 3)!
  assert.deepEqual(created.metadata(CTX).buyers, ['QA Maria Rodriguez', 'QA Juan Rodriguez'])
  const raw = JSON.stringify({ specs, CTX })
  assert.ok(!raw.includes('DEAL-2025-000123'), 'no stale deal id')
  assert.ok(!raw.includes('wf_8f3a9c2e'), 'no stale workflow id')
})

test('QA-GOLDEN SWIMLANE 16: embedded KindGlyph carries explicit dimensions', async () => {
  const glyph = await readFile(new URL('../../components/portal/tech/flight-recorder-console/KindGlyph.tsx', import.meta.url), 'utf8')
  const page = await readFile(new URL('../../components/portal/tech/flight-recorder-console/FlightRecorderPage.tsx', import.meta.url), 'utf8')
  assert.ok(glyph.includes('width?: number'), 'KindGlyph accepts explicit width')
  assert.ok(glyph.includes('height?: number'), 'KindGlyph accepts explicit height')
  assert.ok(page.includes('width={16}') && page.includes('height={16}'), 'Swimlane passes explicit 16x16')
})

test('QA-GOLDEN SWIMLANE 17: Timeline KindGlyph behavior unchanged (no explicit size)', async () => {
  const glyph = await readFile(new URL('../../components/portal/tech/flight-recorder-console/KindGlyph.tsx', import.meta.url), 'utf8')
  assert.ok(glyph.includes("className = 'h-4 w-4'"), 'Timeline default class preserved')
})
