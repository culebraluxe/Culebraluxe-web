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
import { decideAssignmentWrite, decideContractWrite } from '../db/forge-role-assignment-write.ts'
import { decideFindingWrite } from '../db/forge-role-finding.ts'

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

// THE MEDIATOR AT THE BOUNDARY (captain, 2026-09-16). A model may write anything it likes; this is where its
// words become a row. Mechanical shape tolerance only — case, quotes, whitespace, DECLARED synonyms — and a
// value outside the declared set is REFUSED HERE, naming the field and the accepted set, instead of reaching
// Postgres and surfacing as a database error that sends the reader hunting the wrong fault. Nothing is
// inferred and nothing is defaulted: a decision with a default is a decision nobody made.
import { ARCHITECT_HINT, ARCHITECT_REQUIRED, LEAD_DECISION, LEAD_SIZE, describeRefusal, mediateField } from '../lib/field-mediator'
import { acceptanceClauses } from '../workflow_app/forge/agents/qa/types'

/**
 * THE ACCEPTANCE-TO-ASSERTION MAPPING, CARRIED BY THE CONTRACT (ENG-FORGE-ACCEPTANCE-SUPPLIER-01).
 *
 * Repeated flag: `--acceptance-assertion "<clause>=<assertion-ref>"`. A malformed entry (no `=`, or an
 * empty side) is REFUSED here, naming the flag — never dropped, because a silently dropped entry is a
 * clause that comes back UNPROVEN with no record of why. The clause must name one of the STORY's own
 * acceptance clauses; an unmatched clause is refused too, so a mapping that maps nothing cannot be
 * written as if it mapped something.
 *
 * Returns `null` when the flag was not given: absence stays absence, which is a different fact from a
 * mapping that was given and refused.
 */
const parseAcceptanceAssertions = (storyClauses) => {
  const entries = values('acceptance-assertion')
  if (entries.length === 0) return { ok: true, mapping: null }
  const mapping = {}
  for (const raw of entries) {
    const text = raw.trim()
    const at = text.indexOf('=')
    const clause = at >= 0 ? text.slice(0, at).trim() : ''
    const ref = at >= 0 ? text.slice(at + 1).trim() : ''
    if (!clause || !ref) {
      console.error(
        `forge-handoff: --acceptance-assertion needs "<clause>=<assertion-ref>" with both sides ` +
          `non-empty — got ${JSON.stringify(raw)}. Nothing was written.`,
      )
      return { ok: false, mapping: null }
    }
    if (storyClauses.length > 0 && !storyClauses.includes(clause)) {
      console.error(
        `forge-handoff: --acceptance-assertion names a clause that is not one of the story's ` +
          `acceptance clauses: ${JSON.stringify(clause)}. Quote the clause verbatim. Nothing was written.`,
      )
      return { ok: false, mapping: null }
    }
    mapping[clause] = [...(mapping[clause] ?? []), ref]
  }
  return { ok: true, mapping }
}

/** Mediate a REQUIRED closed value. Returns null after printing the refusal; the caller exits 2. */
const closed = (declaration, raw, flag) => {
  const mediated = mediateField(declaration, raw ?? '')
  if (mediated.ok) return String(mediated.value)
  console.error(`forge-handoff: ${describeRefusal(mediated)} (--${flag})`)
  return null
}

/** Mediate an OPTIONAL closed value: absent stays absent, which is a different fact from a bad value. */
const closedOptional = (declaration, raw, flag) => {
  const given = (raw ?? '').trim()
  if (given === '') return { value: null, given: false }
  return { value: closed(declaration, given, flag), given: true }
}

/**
 * Mediate a flag that may be REPEATED (Astra, 2026-09-16). `arg()` reads only the first occurrence, so
 * `--decision SOLO --decision HOLD` silently took SOLO — a conflict resolved by argument order, which is
 * exactly the winner-picking this system forbids. Two different answers is a HOLD naming both, in any order,
 * with any delimiter, for however many repetitions.
 */
