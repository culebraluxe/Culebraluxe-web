import assert from 'node:assert/strict'
import { test } from 'node:test'

import { getForgeRoleContract } from '../../db/forge-role-contract'
import { getForgeRolePlan } from '../../db/forge-role-plan'
import type { QueryExecutor, QueryRow } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// THE DECISION READERS — migrations 170 (contract) and 171 (plan).
//
// These two readers are the fields-first channel: what the Lead writes to the
// database, the decider reads back. Grok's review scored test coverage of the new
// path at 61 and named the gap exactly: "there is no test that imports
// getForgeRoleContract, getForgeRolePlan or leadProposalFromFields". The path that
// cost an 18-minute run was the untested one.
//
// No database: QueryExecutor is an injected type, so a fake that answers by which
// table the SQL names is enough. Both readers take it as their last argument.
// ---------------------------------------------------------------------------

/** Answer each query by the table it names, so both readers can share one fake. */
function executorFor(tables: {
  contract?: QueryRow[]
  assignment?: QueryRow[]
  chunk?: QueryRow[]
}): QueryExecutor {
  return (async (strings: TemplateStringsArray) => {
    const sql = strings.join(' ')
    if (sql.includes('forge_role_contract')) return tables.contract ?? []
    if (sql.includes('forge_role_assignment')) return tables.assignment ?? []
    if (sql.includes('forge_role_plan_chunk')) return tables.chunk ?? []
    return []
  }) as unknown as QueryExecutor
}

const KEY = { taskId: 'task-1', nodeId: 'lead_pre', attempt: 1 }

/** The eight dispatchability numbers, all legal (1..5 / 1..100). */
const vector = {
  semantic_surface: 1,
  dependency_depth: 1,
  uncertainty: 1,
  context_burden: 1,
  proof_burden: 1,
  coupling: 1,
  change_novelty: 1,
  worker_fit: 1,
}

const assignmentRow = (over: QueryRow = {}): QueryRow => ({
  assignment_id: 'A1',
  finding_ids: ['F1'],
  evidence_refs: ['architect_brief'],
  reasoning: 'the row says what to do',
  ...vector,
  ...over,
})

const chunkRow = (over: QueryRow = {}): QueryRow => ({
  assignment_id: 'A1',
  chunk_id: 1,
  size: 'SMALL',
  surface: ['workflow_app/forge/forge-lead-routing.ts'],
  proof: 'node --import tsx --test workflow_app/tests/forge-role-readers.test.ts',
  invariant: 'the reader never invents a value',
  postconditions: null,
  depends_on: [],
  ...over,
})

// --- the contract reader (migration 170) -----------------------------------

test('contract: no row is null, which is the honest "nothing was recorded"', async () => {
  assert.equal(await getForgeRoleContract(KEY, executorFor({})), null)
})

test('contract: a written row normalizes into the typed shape', async () => {
  const contract = await getForgeRoleContract(
    KEY,
    executorFor({
      contract: [
        {
          decision: 'SMITH',
          size: 'MEDIUM',
          size_reason: 'two surfaces',
          reason: 'the finding is required',
          assignment_count: 1,
          finding_ids: ['F1'],
          merge_checks: ['node --import tsx --test x.test.ts'],
          surface_scope: ['workflow_app/forge/forge-lead-routing.ts'],
          attempt: 2,
        },
      ],
    }),
  )

  assert.ok(contract)
  assert.equal(contract!.decision, 'SMITH')
  assert.equal(contract!.size, 'MEDIUM')
  assert.equal(contract!.assignmentCount, 1)
  assert.deepEqual(contract!.findingIds, ['F1'])
  assert.deepEqual(contract!.mergeChecks, ['node --import tsx --test x.test.ts'])
  assert.equal(contract!.attempt, 2)
})

test('contract: NULL columns stay null and lists stay lists, never undefined', async () => {
  const contract = await getForgeRoleContract(
    KEY,
    executorFor({
      contract: [
        {
          decision: null,
          size: null,
          size_reason: null,
          reason: null,
          assignment_count: null,
          finding_ids: null,
          merge_checks: null,
          surface_scope: null,
          attempt: null,
        },
      ],
    }),
  )

  assert.ok(contract)
  assert.equal(contract!.decision, null)
  assert.equal(contract!.assignmentCount, null)
  assert.deepEqual(contract!.findingIds, [])
  assert.deepEqual(contract!.mergeChecks, [])
  assert.equal(contract!.attempt, KEY.attempt, 'a missing attempt falls back to the key asked for')
})

// --- the plan reader (migration 171) ----------------------------------------

test('plan: no assignment rows is null, so the caller falls back instead of guessing', async () => {
  assert.equal(await getForgeRolePlan(KEY, executorFor({})), null)
})

