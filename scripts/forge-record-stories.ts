// ---------------------------------------------------------------------------
// forge-record-stories — record completed work as board history.
//
// A finished story that only exists in git is a story nobody can find. This writes the
// story rows for work that has already shipped and been verified, so the board and the
// packet history agree with what actually happened.
//
//   pnpm forge:record-stories            # dry run (default): says what it would write
//   pnpm forge:record-stories --apply    # write
//
// IDEMPOTENT: an existing story is never rewritten (history is not a scratchpad). Re-running
// reports it as kept. The packet's git blob SHA is recorded when the packet exists, so the
// row and the document can be tied together.
//
// PROD ONLY, and the target is not a choice: the same declaration the reset tool uses decides
// it, and this refuses to write anywhere else.
// ---------------------------------------------------------------------------
import { execFileSync } from 'node:child_process'
import { existsSync } from 'node:fs'

import { createStoryboardStory, setStoryboardStatus } from '../db/storyboard'
import { describeControlPlane } from '../lib/execution-target'

type StoryRecord = {
  id: string
  workstream: string
  operatingSurface: string
  priority: string
  title: string
  goal: string
  scope: string
  acceptance: string
  notes: string
  packet: string
  assayCommands: string
}
// One entry per story. The notes carry the evidence: what shipped, what was proven, and the
// commands that prove it. No completion number is ever estimated.
const STORIES: StoryRecord[] = [
  {
    id: 'ENG-FORGE-WARM-SESSION-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    title: 'One OpenCode session per execution generation, with per-role spend',
    goal:
      'Stop paying the seed tax once per role: the model roles of one execution generation share one live session instead of each cold-starting its own.',
    scope:
      'agent-runtime/opencode/opencode-harness-adapter.ts (session marker + pinning), agent-runtime/harness-usage.ts (read by id, delta), agent-runtime/repositories.ts + types.ts + agent-runtime-adapter.ts (carry the measurement to the run row), db/migrations/174_story_run_harness_session.sql.',
    acceptance:
      'Every model role of one generation runs in ONE session, pinned exactly with --session <id> (never a guessed --continue); a different generation uses a different session; a failed session is dropped once so the replacement costs one role; the session id and the role own spend are recorded on the run row.',
    notes:
      'One session per generation, proven live: architect + both Lead attempts + smith + post all ran in ses_f636a0792ffe..., distinct sessions for that generation = 1, while the previous generation stayed separate. Continuity is ON by default (FORGE_SESSION_CONTINUITY=0 opts out); a bare "1" marker from the old format still degrades to --continue. Spend became per role: readSessionUsage reads the pinned session by id and usageDelta subtracts the launch baseline, because the time-window read finds only the session CREATOR and left every resumed role unmeasured. Measured deltas, one generation: architect $0.0076, lead $0.0142/$0.0126/$0.0111. Evidence: 7a2cdc9, f8b49f6; migration 174 applied and verified on DEV and PROD.',
    packet: 'docs/agent/packets/ENG-FORGE-WARM-SESSION-01.md',
    assayCommands:
      '- `node --import tsx --test agent-runtime/opencode/forge-session.test.ts`\n- `node --import tsx --test agent-runtime/harness-usage.test.ts`',
  },
  {
    id: 'ENG-FORGE-FIELD-AUTHORITY-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    title: 'Fields are the authority — and every field a gate keys on is readable',
    goal:
      'A decision never travels only in chat, and a row that IS written is never read as an absent plan.',
    scope:
      'db/migrations/172_forge_role_finding.sql + db/forge-role-finding.ts (findings in rows), db/forge-role-plan.ts (plan assembly), workflow_app/forge/agent-runtime-role-runner.ts (identity line, findings scope, candidate diff), workflow_app/forge/forge-role-mapping.ts (candidate SHA per lane), workflow_app/forge/forge-lead-routing.ts + forge-lead-routing-prompt.ts (one seat, named flags, scale direction), scripts/forge-handoff.mjs (boundary refusals).',
    acceptance:
      'The Architect writes findings as rows and the Lead reads them scoped to the live process instance and newest attempt per node; the Lead routes SOLO from fields alone; an attempt mismatch, empty reasoning or case-variant assignment id is refused at the boundary naming the field; the runner supplies the candidate diff so the Smith gate never refuses landed work.',
    notes:
      'The last contract travelling as chat JSON (the Architect handoff) now lives in forge_role_finding, written by the handoff CLI and read by the role runner; rows win and the reply parser is only the fallback. Running the chain then exposed five defects that each made a correctly planned story unroutable, all with the same shape — an incomplete or mis-keyed write fails SILENTLY, so the gate reports a missing deliverable and it reads as a model failure: (1) the identity line omitted the attempt while every reader keys on (task,node,attempt), so a retry wrote attempt 1 and the runner read 2; (2) the Lead findings read was story-wide, so earlier runs and retried attempts produced duplicate ids and the gate refused the whole context; (3) the chunk command wrote an assignment with NULL reasoning, which the reader treats as NO assignment; (4) assignment and chunk ids were matched case-sensitively, so a and A1 were different assignments; (5) the dispatchability scale rises with difficulty, so worker-fit 5 means "needs a team" and read as praise HOLDs a good story. Also wired here: the QA candidate SHA the Assay lane actually reads (its absence made every story report a verification gap), and the NO_PROGRESS guard that existed and was never called. Evidence: fa7e426, b60f5b5, 689343b, c996f27, d0f4308, d41187a, f5ef7b4, 7a2cdc9, 427fe40, e764c39. Live: FEATURE completes architect -> lead_pre SOLO -> smith -> post -> qa_verify qaPassed=true.',
    packet: 'docs/agent/packets/ENG-FORGE-FIELD-AUTHORITY-01.md',
    assayCommands:
      '- `node --import tsx --test workflow_app/tests/forge-role-readers.test.ts`\n- `node --import tsx --test workflow_app/tests/forge-lead-fields.test.ts`\n- `node --import tsx --test workflow_app/tests/forge-architect-role.test.ts`',
  },
  {
    id: 'ENG-FORGE-FAST-CANDIDATE-01',
    workstream: 'HARDEN',
    operatingSurface: 'TECH',
    priority: 'High',
    title: 'FAST lane: the smith candidate is carried, and the lane completes',
    goal:
      'The cheap lane finishes: a pre-shaped FAST story produces a candidate, passes its frozen proof, and repairs when QA asks.',
    scope:
      'workflow_app/forge/forge-role-mapping.ts (fast_smith + fast_repair_smith carry the committed SHA), workflow_app/tests/forge-role-mapping.test.ts (the six-node fence).',
    acceptance:
      'A FAST story runs fast_lane_entry -> fast_smith -> fast_qa_verify with a real candidate SHA on the evidence; a repair cycle runs when QA asks; and every candidate-producing node is fenced so a future lane cannot be forgotten.',
    notes:
      'fast_smith and fast_repair_smith were missing from the node list that maps the committed SHA onto the evidence, so the runner had no diff to hand the Smith exit gate and the gate refused work it had just watched land and pass ("role did not deliver smith-candidate") — while the same evidence showed CHANGED= and 15 passing tests. A candidate-producing lane is defined by what it produces, not by the route it took, so the FAST nodes now sit beside their serial siblings, fenced by a test that walks all six. Also recorded here, unexplained rather than explained away: the earlier "could not claim agent work item" failure never reproduced from a clean control plane, and the residue actually observed was stale engine task executions (see ENG-FORGE-PRE-RUN-CLEAN-01), not work items. Evidence: b96caa3, 1d06c31, e2d59fa. Live: fast_smith -> candidateSha 4418dd36 -> fast_qa_verify qaPassed=true -> fast_repair_smith -> fast_qa_verify qaPassed=true.',
    packet: 'docs/agent/packets/ENG-FORGE-FAST-CANDIDATE-01.md',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-role-mapping.test.ts`',
  },
  {
    id: 'ENG-FORGE-PRE-RUN-CLEAN-01',
    workstream: 'HARDEN',
    operatingSurface: 'TECH',
    priority: 'High',
    title: 'Clear the control plane before any run, and make the sweep honest',
    goal:
      'No test or engine run is ever read against another run leftover claims, and the tool that clears them reports what it actually did.',
    scope:
      'scripts/forge-story-reset.ts (+ -config) clean mode and reset closing engine claims, db/forge-engine-recovery.ts (the CAS RETURNING), package.json (forge:clean), AGENTS.md + docs/agent/CURRENT.md (the standing setup rule).',
    acceptance:
      'pnpm forge:clean is idempotent, safe to run while peers work (only claims older than --stale-minutes are touched) and prints its post-condition; a recovered claim is interrupted AND its work item is released; running it before a test leaves nothing able to answer for an earlier run.',
    notes:
      'Two defects were found by building the cleanup. (1) reset aborted the instance, obsoleted tasks and cancelled work items but never closed forge_engine_task_execution, so in-flight engine claims stayed claimed forever — 15 accumulated in one afternoon (9 lead_pre, 4 architect, 2 fast_smith); a reset that leaves claims behind is not a reset. (2) recoverStaleForgeEngineClaims CAS had no RETURNING while its guard read updated.length, so EVERY recovery reported a false cas-miss and returned before releasing the work item while quietly interrupting the row — proven by a sweep printing "0 recovered, 15 skipped" seconds after stamping all fifteen rows "stale claim recovered", and then by a controlled probe showing "1 (skipped 0)" with the work item released to Ready. First clean cleared 15 claims, 2 stale instances and 5 open tasks; the control plane now reads instances=0 openTasks=0 openWorkItems=0 activeEngineClaims=0. Evidence: fb9710d.',
    packet: 'docs/agent/packets/ENG-FORGE-PRE-RUN-CLEAN-01.md',
    assayCommands:
      '- `node --import tsx --test workflow_app/tests/forge-recovery.test.ts`\n- `node --import tsx --test workflow_app/tests/forge-story-reset.test.ts`',
  },
]

/** The packet's blob SHA, so the row points at the document that defines it. */
function packetSha(packet: string): string | null {
  if (!existsSync(packet)) return null
  try {
    return execFileSync('git', ['hash-object', packet], { encoding: 'utf8' }).trim()
  } catch {
    return null
  }
}

/** Record one story, or report that history already has it. */
async function recordOne(story: StoryRecord, apply: boolean): Promise<void> {
  const sha = packetSha(story.packet)
  const packetNote = sha ? ` Packet: ${story.packet} (blob ${sha.slice(0, 10)}).` : ''
  if (!apply) {
    console.log(`[dry-run] would record ${story.id} as Complete (${story.workstream})${packetNote}`)
    return
  }
  try {
    const created = await createStoryboardStory({
      id: story.id,
      workstream: story.workstream,
      operatingSurface: story.operatingSurface,
      title: story.title,
      priority: story.priority,
      status: 'Complete',
      notes: story.notes + packetNote,
      batch: null,
      goal: story.goal,
      scope: story.scope,
      dependencies: null,
      preconditions: null,
      architectBrief: null,
      contextRefs: story.packet,
      acceptanceCriteria: story.acceptance,
      postconditions: null,
      testMode: 'SCOPED',
      assayCommands: story.assayCommands,
      packetSha: sha,
      completion: 100,
      rollup: true,
      plannedStartAt: null,
      actualStartAt: null,
      completedAt: new Date().toISOString(),
    })
    console.log(`[created] ${created.id} — Complete, completion=100${packetNote}`)
    return
  } catch (error) {
    const message = (error as Error).message
    if (!/already exists/i.test(message)) throw error
  }
  // HISTORY IS NOT A SCRATCHPAD: an existing row is reported, not rewritten. Only its status
  // is touched, because the work is finished and the board should say so.
  const updated = await setStoryboardStatus(story.id, 'Complete')
  console.log(
    `[kept] ${story.id} already existed — status ${updated.status}, completion ${updated.completion}`,
  )
}

async function main(): Promise<void> {
  const apply = process.argv.includes('--apply')
  const declared = describeControlPlane(process.env)
  if (declared.target !== 'prod') {
    console.error(
      `forge-record-stories: refusing to write to ${String(declared.target ?? 'an undeclared')} ` +
        'environment — board history lives in PROD. Declare APP_ENV=production.',
    )
    process.exit(2)
  }
  for (const story of STORIES) await recordOne(story, apply)
  if (!apply) console.log('dry run only: re-run with --apply to write')
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})

