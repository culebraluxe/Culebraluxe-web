#!/usr/bin/env node
// ---------------------------------------------------------------------------
// THE CONTRACT COMMAND — how a Forge role states its decision now.
//
// Replaces "emit a JSON line in your reply". The decision goes into FIELDS, and the
// database enforces it: a bad value fails the WRITE, here, with an error the model
// can read and correct — instead of becoming unparseable text that holds a run 18
// minutes later because a marker prefix went missing.
//
// Usage (identity flags are given to the role in its task line):
//
//   node --import tsx --env-file=.env.local scripts/forge-handoff.mjs \
//     --story ENG-1 --process <uuid> --task <uuid> --node lead_pre --attempt 1 \
//     --decision SOLO --size SMALL \
//     --size-reason "one file, one seam" \
//     --reason "smallest honest change" \
//     --assignments 1 \
//     --findings add-story-moves-engine-gate-test \
//     --merge-checks "node --import tsx --test workflow_app/tests/story-moves.test.ts" \
//     --scope workflow_app/tests/story-moves.test.ts
//
//   ... --show     print the contract already recorded for this task/node/attempt
//
// Only --decision is required. Re-running with more flags fills the same row in
// (one row per task/node/attempt; a retry writes a NEW attempt).
// ---------------------------------------------------------------------------
import { forgeDbPool } from '../db/forge-db.ts'

const args = process.argv.slice(2)
const arg = (name) => {
  const i = args.indexOf(`--${name}`)
  return i >= 0 ? args[i + 1] ?? null : null
}
const list = (name) =>
  (arg(name) ?? '')
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean)

const storyId = arg('story')
const processInstanceId = arg('process')
const taskId = arg('task')
const nodeId = arg('node')
const attempt = Number.parseInt(arg('attempt') ?? '1', 10) || 1
const show = args.includes('--show')

if (!storyId || !processInstanceId || !taskId || !nodeId) {
  console.error(
    'forge-handoff: need --story --process --task --node (identity is in your task line). ' +
      'Add --decision SOLO|SMITH|SPLIT|HOLD and the fields, or --show to read the current contract.',
  )
  process.exit(2)
}

const pool = forgeDbPool()

// --chunk mode: record ONE chunk of the work-order plan. Repeat per chunk.
//
//   ... --chunk 1 --assignment a --surface workflow_app/tests/story-moves.test.ts \
//       --proof "node --import tsx --test workflow_app/tests/story-moves.test.ts" \
//       --invariant "lib/story-moves.ts byte-identical" \
//       --finding add-story-moves-engine-gate-test --evidence architect_brief,a
//
// The assignment row is created on first use and its vector can be set with
// --semantic-surface/--dependency-depth/--uncertainty/--context-burden/--proof-burden/
// --coupling/--change-novelty/--worker-fit. The chunks ARE the plan: surface and proof
// are NOT NULL, so a chunk that cannot be checked is refused by the database.
if (arg('chunk')) {
  const chunkId = Number.parseInt(arg('chunk'), 10)
  const assignmentId = arg('assignment') ?? 'a'
  const surface = list('surface')
  const proof = arg('proof')
  const vector = (name) => {
    const raw = arg(name)
    if (raw == null) return null
    const n = Number.parseInt(raw, 10)
    return Number.isFinite(n) ? n : null
  }

  if (!Number.isFinite(chunkId) || chunkId < 1) {
    console.error('forge-handoff: --chunk must be 1..3')
    await pool.end()
    process.exit(2)
  }
  if (surface.length === 0 || !proof?.trim()) {
    console.error(
      'forge-handoff: a chunk needs --surface <path[,path]> and --proof "<exact command>". ' +
        'A chunk that cannot be checked is not a plan.',
    )
    await pool.end()
    process.exit(2)
  }

  try {
    await pool.query(
      `insert into forge_role_assignment
         (story_id, process_instance_id, task_id, node_id, attempt, assignment_id,
          finding_ids, evidence_refs, reasoning,
          semantic_surface, dependency_depth, uncertainty, context_burden,
          proof_burden, coupling, change_novelty, worker_fit)
       values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)
       on conflict (task_id, node_id, attempt, assignment_id) do update set
         finding_ids = case when cardinality(excluded.finding_ids) > 0
                            then excluded.finding_ids else forge_role_assignment.finding_ids end,
         evidence_refs = case when cardinality(excluded.evidence_refs) > 0
                              then excluded.evidence_refs else forge_role_assignment.evidence_refs end,
         reasoning = coalesce(excluded.reasoning, forge_role_assignment.reasoning),
         semantic_surface = coalesce(excluded.semantic_surface, forge_role_assignment.semantic_surface),
         dependency_depth = coalesce(excluded.dependency_depth, forge_role_assignment.dependency_depth),
         uncertainty = coalesce(excluded.uncertainty, forge_role_assignment.uncertainty),
         context_burden = coalesce(excluded.context_burden, forge_role_assignment.context_burden),
         proof_burden = coalesce(excluded.proof_burden, forge_role_assignment.proof_burden),
         coupling = coalesce(excluded.coupling, forge_role_assignment.coupling),
         change_novelty = coalesce(excluded.change_novelty, forge_role_assignment.change_novelty),
         worker_fit = coalesce(excluded.worker_fit, forge_role_assignment.worker_fit),
         updated_at = now()`,
      [
        storyId,
        processInstanceId,
        taskId,
        nodeId,
        attempt,
        assignmentId,
        list('finding'),
        list('evidence'),
        arg('reasoning'),
        vector('semantic-surface'),
        vector('dependency-depth'),
        vector('uncertainty'),
        vector('context-burden'),
        vector('proof-burden'),
        vector('coupling'),
        vector('change-novelty'),
        vector('worker-fit'),
      ],
    )

    const written = await pool.query(
      `insert into forge_role_plan_chunk
         (story_id, process_instance_id, task_id, node_id, attempt, assignment_id,
          chunk_id, size, surface, proof, invariant, preconditions, postconditions,
          classes, risks, depends_on)
       values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)
       on conflict (task_id, node_id, attempt, assignment_id, chunk_id) do update set
         size = coalesce(excluded.size, forge_role_plan_chunk.size),
         surface = excluded.surface,
         proof = excluded.proof,
         invariant = coalesce(excluded.invariant, forge_role_plan_chunk.invariant),
         preconditions = excluded.preconditions,
         postconditions = excluded.postconditions,
         classes = excluded.classes,
         risks = excluded.risks,
         depends_on = excluded.depends_on,
         updated_at = now()
       returning assignment_id, chunk_id, surface, proof`,
      [
        storyId,
        processInstanceId,
        taskId,
        nodeId,
        attempt,
        assignmentId,
        chunkId,
        arg('size') ?? arg('chunk-size'),
        surface,
        proof,
        arg('invariant'),
        list('preconditions'),
        list('postconditions'),
        list('classes'),
        list('risks'),
        list('depends-on'),
      ],
    )
    console.log('chunk recorded:', JSON.stringify(written.rows[0]))
    console.log('Do not also emit a LEAD_PLAN JSON line — the rows above ARE the plan.')
  } catch (error) {
    const e = error
    console.error(`PLAN REJECTED by the database: ${e?.message ?? String(error)}`)
    if (e?.constraint) console.error(`failing constraint: ${e.constraint}`)
    console.error('Fix the named field and run the command again.')
    await pool.end()
    process.exit(1)
  }
  await pool.end()
  process.exit(0)
}