test('plan: a complete row reads back with its own size, outcome, invariant and proof', async () => {
  const plan = await getForgeRolePlan(
    KEY,
    executorFor({
      assignment: [assignmentRow()],
      chunk: [chunkRow({ chunk_id: 1, depends_on: ['1'] })],
    }),
  )

  assert.ok(plan)
  assert.equal(plan!.size, 'SMALL')
  assert.equal(plan!.assignments.length, 1)
  const a = plan!.assignments[0]
  assert.equal(a.id, 'A1')
  assert.equal(a.reasoning, 'the row says what to do')
  assert.equal(a.plan.size, 'SMALL')
  assert.equal(a.plan.chunks[0].invariant, 'the reader never invents a value')
  assert.equal(a.plan.chunks[0].outcome, 'the reader never invents a value')
  assert.equal(a.plan.chunks[0].dependsOn[0], 1, 'chunk-level ordering is read')
})

test('plan: a blank reasoning is missing, NOT a fabricated "(not stated)"', async () => {
  // `proposalValidShape` tests `nonempty(a.reasoning)`. Substituting a placeholder
  // satisfied that check with text the Lead never wrote — a validation bypass. An
  // incomplete row must return null so the caller falls back to the reply.
  const plan = await getForgeRolePlan(
    KEY,
    executorFor({
      assignment: [assignmentRow({ reasoning: '   ' })],
      chunk: [chunkRow()],
    }),
  )
  assert.equal(plan, null)
})

test('plan: a chunk stating neither an invariant nor postconditions is incomplete', async () => {
  const plan = await getForgeRolePlan(
    KEY,
    executorFor({
      assignment: [assignmentRow()],
      chunk: [chunkRow({ invariant: '', postconditions: [] })],
    }),
  )
  assert.equal(plan, null)
})

test('plan: postconditions ARE the outcome and invariant when that is all the Lead stated', async () => {
  const plan = await getForgeRolePlan(
    KEY,
    executorFor({
      assignment: [assignmentRow()],
      chunk: [chunkRow({ invariant: '', postconditions: ['the test exits 0', 'one file changed'] })],
    }),
  )

  assert.ok(plan)
  assert.equal(plan!.assignments[0].plan.chunks[0].outcome, 'the test exits 0; one file changed')
  assert.equal(plan!.assignments[0].plan.chunks[0].invariant, 'the test exits 0; one file changed')
})

test('plan: size belongs to each assignment, not to whichever chunk was largest overall', async () => {
  // The old reader kept ONE running maximum and then broadcast it to every sibling,
  // so a single MEDIUM chunk promoted the whole plan.
  const plan = await getForgeRolePlan(
    KEY,
    executorFor({
      assignment: [assignmentRow(), assignmentRow({ assignment_id: 'A2', finding_ids: ['F2'] })],
      chunk: [
        chunkRow({ chunk_id: 1, size: 'MEDIUM' }),
        chunkRow({ assignment_id: 'A2', chunk_id: 1, size: 'SMALL' }),
      ],
    }),
  )

  assert.ok(plan)
  const [a1, a2] = plan!.assignments
  assert.equal(a1.plan.size, 'MEDIUM', 'A1 carries its own chunk size')
  assert.equal(a2.plan.size, 'SMALL', 'A2 is NOT promoted by A1')
  assert.equal(plan!.size, 'MEDIUM', 'the plan envelope is the maximum of the assignments')
})

test('plan: LARGE is read, where the old reader ignored it entirely', async () => {
  const plan = await getForgeRolePlan(
    KEY,
    executorFor({
      assignment: [assignmentRow()],
      chunk: [chunkRow({ size: 'LARGE' })],
    }),
  )

  assert.ok(plan)
  assert.equal(plan!.assignments[0].plan.size, 'LARGE')
  assert.equal(plan!.size, 'LARGE')
})

test('plan: an incomplete dispatchability vector is null, never guessed numbers', async () => {
  const plan = await getForgeRolePlan(
    KEY,
    executorFor({
      assignment: [assignmentRow({ uncertainty: 0 })],
      chunk: [chunkRow()],
    }),
  )
  assert.equal(plan, null)
})

test('plan: assignment-level ordering is honestly empty, chunk ordering is real', async () => {
  const plan = await getForgeRolePlan(
    KEY,
    executorFor({
      assignment: [assignmentRow()],
      chunk: [chunkRow({ chunk_id: 2, depends_on: ['1'] })],
    }),
  )

  assert.ok(plan)
  assert.deepEqual(plan!.assignments[0].dependsOn, [], 'migration 171 records no assignment ordering')
  assert.deepEqual(plan!.assignments[0].plan.chunks[0].dependsOn, [1])
})
