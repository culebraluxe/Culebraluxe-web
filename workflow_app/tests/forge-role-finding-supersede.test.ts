import assert from 'node:assert/strict'
import { test } from 'node:test'

import { decideFindingWrite, listStoryForgeFindingHandoff } from '../../db/forge-role-finding'
import type { QueryExecutor, QueryRow } from '../../db/query-executor'
import { buildLeadRoutingContext } from '../forge/lead-routing-context'
import { buildLeadRoutingDirective } from '../forge/forge-lead-routing-prompt'

// ---------------------------------------------------------------------------
// A LATER ATTEMPT MAY NOT SILENTLY DROP A SEAM THE EARLIER ONE DECLARED.
//
// Migration 172 scoped the Lead's findings read to the newest attempt per node, which is what
// stops a story-wide read feeding the Lead duplicate finding ids. That scope also hid a loss:
// attempt N replaced the rows and the Lead never learned a seam was gone. The rule under test
// makes the drop either REFUSED by name or SUPERSEDED by a typed row — never silent — and the
// Lead context states which attempt is in force and what it lost.
//
// No database: `decideFindingWrite` is pure, and the reader takes an injected QueryExecutor.
// ---------------------------------------------------------------------------

const STORY_RUN = { storyId: 'ENG-FORGE-FINDING-SUPERSEDE-01', processInstanceId: 'proc-1' }

/** Attempt 1 declared two seams, one per finding. */
const ATTEMPT_ONE = {
  attempt: 1,
  findings: [
    { findingId: 'F1', seams: ['db/forge-role-finding.ts'] },
    { findingId: 'F2', seams: ['scripts/forge-handoff.mjs'] },
  ],
}

// --- the write decision (db/forge-role-finding.ts) -------------------------

test('a later attempt that drops a seam is refused, naming the seam and the attempt that declared it', () => {
  const decision = decideFindingWrite({
    attempt: 2,
    prior: ATTEMPT_ONE,
    current: [],
    incoming: { findingId: 'F1', seams: ['db/forge-role-finding.ts'] },
    acknowledged: [],
  })
  if (decision.kind !== 'refuse') assert.fail('a dropped seam with no acknowledgement must refuse')
  assert.deepEqual(decision.dropped, [
    { seam: 'scripts/forge-handoff.mjs', declaredByAttempt: 1, declaredByFindingId: 'F2' },
  ])
})

test('an exact --supersede acknowledgement allows the write and records the dropped seam', () => {
  const decision = decideFindingWrite({
    attempt: 2,
    prior: ATTEMPT_ONE,
    current: [],
    incoming: { findingId: 'F1', seams: ['db/forge-role-finding.ts'] },
    acknowledged: ['scripts/forge-handoff.mjs'],
  })
  if (decision.kind !== 'allow') assert.fail('an exact acknowledgement must allow the write')
  assert.deepEqual(decision.superseded, [
    { seam: 'scripts/forge-handoff.mjs', declaredByAttempt: 1, declaredByFindingId: 'F2' },
  ])
})

test('an acknowledgement that is a subset or a superset of the drops is refused', () => {
  const base = {
    attempt: 2,
    prior: ATTEMPT_ONE,
    current: [],
    incoming: { findingId: 'F1', seams: ['db/forge-role-finding.ts'] },
  }
  assert.equal(
    decideFindingWrite({ ...base, acknowledged: ['a/other.ts'] }).kind,
    'refuse',
    'a subset launders the seam it does not name',
  )
  assert.equal(
    decideFindingWrite({
      ...base,
      acknowledged: ['scripts/forge-handoff.mjs', 'a/other.ts'],
    }).kind,
    'refuse',
    'a superset acknowledges a drop that is not happening',
  )
})

test('a re-issue that preserves every seam is allowed, even when it re-keys the finding', () => {
  const decision = decideFindingWrite({
    attempt: 2,
    prior: ATTEMPT_ONE,
    current: [],
    incoming: {
      findingId: 'RENAMED',
      seams: ['db/forge-role-finding.ts', 'scripts/forge-handoff.mjs'],
    },
    acknowledged: [],
  })
  if (decision.kind !== 'allow') assert.fail('a re-keyed finding that keeps its seams is allowed')
  assert.deepEqual(decision.superseded, [])
})

test('a seam already written earlier in the same attempt is not a drop', () => {
  const decision = decideFindingWrite({
    attempt: 2,
    prior: ATTEMPT_ONE,
    current: [{ findingId: 'F2', seams: ['scripts/forge-handoff.mjs'] }],
    incoming: { findingId: 'F1', seams: ['db/forge-role-finding.ts'] },
    acknowledged: [],
  })
  assert.equal(decision.kind, 'allow')
})

test('a first attempt has no prior attempt and drops nothing', () => {
  const decision = decideFindingWrite({
    attempt: 1,
    prior: null,
    current: [],
    incoming: { findingId: 'F1', seams: ['db/forge-role-finding.ts'] },
    acknowledged: [],
  })
  assert.equal(decision.kind, 'allow')
})

// --- the handoff reader (db/forge-role-finding.ts) -------------------------

