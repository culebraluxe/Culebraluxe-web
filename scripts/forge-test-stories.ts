// ---------------------------------------------------------------------------
// forge-test-stories — seed the board with stories whose purpose is to DRIVE
// Forge and to leave real work behind, so the token spend buys something.
//
//   pnpm forge:test-stories            # dry run (default): says what it would write
//   pnpm forge:test-stories --apply    # write
//
// Each story is bounded, each one exercises a seam that is still unproven in a
// live run, and each one delivers a capability the operator asked for. The
// proof is a test file the story itself creates, so the proof FAILS at base
// (a story whose proof is already green cannot be evidence of work) and passes
// only when the change exists.
//
// IDEMPOTENT: an existing story is reported, never rewritten — history is not a
// scratchpad. PROD ONLY: the same declaration the other board writers use.
// ---------------------------------------------------------------------------
import { createStoryboardStory } from '../db/storyboard'
import { describeControlPlane } from '../lib/execution-target'

type TestStory = {
  id: string
  workstream: string
  operatingSurface: string
  priority: string
  title: string
  goal: string
  scope: string
  acceptance: string
  notes: string
  assayCommands: string
  /** The sprint the deployment defers to. Wave 2 is batch 2. */
  batch?: number
}


const STORIES: TestStory[] = [
  {
    id: 'ENG-FORGE-RECEIPT-KIND-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    title: 'A release receipt is recorded by its kind, never by elimination',
    goal:
      'A receipt whose kind is not `deployment` can never set `deploymentReceipt` or `deployedSha`, so a ' +
      'publish or an integration is never recorded as a deployment.',
    scope:
      'workflow_app/forge/agents/role-agents.ts (DevOpsAgent.collect — decide by kind, not by "not ' +
      'production_verification"), workflow_app/tests/forge-receipt-kind.test.ts (new).',
    acceptance:
      'Given a release receipt of kind `integration`, DevOpsAgent records neither deploymentReceipt nor ' +
      'deployedSha. Given kind `deployment`, it records both. Given `production_verification`, it records ' +
      'the production receipt and productionVerifiedSha only. No branch may treat an unlisted kind as a ' +
      'deployment.',
    notes:
      'FOUND 2026-09-16 by reading Neon: artifact 806dd3aa and the durable evidence row it mirrors carried ' +
      '`deploymentReceipt=push:08b569f8…` and `deployedSha=08b569f8…` for a publish that was never a ' +
      'deployment. Root cause: `DevOpsAgent.collect` recorded ANY receipt that was not ' +
      '`production_verification` as a deployment, so `integration` lands there too; and the runner feeds a ' +
      'stored receipt back in as `kind: "deployment"`, so one bad receipt re-armed itself on every pass. ' +
      'The kinds are `deployment | production_verification | integration`.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-receipt-kind.test.ts`',
  },
  {
    id: 'ENG-FORGE-HOLD-VISIBLE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    title: 'A story on Hold says why, and where it stopped',
    goal:
      'A held story reports its hold reason and the node it held at, read from the durable hold record, so ' +
      'an operator never has to infer why a chain parked.',
    scope:
      'workflow_app/forge/forge-visibility.ts (add the hold line to the snapshot), ' +
      'workflow_app/tests/forge-hold-visibility.test.ts (new).',
    acceptance:
      'Given an open hold record for a story, the visibility snapshot names the reason and the originating ' +
      'node. Given no hold record, it reports none. A hold record with no reason reads as unknown — never ' +
      'as an empty string that reads like "no problem".',
    notes:
      'WRITTEN 2026-09-16 after a live drive parked at the hold gate: the reason existed only in ' +
      'forge_hold_record and process_events, so answering "why is this story held?" took a database query ' +
      'rather than a look at the board.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-hold-visibility.test.ts`',
  },
  {
    id: 'ENG-FORGE-RUN-SPEND-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    title: 'A story reports what it cost, per lane and in total',
    goal:
      'A story reports the spend of its own runs and lanes from the durable run rows, with unknown spend ' +
      'as null, so the cost of a story is readable without a separate report.',
    scope:
      'workflow_app/forge/forge-visibility.ts (spend block on the snapshot), ' +
      'workflow_app/tests/forge-run-spend.test.ts (new).',
    acceptance:
      'Per-lane spend is read from the durable run rows (tokens_input, tokens_output, cost_usd, ' +
      'cost_widgets, cost_source). A run with no recorded spend reports null and NEVER 0 — an unmeasured ' +
      'zero and a measured zero are different facts. The total is the sum of what is known, or null when ' +
      'nothing is known.',
    notes:
      'WRITTEN 2026-09-16 for the operator rule that a story should return something for the tokens it ' +
      'spends: the spend columns already exist on storyboard_story_run and are already read by ' +
      'db/storyboard.ts, so this makes them visible per story rather than only in aggregate.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-run-spend.test.ts`',
  },
  {
    id: 'ENG-FORGE-QA-VERDICT-VISIBLE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    title: 'The QA verdict is readable per story, and a run that measured nothing says so',
    goal:
      'A story view reports qaPassed with each frozen command and its exit code, and reports NO verdict — ' +
      'rather than a historical one — for a run that measured nothing.',
    scope:
      'workflow_app/forge/forge-visibility.ts (verdict block), workflow_app/forge/forge-evidence-db.ts ' +
      '(read only, no behaviour change), workflow_app/tests/forge-qa-verdict-visible.test.ts (new).',
    acceptance:
      'Given a run whose QA passed, the view reports qaPassed=true with each frozen command and its exit ' +
      'code. Given a run with no rows of its own, it reports no verdict at all rather than a verdict from ' +
      'another run. A failed command is named with its exit code. The verdict carries no SHA.',
    notes:
      'WRITTEN 2026-09-16 to exercise the one seam that is still unproven in a live run: the QA verdict ' +
      'and the durable row it writes. It also pins the rule that a story verdict belongs to its own run ' +
      'only, so this test fails if the scoping regresses.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-qa-verdict-visible.test.ts`',
  },
  // --- WAVE 2 — run these like a stranger would: drive, watch, touch nothing --------------------
  {
    id: 'ENG-FORGE-RELEASE-MARKERS-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 2,
    title: 'A resolved release stops reporting the failure it resolved',
    goal:
      'When a release stage succeeds after failing, the story stops reporting the old failure, so a ' +
      'router that reads the failed stage cannot send a resolved story back to the stage that succeeded.',
    scope:
      'workflow_app/forge/db-release-executor.ts (clear the markers with the success it records), ' +
      'workflow_app/tests/forge-release-markers.test.ts (new).',
    acceptance:
      'Given a release stage that failed and then succeeded, the durable evidence reports failureClass ' +
      'null and failedReleaseStage null after the success, and the success flag true. A failure NOT yet ' +
      'followed by a success keeps its markers — the record is corrected, never blanked.',
    notes:
      'FOUND 2026-09-16 while closing ENG-FORGE-RECEIPT-KIND-01: the publish succeeded, origin/main ' +
      'advanced, and the row still read failure_class=ENVIRONMENT with failed_release_stage=PUBLISH. ' +
      'devops_resume_router routes ON failedReleaseStage, so a resolved failure can send a story back to ' +
      'the stage that already succeeded.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-release-markers.test.ts`',
  },
  {
    id: 'ENG-FORGE-LANE-LABELS-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 2,
    title: 'Each run is named by its lane, in words',
    goal:
      'A story view names each run by the lane that produced it — Architect, Lead, Smith, QA, DEV_OPS — ' +
      'instead of a raw run_type token.',
    scope:
      'workflow_app/forge/forge-visibility.ts (additive label per run), ' +
      'workflow_app/tests/forge-lane-labels.test.ts (new).',
    acceptance:
      'Every run_type the engine writes maps to a human label. An unknown run_type is reported verbatim ' +
      'rather than guessed, and never as an empty string.',
    notes:
      'WRITTEN 2026-09-16: reading a story history meant translating run_type tokens by hand, so the ' +
      'operator could not see at a glance which lane did what.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-lane-labels.test.ts`',
  },
  {
    id: 'ENG-FORGE-QA-NO-GIT-GUARD-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 2,
    title: 'A guard test keeps QA out of git',
    goal:
      'A test fails if a QA module reads git or carries a sha, so the rule that QA answers only "did the ' +
      'tests pass" cannot regress silently.',
    scope: 'workflow_app/tests/forge-qa-no-git.test.ts (new).',
    acceptance:
      'The test reads the QA modules as text and fails if any of them names a sha field, runs a git ' +
      'command, or checks lineage; it passes on the current tree, and it fails if such a reference is ' +
      'added back.',
    notes:
      'WRITTEN 2026-09-16 after the publish gate demanded a QA-held sha and refused every release ' +
      '(instance 8fc792a6: qa_passed=true, qa_verified_sha=null, publish_succeeded=false). QA answers ' +
      'for the tests; the release path asks for that answer.',
    assayCommands:
      '- `node --import tsx --test workflow_app/tests/forge-qa-no-git.test.ts`\n' +
      '- `node --import tsx --test workflow_app/tests/forge-qa-seam.test.ts`',
  },
  {
    id: 'ENG-FORGE-REPAIR-BUDGET-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    batch: 2,
    title: 'A story reports the repair budget it has left',
    goal:
      'A story view reports the repairs and replans it has used and how many remain, so a story about to ' +
      'exhaust its budget is visible before it holds.',
    scope:
      'workflow_app/forge/forge-visibility.ts (additive), ' +
      'workflow_app/tests/forge-repair-budget.test.ts (new).',
    acceptance:
      'Used and remaining counts are read from the durable counters against the engine caps. An unknown ' +
      'cap reports remaining as null, NEVER as zero — "we do not know" and "none left" are different ' +
      'facts, and only one of them is a hold.',
    notes:
      'WRITTEN 2026-09-16: a story that runs out of repair budget holds, and nothing on the board showed ' +
      'how close it was until it did.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-repair-budget.test.ts`',
  },
]

