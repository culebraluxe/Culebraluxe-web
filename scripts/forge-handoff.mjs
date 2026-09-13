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
// EVERY occurrence of a flag, not just the first. A repeated flag is how a caller
// passes two values without inventing a separator.
const values = (name) =>
  args.reduce(
    (acc, token, i) => (token === `--${name}` && args[i + 1] ? [...acc, args[i + 1]] : acc),
    [],
  )

// Comma-separated OR repeated: `--surface a,b` and `--surface a --surface b` both work,
// so nothing that used to be legal stops being legal.
const list = (name) =>
  values(name)
    .flatMap((v) => v.split(','))
    .map((s) => s.trim())
    .filter(Boolean)

// A list of COMMANDS. Never comma-split: a proof command may legitimately contain a
// comma, and splitting one command into two produces a second command that does not
// exist — a silent lie in a frozen-acceptance field. Repeat the flag instead.
const commands = (name) => values(name).map((s) => s.trim()).filter(Boolean)

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

// ---------------------------------------------------------------------------
// IDENTITY IS CHECKED, NOT TRUSTED.
//
// A warm session carries the earlier generations' identity lines in the model's context.
// On 2026-09-13 a lead_pre turn copied one: it wrote its whole contract, assignment and
// chunk under the PREVIOUS run's task id (verified by timestamp — the rows it claimed to
// have just written were stamped six minutes before the run began). The runner then read
// the LIVE task, found nothing, and HOLDed a story that had in fact been routed correctly.
// Nothing was broken except the address, and the address came from chat.
//
// So before ANY write, ask the engine which execution is live for this story/node and
// refuse a mismatched id, naming the live one. A write aimed at a task the engine is not
// running is a write into history: it cannot be reviewed, so it must not be accepted.
//
// Only a CONFLICT is refused. When nothing is live (hand seeding, --show, a scripted
// fixture) there is no evidence of a mismatch and the write proceeds.
if (!show) {
  const live = await pool.query(
    `select task_id from forge_engine_task_execution
      where story_id = $1 and node_id = $2 and status in ('claimed', 'running')
      order by heartbeat_at desc nulls last
      limit 1`,
    [storyId, nodeId],
  )
  const liveTaskId = live.rows[0]?.task_id ? String(live.rows[0].task_id) : null
  if (liveTaskId && liveTaskId !== taskId) {
    console.error(
      `forge-handoff: REFUSED — task ${taskId} is not the live ${nodeId} execution for ` +
        `${storyId}. The live task is ${liveTaskId}. An identity line from earlier in this ` +
        `session is stale: re-read the identity in your CURRENT task line and run the ` +
        `command again with --task ${liveTaskId}.`,
    )
    await pool.end()
    process.exit(2)
  }
}


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
  // ONE LABEL, ONE CASE. The assignment id is a label, and a label that differs only in
  // case is the same assignment — but the writer used to store it verbatim while the plan
  // reader matched chunk-to-assignment EXACTLY. A model that wrote its contract rows under
  // `a` and its chunk under `A1` therefore produced a plan that read as EMPTY, and the Lead
  // reviewer reported "SOLO requires one assignment" for work it had just planned. Observed
  // live on 2026-09-13. Normalized here so no reader has to guess, and matched
  // case-insensitively in the reader so rows already written are not stranded.
  const assignmentId = (arg('assignment') ?? 'a').trim().toLowerCase()
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
  // REASONING IS NOT DECORATION — IT IS READ. The plan reader refuses an assignment with
  // an empty `reasoning` and returns null, which the Lead reviewer can only report as
  // "no assignment": the routing then fails while the model believes it has written a plan.
  // Observed live on 2026-09-13. Required here so the failure is a named field at the
  // boundary instead of a silently unroutable row.
  if (!arg('reasoning')?.trim()) {
    console.error(
      'forge-handoff: a chunk needs --reasoning "<why this chunk is the cheapest sound ' +
        'shape>". The reader refuses an assignment without it and the Lead cannot route.',
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
// --finding mode: record ONE finding (Architect / Scout). Repeat per finding.
//
//   ... --finding-id F1 --summary "<what must land>" --seams "a/b.ts,c/d.ts" \
//       [--required true|false] [--hint SAME_UNIT|SPLIT_CHILD|FOLLOW_UP_STORY|NOTE|HOLD] \
//       [--risks "r1,r2"] [--preconditions ...] [--postconditions ...] [--classes ...]
//
// This is the LAST contract still travelling as reply JSON (FORGE_ARCHITECT_HANDOFF /
// FORGE_FINDINGS_JSON). The database refuses exactly what the architect gate refuses: no
// seam, more than three seams, a required HOLD with no named risk, an unknown hint, a
// blank summary — and it NAMES the failing constraint, so the model fixes the field
// rather than the prose.
if (arg('finding-id')) {
  const findingId = (arg('finding-id') ?? '').trim()
  const summary = (arg('summary') ?? '').trim()
  const seams = list('seams')
  const requiredRaw = (arg('required') ?? '').trim().toLowerCase()
  const required = requiredRaw === '' ? true : !['false', '0', 'no'].includes(requiredRaw)
  const hint = (arg('hint') ?? '').trim() || null

  if (!findingId || !summary || seams.length === 0) {
    console.error(
      'forge-handoff: a finding needs --finding-id <id>, --summary "<what must land>" and ' +
        '--seams <path[,path]> (1..3). A finding that names no seam cannot be dispatched.',
    )
    await pool.end()
    process.exit(2)
  }

  try {
    const written = await pool.query(
      `insert into forge_role_finding
         (story_id, process_instance_id, task_id, node_id, attempt, finding_id,
          summary, required, seams, hint, preconditions, postconditions, classes, risks)
       values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
       on conflict (task_id, node_id, attempt, finding_id) do update set
         summary = excluded.summary,
         required = excluded.required,
         seams = excluded.seams,
         hint = coalesce(excluded.hint, forge_role_finding.hint),
         preconditions = excluded.preconditions,
         postconditions = excluded.postconditions,
         classes = excluded.classes,
         risks = excluded.risks,
         updated_at = now()
       returning finding_id, required, seams, hint`,
      [
        storyId,
        processInstanceId,
        taskId,
        nodeId,
        attempt,
        findingId,
        summary,
        required,
        seams,
        hint,
        list('preconditions'),
        list('postconditions'),
        list('classes'),
        list('risks'),
      ],
    )
    console.log('finding recorded:', JSON.stringify(written.rows[0]))
    console.log(
      'Do not also emit FORGE_ARCHITECT_HANDOFF or FORGE_FINDINGS_JSON — these rows ARE the findings.',
    )
  } catch (error) {
    const e = error
    console.error(`FINDING REJECTED by the database: ${e?.message ?? String(error)}`)
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
      commands('merge-checks'),
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