if (show || !arg('decision')) {
  const rows = await pool.query(
    `select decision, size, size_reason, reason, assignment_count, finding_ids,
            merge_checks, surface_scope, attempt, updated_at
     from forge_role_contract
     where task_id = $1 and node_id = $2 and attempt = $3`,
    [taskId, nodeId, attempt],
  )
  console.log(rows.rows[0] ? JSON.stringify(rows.rows[0], null, 2) : '(no contract recorded yet)')
  await pool.end()
  process.exit(0)
}

const assignments = arg('assignments')
try {
  const written = await pool.query(
    `insert into forge_role_contract
       (story_id, process_instance_id, task_id, node_id, attempt,
        decision, size, size_reason, reason, assignment_count,
        finding_ids, merge_checks, surface_scope)
     values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
     on conflict (task_id, node_id, attempt) do update set
       decision = excluded.decision,
       size = coalesce(excluded.size, forge_role_contract.size),
       size_reason = coalesce(excluded.size_reason, forge_role_contract.size_reason),
       reason = coalesce(excluded.reason, forge_role_contract.reason),
       assignment_count = coalesce(excluded.assignment_count, forge_role_contract.assignment_count),
       finding_ids = case when cardinality(excluded.finding_ids) > 0
                          then excluded.finding_ids else forge_role_contract.finding_ids end,
       merge_checks = case when cardinality(excluded.merge_checks) > 0
                           then excluded.merge_checks else forge_role_contract.merge_checks end,
       surface_scope = case when cardinality(excluded.surface_scope) > 0
                            then excluded.surface_scope else forge_role_contract.surface_scope end,
       updated_at = now()
     returning decision, size, assignment_count`,
    [
      storyId,
      processInstanceId,
      taskId,
      nodeId,
      attempt,
      arg('decision'),
      arg('size'),
      arg('size-reason'),
      arg('reason'),
      assignments == null ? null : Number.parseInt(assignments, 10),
      list('findings'),
      list('merge-checks'),
      list('scope'),
    ],
  )
  console.log('contract recorded:', JSON.stringify(written.rows[0]))
  console.log('Stop here. Do not also emit a JSON line — the fields above ARE the contract.')
} catch (error) {
  const e = error
  console.error(`CONTRACT REJECTED by the database: ${e?.message ?? String(error)}`)
  if (e?.constraint) console.error(`failing constraint: ${e.constraint}`)
  console.error('Fix the named field and run the command again.')
  await pool.end()
  process.exit(1)
}

await pool.end()