/**
 * Write one story as Planned. A story that already exists is REPORTED, not rewritten: its notes are the
 * record of what happened, not a place to keep a seed file in sync.
 */
async function recordOne(story: TestStory, apply: boolean): Promise<void> {
  if (!apply) {
    console.log(`[dry-run] would record ${story.id} as Planned — ${story.title}`)
    return
  }
  try {
    const created = await createStoryboardStory({
      id: story.id,
      workstream: story.workstream,
      operatingSurface: story.operatingSurface,
      title: story.title,
      priority: story.priority,
      status: 'Planned',
      notes: story.notes,
      batch: story.batch ?? null,
      goal: story.goal,
      scope: story.scope,
      dependencies: null,
      preconditions: null,
      architectBrief: null,
      contextRefs: null,
      acceptanceCriteria: story.acceptance,
      postconditions: null,
      testMode: 'SCOPED',
      assayCommands: story.assayCommands,
      packetSha: null,
      completion: 0,
      rollup: false,
      plannedStartAt: null,
      actualStartAt: null,
      completedAt: null,
    })
    console.log(`[created] ${created.id} — Planned, completion 0`)
    return
  } catch (error) {
    const message = (error as Error).message
    if (!/already exists/i.test(message)) throw error
  }
  console.log(`[kept] ${story.id} already exists — left exactly as it is`)
}

async function main(): Promise<void> {
  const apply = process.argv.includes('--apply')
  const declared = describeControlPlane(process.env)
  if (declared.target !== 'prod') {
    console.error(
      `forge-test-stories: refusing to write to ${String(declared.target ?? 'an undeclared')} ` +
        'environment — the board lives in PROD. Declare APP_ENV=production.',
    )
    process.exit(2)
  }
  // The ready gate refuses a story with no acceptance criteria or no assay plan, so a seeded story that is
  // missing either could never start. Fail here instead of leaving a story that cannot run.
  for (const story of STORIES) {
    if (!story.acceptance.trim()) throw new Error(`${story.id}: no acceptance criteria — the ready gate refuses it`)
    if (!story.assayCommands.trim()) throw new Error(`${story.id}: no assay plan — the ready gate refuses it`)
  }
  for (const story of STORIES) await recordOne(story, apply)
  if (!apply) console.log('dry run only: re-run with --apply to write')
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