const closedUnique = (declaration, name, flag) => {
  const mediated = values(name).map((raw) => closed(declaration, raw, flag))
  if (mediated.some((value) => value === null)) return null
  const unique = [...new Set(mediated)]
  if (unique.length > 1) {
    console.error(
      `forge-handoff: ${flag}: CONFLICT — repeated flags said ${unique.join(' | ')}. Nothing is picked; ` +
        'state one value and HOLD on the disagreement.',
    )
    return null
  }
  return unique.length === 1 ? unique[0] : null
}

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

  // THE DECISION OWNS THE WRITE (2026-09-17). The upsert below used to replace `finding_ids`
  // whenever the write declared any, so three chunk writes on one assignment kept only the last
  // chunk's findings. The union/refuse rule lives in `db/forge-role-assignment-write.ts`; this call
  // is its only writer, and a refused write returns before any SQL, leaving the row untouched.
  const existingAssignment = await pool.query(
    `select finding_ids from forge_role_assignment
      where task_id = $1 and node_id = $2 and attempt = $3 and assignment_id = $4`,
    [taskId, nodeId, attempt, assignmentId],
  )
  const assignmentWrite = decideAssignmentWrite(
    existingAssignment.rows[0]
      ? { assignmentId, attempt, findingIds: existingAssignment.rows[0].finding_ids ?? [] }
      : null,
    { assignmentId, attempt, findingIds: list('finding') },
  )
  if (assignmentWrite.kind === 'refuse') {
    console.error(
      `forge-handoff: REFUSED — assignment ${assignmentWrite.assignmentId} attempt ` +
        `${assignmentWrite.attempt} would drop finding(s) ${assignmentWrite.dropped.join(', ')}. ` +
        'Nothing was written: the assignment row is untouched. Add the dropped findings to the ' +
        'write, or use a different --assignment id.',
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
         finding_ids = excluded.finding_ids,
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
        assignmentWrite.findingIds,
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
  // ASTRA'S ITEM 4 (2026-09-16): AN INVALID `required` REFUSES. It used to be `!['false','0','no'].includes(raw)`,
  // so `--required maybe` silently became TRUE — an unreadable value read as a yes, which is winner-picking by
  // fallthrough. Spellings now come from the mediator's boolean vocabulary (one definition, not a second list),
  // an explicit true/false keeps its meaning, and an OMITTED flag keeps the documented default: a finding is
  // required unless the Architect says otherwise. Absence and a bad value are different facts.
  const requiredRaw = (arg('required') ?? '').trim()
  let required = true
  if (requiredRaw !== '') {
    const mediated = mediateField({ ...ARCHITECT_REQUIRED, decision: false }, requiredRaw)
    if (!mediated.ok) {
      console.error(`forge-handoff: ${describeRefusal(mediated)} (--required)`)
      await pool.end()
      process.exit(2)
    }
    required = Boolean(mediated.value)
  }
  // THE HINT CROSSES THE MEDIATOR. It used to go straight into SQL, so an unknown hint was refused by the
  // DATABASE and read as a driver problem. Absent stays absent (hint is optional); a bad hint is refused here,
  // naming the field and the accepted set.
  const hint = closedOptional(ARCHITECT_HINT, arg('hint'), 'hint')
  if (hint.given && hint.value === null) {
    await pool.end()
    process.exit(2)
  }

  if (!findingId || !summary || seams.length === 0) {
    console.error(
      'forge-handoff: a finding needs --finding-id <id>, --summary "<what must land>" and ' +
        '--seams <path[,path]> (1..3). A finding that names no seam cannot be dispatched.',
    )
    await pool.end()
    process.exit(2)
  }

  // A LATER ATTEMPT MAY NOT SILENTLY DROP A SEAM THE EARLIER ONE DECLARED.
  //
  // Until this guard the write upserted one row per finding and never looked at attempt N-1, and
  // the reader's newest-attempt scope then hid the loss from the Lead. The rule lives once, in
  // `db/forge-role-finding.ts` (decideFindingWrite), so this script and its test read the same
  // decision instead of two copies. A drop is REFUSED by name, or recorded as an explicit
  // supersede row — never a silent loss.
  const priorRow = await pool.query(
    `select max(attempt) as attempt from forge_role_finding
      where story_id = $1 and process_instance_id = $2 and node_id = $3 and attempt < $4`,
    [storyId, processInstanceId, nodeId, attempt],
  )
  const priorAttempt = priorRow.rows[0]?.attempt == null ? null : Number(priorRow.rows[0].attempt)
  const priorFindings =
    priorAttempt == null
      ? []
      : (
          await pool.query(
            `select finding_id, seams from forge_role_finding
              where story_id = $1 and process_instance_id = $2 and node_id = $3 and attempt = $4`,
            [storyId, processInstanceId, nodeId, priorAttempt],
          )
        ).rows
  const currentFindings = (
    await pool.query(
      `select finding_id, seams from forge_role_finding
        where task_id = $1 and node_id = $2 and attempt = $3 and finding_id <> $4`,
      [taskId, nodeId, attempt, findingId],
    )
  ).rows
  const seamSet = (row) => ({
    findingId: String(row.finding_id),
    seams: Array.isArray(row.seams) ? row.seams : [],
  })
  const findingWrite = decideFindingWrite({
    attempt,
    prior:
      priorAttempt == null ? null : { attempt: priorAttempt, findings: priorFindings.map(seamSet) },
    current: currentFindings.map(seamSet),
    incoming: { findingId, seams },
    acknowledged: list('supersede'),
  })
  if (findingWrite.kind === 'refuse') {
    const named = findingWrite.dropped
      .map((d) => `${d.seam} (declared by attempt ${d.declaredByAttempt} in ${d.declaredByFindingId})`)
      .join(', ')
    console.error(
      `forge-handoff: REFUSED — attempt ${attempt} would drop seam(s) declared by an earlier ` +
        `attempt: ${named}. Nothing was written. Re-declare the seam, or acknowledge the loss ` +
        'with --supersede <seam> for each dropped seam.',
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
        hint.value,
        list('preconditions'),
        list('postconditions'),
        list('classes'),
        list('risks'),
      ],
    )
    // ONE TYPED ROW PER ACKNOWLEDGED DROP. A refusal wrote nothing, so the presence of these
    // rows is what makes an explicit supersede distinguishable from a refusal — for the Lead and
    // for any later reader.
    for (const entry of findingWrite.superseded) {
      await pool.query(
        `insert into forge_role_finding_supersede
           (story_id, process_instance_id, task_id, node_id, attempt, finding_id, seam,
            declared_by_attempt, declared_by_finding_id)
         values ($1,$2,$3,$4,$5,$6,$7,$8,$9)
         on conflict (task_id, node_id, attempt, seam) do update set
           finding_id = excluded.finding_id,
           declared_by_attempt = excluded.declared_by_attempt,
           declared_by_finding_id = excluded.declared_by_finding_id`,
        [
          storyId,
          processInstanceId,
          taskId,
          nodeId,
          attempt,
          findingId,
          entry.seam,
          entry.declaredByAttempt,
          entry.declaredByFindingId,
        ],
      )
    }
    console.log('finding recorded:', JSON.stringify(written.rows[0]))
    if (findingWrite.superseded.length > 0) {
      console.log(
        'superseded seam(s) recorded:',
        findingWrite.superseded.map((entry) => entry.seam).join(', '),
      )
    }
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

// THE DECISION AND THE SIZE CROSS THE MEDIATOR BEFORE THEY REACH SQL. `--decision` is required here (an
// absent one routed to --show above), so a value outside the set is refused with the accepted set; `--size`
// is optional and absent stays absent.
const decisionValue = closedUnique(LEAD_DECISION, 'decision', 'decision')
const sizeValue = closedOptional(LEAD_SIZE, arg('size'), 'size')
if (decisionValue === null || (sizeValue.given && sizeValue.value === null)) {
  await pool.end()
  process.exit(2)
}

// THE ACCEPTANCE MAPPING THE CONTRACT CARRIES (ENG-FORGE-ACCEPTANCE-SUPPLIER-01). The clause check
// reads the story's OWN acceptance criteria, so an unmatched clause is refused by name here rather
// than written as a mapping that maps nothing.
const storyAcceptanceRows = await pool.query(
  'select acceptance_criteria from storyboard_story where id = $1',
  [storyId],
)
const acceptance = parseAcceptanceAssertions(
  acceptanceClauses(storyAcceptanceRows.rows[0]?.acceptance_criteria ?? null),
)
if (!acceptance.ok) {
  await pool.end()
  process.exit(2)
}
const acceptanceJson = acceptance.mapping ? JSON.stringify(acceptance.mapping) : null

// THE CONTRACT ROW HAS ONE WRITER TOO (2026-09-17). The three array columns used to be replaced by any
// non-empty write (`case when cardinality(excluded.x) > 0 ...`), a second rule beside the assignment
// row's decider. The same `decideAssignmentWrite` now decides each column against its OWN existing
// value: an add unions, an empty write is a no-op, and a write that would shrink a column is refused by
// name before any SQL runs. A shrunken surface_scope is the dangerous one — it can make two genuinely
// overlapping lanes read as disjoint.
const existingContract = await pool.query(
  `select finding_ids, merge_checks, surface_scope from forge_role_contract
    where task_id = $1 and node_id = $2 and attempt = $3`,
  [taskId, nodeId, attempt],
)
const contractWrite = decideContractWrite(
  existingContract.rows[0]
    ? {
        findingIds: existingContract.rows[0].finding_ids ?? [],
        mergeChecks: existingContract.rows[0].merge_checks ?? [],
        surfaceScope: existingContract.rows[0].surface_scope ?? [],
      }
    : null,
  {
    findingIds: list('findings'),
    mergeChecks: commands('merge-checks'),
    surfaceScope: list('scope'),
  },
)
if (contractWrite.kind === 'refuse') {
  console.error(
    `forge-handoff: REFUSED — contract ${contractWrite.column} would drop ` +
      `${contractWrite.dropped.join(', ')}. Nothing was written: the contract row is untouched. ` +
      'Add the dropped entries to the write, or use a different attempt.',
  )
  await pool.end()
  process.exit(2)
}
try {
  const written = await pool.query(
    `insert into forge_role_contract
       (story_id, process_instance_id, task_id, node_id, attempt,
        decision, size, size_reason, reason, assignment_count,
        finding_ids, merge_checks, surface_scope, acceptance_assertions)
     values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14::jsonb)
     on conflict (task_id, node_id, attempt) do update set
       decision = excluded.decision,
       size = coalesce(excluded.size, forge_role_contract.size),
       size_reason = coalesce(excluded.size_reason, forge_role_contract.size_reason),
       reason = coalesce(excluded.reason, forge_role_contract.reason),
       assignment_count = coalesce(excluded.assignment_count, forge_role_contract.assignment_count),
       finding_ids = excluded.finding_ids,
       merge_checks = excluded.merge_checks,
       surface_scope = excluded.surface_scope,
       -- AN ABSENT MAPPING DOES NOT CLEAR A DECLARED ONE: a re-write that omits the flag leaves the
       -- mapping untouched, the same contract the other scalar fields already follow.
       acceptance_assertions = coalesce(excluded.acceptance_assertions, forge_role_contract.acceptance_assertions),
       updated_at = now()
     returning decision, size, assignment_count`,
    [
      storyId,
      processInstanceId,
      taskId,
      nodeId,
      attempt,
      decisionValue,
      sizeValue.value,
      arg('size-reason'),
      arg('reason'),
      assignments == null ? null : Number.parseInt(assignments, 10),
      contractWrite.findingIds,
      contractWrite.mergeChecks,
      contractWrite.surfaceScope,
      acceptanceJson,
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
