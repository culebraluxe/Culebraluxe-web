import assert from 'node:assert/strict'
import { test } from 'node:test'

import { listStoryForgeFindingHandoff } from '../../db/forge-role-finding'
import type { QueryExecutor, QueryRow } from '../../db/query-executor'
import { reviewLeadProposal, type LeadProposal, type RoutingContext } from '../forge/forge-lead-routing'
import { buildLeadRoutingContext } from '../forge/lead-routing-context'
import { buildLeadRoutingDirective } from '../forge/forge-lead-routing-prompt'

// ---------------------------------------------------------------------------
// A RE-RUN RETIRES WHAT IT SUPERSEDED.
//
// Migration 172 scoped the Lead's findings read to the newest ATTEMPT per node, which stops a
// retried Architect feeding the Lead duplicate ids. It does not stop an OPERATOR RE-RUN, which
// creates a SECOND TASK for the same node at attempt 1 — the attempt rule cannot separate the two,
// so both tasks' rows joined and the Lead context carried nine rows for five ids. reviewLeadProposal
// refuses duplicate finding ids by name, so no non-HOLD route could validate and the story stranded
// until an operator deleted PROD rows by task id (observed live 2026-09-17).
//
// The reader now scopes to the newest TASK per node. This proof drives the two-task case: the
// overlapping ids arrive once, a non-HOLD route validates, and the retired task is named.
//
// No database: the reader takes an injected QueryExecutor.
// ---------------------------------------------------------------------------

const PROOF = 'node --import tsx --test workflow_app/tests/finding-dedupe.test.ts'
const STORY_RUN = { storyId: 'ENG-FORGE-FINDING-DEDUPE-01', processInstanceId: 'proc-1' }

const findingRow = (over: QueryRow = {}): QueryRow => ({
  finding_id: 'F1',
  summary: 'one bounded change',
  required: true,
  seams: ['db/forge-role-finding.ts'],
  hint: null,
  attempt: 1,
  node_id: 'architect',
  task_id: 'task-old',
  created_at: '2026-09-17T20:48:00Z',
  ...over,
})

/** Two Architect tasks for ONE node, both attempt 1, with F1..F3 overlapping. */
function twoArchitectTasks(): QueryRow[] {
  const old = '2026-09-17T20:48:00Z'
  const fresh = '2026-09-17T21:00:00Z'
  return [
    findingRow({ task_id: 'task-old', finding_id: 'F1', created_at: old }),
    findingRow({ task_id: 'task-old', finding_id: 'F2', created_at: old }),
    findingRow({ task_id: 'task-old', finding_id: 'F3', created_at: old }),
    findingRow({ task_id: 'task-new', finding_id: 'F1', created_at: fresh }),
    findingRow({ task_id: 'task-new', finding_id: 'F2', created_at: fresh }),
    findingRow({ task_id: 'task-new', finding_id: 'F3', created_at: fresh }),
    findingRow({ task_id: 'task-new', finding_id: 'F4', created_at: fresh }),
  ]
}

/** The newest task per node: max created_at, `task_id` as the deterministic tie-break. */
function newestTaskPerNode(rows: QueryRow[]): Map<string, string> {
  const best = new Map<string, { taskId: string; last: string }>()
  for (const row of rows) {
    const node = String(row.node_id ?? '')
    const task = String(row.task_id ?? '')
    const last = String(row.created_at ?? '')
    const current = best.get(node)
    if (!current || last > current.last || (last === current.last && task > current.taskId)) {
      best.set(node, { taskId: task, last })
    }
  }
  return new Map([...best].map(([node, value]) => [node, value.taskId]))
}

/**
 * TASK-AWARE fake. The SCOPE is the fix, so a table-name-only fake cannot exercise it: this answers
 * the newest-task read with EVERY task's rows UNLESS the SQL scopes to the newest task — which is
 * exactly what the fix adds. On the pre-fix SQL the reader sees both tasks' overlapping ids and the
 * Lead gate refuses; on the fixed SQL it sees one set.
 */
function taskAwareExecutor(rows: QueryRow[]): QueryExecutor {
  return (async (strings: TemplateStringsArray) => {
    const sql = strings.join(' ')
    if (sql.includes('superseded_by_task_id')) {
      const newest = newestTaskPerNode(rows)
      const retired: QueryRow[] = []
      const seen = new Set<string>()
      for (const row of rows) {
        const node = String(row.node_id ?? '')
        const task = String(row.task_id ?? '')
        const winner = newest.get(node)
        if (!winner || winner === task || seen.has(`${node}|${task}`)) continue
        seen.add(`${node}|${task}`)
        retired.push({ node_id: node, task_id: task, superseded_by_task_id: winner })
      }
      return retired
    }
    if (sql.includes('forge_role_finding_supersede')) return []
    if (!sql.includes('forge_role_finding')) return []
    if (!sql.includes('newest_task')) return rows
    const newest = newestTaskPerNode(rows)
    return rows.filter((row) => newest.get(String(row.node_id ?? '')) === String(row.task_id ?? ''))
  }) as unknown as QueryExecutor
}