/**
 * Answer by which table the SQL names. The supersede table name CONTAINS the finding table name,
 * so it is checked first — otherwise the fake would answer both queries from one table.
 */
function handoffExecutor(rows: { findings?: QueryRow[]; superseded?: QueryRow[] }): QueryExecutor {
  return (async (strings: TemplateStringsArray) => {
    const sql = strings.join(' ')
    if (sql.includes('forge_role_finding_supersede')) return rows.superseded ?? []
    if (sql.includes('forge_role_finding')) return rows.findings ?? []
    return []
  }) as unknown as QueryExecutor
}

const findingRow = (over: QueryRow = {}): QueryRow => ({
  finding_id: 'F1',
  summary: 'one bounded change',
  required: true,
  seams: ['db/forge-role-finding.ts'],
  hint: null,
  attempt: 2,
  ...over,
})

test('no finding rows is null, so the caller falls back to the reply parser', async () => {
  assert.equal(await listStoryForgeFindingHandoff(STORY_RUN, handoffExecutor({})), null)
})

test('the newest-attempt read reports the attempt in force and never duplicates a finding id', async () => {
  const handoff = await listStoryForgeFindingHandoff(
    STORY_RUN,
    handoffExecutor({
      findings: [
        findingRow({ finding_id: 'F1', attempt: 2 }),
        findingRow({ finding_id: 'F2', attempt: 2, seams: ['scripts/forge-handoff.mjs'] }),
      ],
    }),
  )
  assert.ok(handoff)
  const ids = handoff.findings.map((f) => f.id)
  assert.equal(new Set(ids).size, ids.length, 'a retried attempt must not read as duplicates')
  assert.equal(handoff.attemptInForce, 2)
})

test('a superseded seam is read back with the attempt that declared it', async () => {
  const handoff = await listStoryForgeFindingHandoff(
    STORY_RUN,
    handoffExecutor({
      findings: [findingRow()],
      superseded: [
        { seam: 'scripts/forge-handoff.mjs', declared_by_attempt: 1, declared_by_finding_id: 'F2' },
      ],
    }),
  )
  assert.ok(handoff)
  assert.deepEqual(handoff.superseded, [
    { seam: 'scripts/forge-handoff.mjs', declaredByAttempt: 1, declaredByFindingId: 'F2' },
  ])
})

// The fake executor answers by table name, so it cannot catch a scope mistake: the query would
// still succeed, just too broadly. This reads the SQL itself, because the SCOPE is the part that
// broke the chain live — a story-wide read fed the Lead duplicate finding ids.
test('the read is scoped to the current process and the newest attempt per node', async () => {
  let seen = ''
  const capture = (async (strings: TemplateStringsArray) => {
    seen = seen || strings.join(' ')
    return []
  }) as unknown as QueryExecutor

  await listStoryForgeFindingHandoff(STORY_RUN, capture)

  assert.match(seen, /process_instance_id/, 'every earlier run of the story must be excluded')
  assert.match(seen, /group by node_id/, 'the newest attempt is chosen per node')
  assert.match(seen, /max\(attempt\)/, 'a retried node writes the same ids again on purpose')
})

// --- the Lead routing context ----------------------------------------------

const STORY = {
  id: 'ENG-FORGE-FINDING-SUPERSEDE-01',
  assayCommands: 'node --import tsx --test workflow_app/tests/forge-role-finding-supersede.test.ts',
}
const CAPS = { splitEnabled: true, maxSmiths: 2 }
const FINDINGS = [{ id: 'F1', required: true, seams: ['db/forge-role-finding.ts'] }]

test('the Lead directive states the attempt in force and names the dropped seam and its declaring attempt', () => {
  const context = buildLeadRoutingContext({
    story: STORY,
    findings: FINDINGS,
    capabilities: CAPS,
    findingHandoff: {
      attemptInForce: 2,
      superseded: [
        { seam: 'scripts/forge-handoff.mjs', declaredByAttempt: 1, declaredByFindingId: 'F2' },
      ],
    },
  })
  assert.equal(context.findingHandoff?.attemptInForce, 2)
  const directive = buildLeadRoutingDirective(context)
  assert.match(directive, /attempt 2 is in force/)
  assert.match(directive, /DROPPED SEAM: scripts\/forge-handoff\.mjs/)
  assert.match(directive, /declared by attempt 1/)
})

test('no supersede renders nothing, and the legacy fallback states no attempt', () => {
  const quiet = buildLeadRoutingDirective(
    buildLeadRoutingContext({
      story: STORY,
      findings: FINDINGS,
      capabilities: CAPS,
      findingHandoff: { attemptInForce: 2, superseded: [] },
    }),
  )
  assert.doesNotMatch(quiet, /DROPPED SEAM/)
  assert.match(quiet, /attempt 2 is in force/)

  const legacy = buildLeadRoutingDirective(
    buildLeadRoutingContext({
      story: STORY,
      findings: FINDINGS,
      capabilities: CAPS,
      findingHandoff: null,
    }),
  )
  assert.doesNotMatch(legacy, /attempt \d+ is in force/)
  assert.match(legacy, /no attempt is recorded/)
})
