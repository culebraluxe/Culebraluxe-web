import assert from 'node:assert/strict'
import test from 'node:test'

import { collectAssayEvidence } from '../forge/agents/assay-collect'
import type { RoleEffectPorts } from '../forge/agents/ports'
import type { CommandResult } from '../forge/agents/qa/types'
import { acceptanceClauses, buildAcceptanceMap } from '../forge/agents/qa/types'
import {
  normalizeAcceptanceAssertions,
  resolveAcceptanceAssertions,
} from '../forge/forge-architect-contract'

// ---------------------------------------------------------------------------
// ENG-FORGE-ACCEPTANCE-SUPPLIER-01 — THE ACCEPTANCE MAPPING HAS A PRODUCER.
//
// Measured before this story: the reader (`collectAssayEvidence`) and the builder
// (`buildStoryAcceptanceMap`) existed, but NOTHING supplied the mapping on the QA port, so every
// story came back UNPROVEN with `acceptance-map-missing` no matter how well its clauses were
// covered. These lock the producer and the ONE READER that resolves the two declaration places:
// handoff-declared WINS, story-declared is the fallback, and the source is named.
// ---------------------------------------------------------------------------

const COMMAND = 'node --import tsx --test workflow_app/tests/acceptance-supplier.test.ts'

const ok = (command: string): CommandResult => ({
  command,
  exitCode: 0,
  passed: true,
  excerpt: 'ok',
})

const collectorPorts = (over: Partial<RoleEffectPorts>): RoleEffectPorts =>
  ({ assayCommands: [COMMAND], runCommand: ok, ...over }) as unknown as RoleEffectPorts

/** Exactly what the runner does: resolve the declaration, then build the map QA is handed. */
const suppliedMap = (input: {
  handoff?: unknown
  story?: unknown
  acceptance: string[]
}) => {
  const resolved = resolveAcceptanceAssertions({ handoff: input.handoff, story: input.story })
  if (!resolved.assertions) return { resolved, map: undefined }
  return {
    resolved,
    map: buildAcceptanceMap({ acceptance: input.acceptance, assertions: resolved.assertions }),
  }
}

// ---------------------------------------------------------------------------
// 1. FULLY MAPPED -> PASS. A covered story is no longer UNPROVEN.
// ---------------------------------------------------------------------------
test('a fully-mapped acceptance with a passing proof returns PASS through the producer port', () => {
  const clauses = ['the supplier prefers the handoff', 'an absent mapping stays absent']
  const { resolved, map } = suppliedMap({
    handoff: {
      'the supplier prefers the handoff': ['asserts-handoff'],
      'an absent mapping stays absent': ['asserts-absent'],
    },
    acceptance: clauses,
  })
  assert.equal(resolved.source, 'handoff')
  assert.ok(map, 'a declared mapping produces a map for the port')

  const evidence = collectAssayEvidence({} as never, collectorPorts({ acceptanceMap: map }))
  assert.equal(evidence.qaPassed, true, 'every clause has an assertion behind it, so QA passes')
})

// ---------------------------------------------------------------------------
// 2. PARTIAL -> UNPROVEN, NAMING THE UNCOVERED CLAUSE.
// ---------------------------------------------------------------------------
test('a partially mapped acceptance is UNPROVEN and names the uncovered clause', () => {
  const clauses = ['a covered clause', 'an uncovered clause']
  const { map } = suppliedMap({
    story: { 'a covered clause': ['asserts-covered'] },
    acceptance: clauses,
  })
  assert.ok(map)

  const evidence = collectAssayEvidence({} as never, collectorPorts({ acceptanceMap: map }))
  assert.equal(evidence.qaPassed, false, 'a clause with no assertion is never a pass')
  assert.match(evidence.deliverableRejection ?? '', /unproven=\[/)
  assert.match(
    evidence.deliverableRejection ?? '',
    /an-uncovered-clause/,
    'the uncovered clause is named on the row',
  )
})

// ---------------------------------------------------------------------------
// 3. ABSENT -> UNPROVEN with `acceptance-map-missing`, UNCHANGED.
// ---------------------------------------------------------------------------
test('no declaration at all is UNPROVEN with acceptance-map-missing, unchanged', () => {
  const { resolved, map } = suppliedMap({ acceptance: ['a clause'] })
  assert.equal(resolved.source, null)
  assert.equal(resolved.assertions, null)
  assert.equal(map, undefined, 'an absent mapping stays absent — undefined, never an empty map')

  const evidence = collectAssayEvidence({} as never, collectorPorts({}))
  assert.equal(evidence.qaPassed, false)
  assert.match(evidence.deliverableRejection ?? '', /acceptance-map-missing/)
})

// ---------------------------------------------------------------------------
// 4. HANDOFF-DECLARED BEATS STORY-DECLARED, AND THE SOURCE IS NAMED.
// ---------------------------------------------------------------------------
test('the handoff declaration wins over the story declaration, and the source is named', () => {
  const resolved = resolveAcceptanceAssertions({
    handoff: { 'a clause': ['asserts-handoff'] },
    story: { 'a clause': ['asserts-story'], 'another clause': ['asserts-other'] },
  })
  assert.equal(resolved.source, 'handoff', 'the record says which source was used')
  assert.deepEqual(resolved.assertions, { 'a clause': ['asserts-handoff'] })
})

test('an EMPTY handoff declaration does not shadow a real story declaration', () => {
  const resolved = resolveAcceptanceAssertions({
    handoff: {},
    story: { 'a clause': ['asserts-story'] },
  })
  assert.equal(resolved.source, 'story')
  assert.deepEqual(resolved.assertions, { 'a clause': ['asserts-story'] })
})

test('neither source present resolves to null with no source named', () => {
  const resolved = resolveAcceptanceAssertions({ handoff: null, story: undefined })
  assert.deepEqual(resolved, { assertions: null, source: null })
})

// ---------------------------------------------------------------------------
// 5. A DECLARATION THAT NAMES A CLAUSE BUT NO ASSERTION IS PRESERVED, so the ready gate can refuse
//    it as unmapped instead of reading it as "nothing was declared".
// ---------------------------------------------------------------------------
test('a declared-but-unmapped clause is preserved as a declaration, not dropped to absent', () => {
  const resolved = resolveAcceptanceAssertions({ story: { 'a clause': [] } })
  assert.equal(resolved.source, 'story')
  assert.deepEqual(resolved.assertions, { 'a clause': [] })
})

test('a JSON string from the driver is normalized at the repository boundary', () => {
  assert.deepEqual(normalizeAcceptanceAssertions('{"a clause":["asserts-a"]}'), {
    'a clause': ['asserts-a'],
  })
  assert.equal(normalizeAcceptanceAssertions('{}'), null, 'an empty mapping is absent, not present')
  assert.equal(normalizeAcceptanceAssertions('not json'), null)
})

test('the shared clause splitter feeds the map the producer supplies', () => {
  assert.deepEqual(acceptanceClauses('- one clause\n- another clause'), [
    'one clause',
    'another clause',
  ])
})