function contextFor(findings: Array<{ id: string; required: boolean; seams: string[] }>): RoutingContext {
  return {
    findings,
    evidenceRefs: ['architect_brief'],
    splitEnabled: true,
    maxSmiths: 2,
    allowedProofs: [PROOF],
  }
}

function proposal(findingIds: string[]): LeadProposal {
  return {
    version: 1,
    decision: 'SMITH',
    size: 'MEDIUM',
    sizeReason: 'One coherent reader-scope change across a reader, a directive and a proof.',
    reason: 'One Smith: the surfaces are serial and no sibling can proceed without the reader output.',
    assignments: [
      {
        id: 'a',
        findingIds,
        dependsOn: [],
        evidenceRefs: ['architect_brief'],
        reasoning: 'The reader scope, the handoff record and the proof are one serial unit.',
        features: {
          semanticSurface: 2,
          dependencyDepth: 2,
          uncertainty: 2,
          contextBurden: 2,
          proofBurden: 2,
          coupling: 2,
          changeNovelty: 2,
          workerFit: 1,
        },
        plan: {
          size: 'MEDIUM',
          chunks: [
            {
              id: 1,
              outcome: 'the newest task per node is the only findings set in force',
              surface: ['db/forge-role-finding.ts'],
              invariant: 'the duplicate-id refusal is unchanged for a genuinely duplicated handoff',
              proof: PROOF,
            },
          ],
        },
      },
    ],
    mergeChecks: [PROOF],
  }
}

test("a re-run architect task supersedes the earlier task's findings so reviewLeadProposal sees one set of finding ids", async () => {
  const handoff = await listStoryForgeFindingHandoff(STORY_RUN, taskAwareExecutor(twoArchitectTasks()))
  assert.ok(handoff, 'the newest task wrote findings, so the handoff is present')
  const ids = handoff.findings.map((f) => f.id).sort()
  assert.deepEqual(ids, ['F1', 'F2', 'F3', 'F4'], "exactly the newest task's findings are in force")
  assert.equal(new Set(ids).size, ids.length, 'no finding id arrives twice')
  assert.equal(handoff.attemptInForce, 1)
})

test('a route is never refused because of a row a re-run created', async () => {
  const handoff = await listStoryForgeFindingHandoff(STORY_RUN, taskAwareExecutor(twoArchitectTasks()))
  assert.ok(handoff)
  const findings = handoff.findings.map((f) => ({ id: f.id, required: f.required, seams: f.seams }))

  // The pre-fix read is the deadlock: both tasks' rows, so the same id twice, refused by name.
  const duplicated = contextFor([...findings, ...findings.map((f) => ({ ...f }))])
  const refused = reviewLeadProposal(proposal(['F1', 'F2', 'F3', 'F4']), duplicated)
  assert.equal(refused.ok, false)
  if (!refused.ok) assert.match(refused.errors.join('\n'), /Duplicate finding IDs/)

  const review = reviewLeadProposal(proposal(['F1', 'F2', 'F3', 'F4']), contextFor(findings))
  assert.equal(review.ok, true, review.ok ? '' : review.errors.join('\n'))
})

test('the handoff names the retired task and why', async () => {
  const handoff = await listStoryForgeFindingHandoff(STORY_RUN, taskAwareExecutor(twoArchitectTasks()))
  assert.ok(handoff)
  assert.deepEqual(handoff.retiredTasks, [
    { nodeId: 'architect', taskId: 'task-old', supersededByTaskId: 'task-new' },
  ])

  const context = buildLeadRoutingContext({
    story: { id: 'ENG-FORGE-FINDING-DEDUPE-01', assayCommands: PROOF },
    findings: handoff.findings.map((f) => ({ id: f.id, required: f.required, seams: f.seams })),
    capabilities: { splitEnabled: true, maxSmiths: 2 },
    findingHandoff: {
      attemptInForce: handoff.attemptInForce,
      superseded: handoff.superseded,
      retiredTasks: handoff.retiredTasks,
    },
  })
  const directive = buildLeadRoutingDirective(context)
  assert.match(directive, /RETIRED FINDINGS TASK/)
  assert.match(directive, /task task-old/)
  assert.match(directive, /task task-new/)
  assert.match(directive, /superseded/)
})
