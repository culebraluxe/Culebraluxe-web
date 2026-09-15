// ---------------------------------------------------------------------------
// forge-ladder — seed the difficulty ladder: small, UNLANDED, useful stories for
// exercising the engine end to end.
//
// WHY THIS EXISTS
//
// A story whose work has already landed cannot be run: the Architect reports
// "already-landed", the Lead correctly refuses to route, and the run HOLDs. Two
// attempts tonight ended that way. So the ladder is deliberately made of work that
// does NOT exist yet, in one or two files, with a proof command that fails until the
// work is done — and every rung is genuinely useful to the engine rather than a
// token bonfire.
//
//   pnpm forge:ladder            # dry run (default)
//   pnpm forge:ladder --apply    # create the story rows in PROD
//
// The rungs are ordered easiest-first, one coherent unit each, so the Lead should
// route SOLO or one SMITH. FORK AND SPLIT ARE OUT OF SCOPE BY CONSTRUCTION: nothing
// here needs two workers, and the ladder exists to prove the BASE path.
//
// PROD ONLY, and the target is not a choice: the same declaration the reset tool
// uses decides it.
// ---------------------------------------------------------------------------
import { createStoryboardStory } from '../db/storyboard'
import { describeControlPlane } from '../lib/execution-target'

type LadderRung = {
  id: string
  title: string
  goal: string
  scope: string
  acceptance: string
  notes: string
  /** The frozen proof the QA lane runs. Must FAIL before the work and PASS after it. */
  assayCommands: string
}

const LADDER: LadderRung[] = [
  {
    id: 'ENG-FORGE-DOCTOR-01',
    title: 'forge:doctor — read the control plane without changing it',
    goal:
      'One command that answers "is the control plane clear?" before any test, instead of hand-writing the query every time.',
    scope:
      'scripts/forge-doctor.ts (new, read-only operator command), workflow_app/forge/forge-doctor-report.ts (new, the pure formatter it renders: the control-plane report AND the mailbox postcard block), and `package.json` (register the `pnpm forge:doctor` script). No other existing file changes.',
    acceptance:
      'pnpm forge:doctor prints instances, open tasks, open work items and active engine claims, plus the age of the oldest active claim, and writes nothing: no update, no insert, no claim. It also reports whether the scheduled WORKER is alive from its own logs (the newest invocation and the most recent failure reason, from AGENT_WORKER_LOG_DIR, default ~/Library/Logs/CulebraLuxe), and renders the POSTCARD block the Grok<->DeepSeek mailbox asks for - board-vs-table agreement, active decision count, the 7-day ROI rows and the newest learn-pass attempt. Every fact it prints is read from data that already exists on the run base ref; the rendering is a pure function with a unit test, including the empty-control-plane case, the board-drifted-from-table case and the worker-failing case.',
    notes:
      'Requested after a night of clearing stale claims by hand before every run. forge:clean is the writer; this is its read-only sibling, so an operator can look before deciding to clean. Read-only is a hard requirement: a doctor that mutates is not a doctor. The postcard half was added 2026-09-15 when the mailbox protocol went live and the reply had to be assembled by hand from five commands; the doctor already reads every one of those facts, so printing them is the same read, not a second tool. The WORKER-LIVENESS half was added the same afternoon, and it is the reason the doctor earns its name: the scheduled worker had been dying at its own preflight on every tick since 2026-09-03 (463 invocations, exit 2, never reaching pnpm agent:work) because macOS TCC denies a launchd-spawned process access to ~/Documents, and NOTHING in the cockpit or the control plane said so - the board looked healthy and empty. A doctor that reports a clear control plane while the worker has been dead for twelve days is not answering the operator question.',
    assayCommands:
      '- `node --import tsx --test workflow_app/tests/forge-doctor-report.test.ts`',
  },
  {
    id: 'ENG-FORGE-TURN-VISIBILITY-01',
    title: 'Show turns used against the cap in the run evidence',
    goal:
      'Make the turn cap visible before it fires: an operator reading the ENGINE QUEUE should see how much of the generation budget is spent.',
    scope:
      'workflow_app/forge/model-turn-budget.ts (add a pure line renderer) and workflow_app/forge/agent-runtime-role-runner.ts (append that line to the run detail). No behaviour change to the cap itself.',
    acceptance:
      'Each role run records a line naming turns used and the cap, and a generation at or over the cap records the refusal reason it already throws. The renderer is pure and unit tested, including the at-cap and over-cap cases.',
    notes:
      'The cap landed today but is invisible until it trips. Rendering it into the evidence is the cheapest honest fix and keeps the rule legible to a human reading the queue.',
    assayCommands:
      '- `node --import tsx --test workflow_app/tests/forge-model-turn-budget.test.ts`',
  },
  {
    id: 'ENG-FORGE-LEDGER-QA-LABEL-01',
    title: 'Label the dispatch ledger with the QA verdict, not just the Smith exit',
    goal:
      'Close the labelling gap in the dispatch ledger: a unit labelled pass at the Smith exit but failed later by QA is today invisible to the calibration set.',
    scope:
      'db/forge-dispatch-score.ts (add a reader for the latest unlabelled prediction of a story) and workflow_app/forge/agent-runtime-role-runner.ts (at the QA node, record the QA verdict onto that row). No schema change: the outcome columns already exist.',
    acceptance:
      'When the QA lane finishes, the unit it verified carries the QA verdict as its outcome detail, and a QA failure is recorded as fail rather than left as pass. An absent prediction row is reported, never thrown.',
    notes:
      'Found while building the ledger: the label was written only at the Smith exit, so a candidate that landed and then failed QA stayed labelled pass. Recorded as a known gap at the time; this is the slice that closes it.',
    assayCommands:
      '- `node --import tsx --test workflow_app/tests/forge-dispatch-ledger.test.ts`',
  },
]

const args = process.argv.slice(2)
const APPLY = args.includes('--apply')

async function main(): Promise<void> {
  const declared = describeControlPlane(process.env)
  if (declared.target !== 'prod') {
    console.error(
      `forge-ladder: refusing to write to ${String(declared.target ?? 'an undeclared')} ` +
        'environment — the ladder is run by the engine against PROD. Declare APP_ENV=production.',
    )
    process.exit(2)
  }

  for (const rung of LADDER) {
    if (!APPLY) {
      console.log(`[dry-run] would seed ${rung.id} as Planned — ${rung.title}`)
      continue
    }
    try {
      const created = await createStoryboardStory({
        id: rung.id,
        workstream: 'ENGINEERING',
        operatingSurface: 'TECH',
        title: rung.title,
        priority: 'High',
        // Planned, not Complete: the engine is supposed to finish these.
        status: 'Planned',
        notes: rung.notes,
        batch: null,
        goal: rung.goal,
        scope: rung.scope,
        dependencies: null,
        preconditions: null,
        architectBrief: null,
        contextRefs: null,
        acceptanceCriteria: rung.acceptance,
        postconditions: null,
        testMode: 'SCOPED',
        assayCommands: rung.assayCommands,
        packetSha: null,
        completion: 0,
        rollup: true,
        plannedStartAt: null,
        actualStartAt: null,
        completedAt: null,
      })
      console.log(`[seeded] ${created.id} — Planned, proof: ${rung.assayCommands.split('`')[1] ?? '?'}`)
    } catch (error) {
      const message = (error as Error).message
      if (!/already exists/i.test(message)) throw error
      console.log(`[kept] ${rung.id} already exists — left as it is (status is the engine's to change)`)
    }
  }
  if (!APPLY) console.log('dry run only: re-run with --apply to seed the ladder')
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})

