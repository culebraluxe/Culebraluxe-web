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
