#!/usr/bin/env node
import {
  accountingCaveats,
  closeSprint,
  formatCoverage,
  formatDuration,
  formatSprintCost,
  formatSprintLaneTime,
  formatUsd,
  formatWidgets,
  getSprintBoardByNumber,
  listSprintBoard,
  openSprint,
  setSprintGoal,
  snapshotSprint,
  unassignedAccounting,
  type SprintBoard,
} from '../db/sprint'
import { PortalWriteError } from '../lib/portal-write-error'

// ---------------------------------------------------------------------------
// THE SPRINT PARENT, AS A COMMAND (migration 187).
//
// The board has carried a `batch` integer for a long time; this is the row that integer points at,
// with a goal, an owner, a status and — required at close — an outcome. Sprints are how this work is
// run from here on, so the operations a captain actually needs are here rather than in ad-hoc SQL:
//
//   node --import tsx --env-file=.env.local scripts/sprint.ts list
//   node --import tsx --env-file=.env.local scripts/sprint.ts show 92
//   node --import tsx --env-file=.env.local scripts/sprint.ts open 101 "Sprint 101" --goal "…"
//   node --import tsx --env-file=.env.local scripts/sprint.ts goal 101 "…"
//   node --import tsx --env-file=.env.local scripts/sprint.ts close 101 --outcome "what happened"
//
// Reads and refusals print a sentence and exit non-zero; a refusal never half-writes. Story
// membership is NOT set here: a story joins a sprint by its batch, and the database derives the link.
// ---------------------------------------------------------------------------

const args = process.argv.slice(2)
const flag = (name: string): string | null => {
  const i = args.indexOf(`--${name}`)
  return i >= 0 ? args[i + 1] ?? null : null
}
const positional = args.filter((a, i) => !a.startsWith('--') && (i === 0 || !args[i - 1].startsWith('--')))
const [command, ...rest] = positional

function usage(): never {
  console.error(
    [
      'usage: sprint <command>',
      '  list',
      '  show <number>',
      '  open <number> "<title>" [--goal "…"] [--theme "…"] [--owner "…"]',
      '  goal <number> "<goal>"',
      '  close <number> --outcome "<what actually happened>"',
      '  snapshot <number>',
    ].join('\n'),
  )
  process.exit(2)
}

const bar = (percent: number | null): string =>
  percent === null ? '  n/a' : `${percent.toFixed(1).padStart(5)}%`

function line(sprint: SprintBoard): string {
  const counts = `${sprint.storiesComplete}/${sprint.stories} done, ${sprint.storiesOpen} open${
    sprint.storiesHeld > 0 ? `, ${sprint.storiesHeld} HELD` : ''
  }`
  const goal = sprint.goal ? `  — ${sprint.goal}` : '  — (no goal recorded)'
  const cost = formatSprintCost(sprint).padStart(8)
  const lane = formatSprintLaneTime(sprint)
  return `${sprint.id.padEnd(5)} ${sprint.status.padEnd(8)} ${bar(sprint.percentComplete)}  ${counts.padEnd(
    26,
  )} ${cost}  ${lane.padEnd(9)}${goal}`
}

async function fail(message: string): Promise<never> {
  console.error(message)
  process.exit(1)
}

async function main(): Promise<void> {
  if (!command) usage()

  if (command === 'list') {
    const rows = await listSprintBoard()
    if (rows === null) {
      await fail('sprints: table not present (apply migration 187)')
      return
    }
    if (rows.length === 0) console.log('no sprints yet')
    for (const sprint of rows) console.log(line(sprint))
    // THE OMISSION IS STATED, NOT SWALLOWED. Runs belong to a sprint only through their story, so a
    // story with no batch is spending nobody's sprint total contains. Measured on prod 2026-09-17:
    // 493 of 1075 runs, $3.85, 525 lane hours.
    const loose = await unassignedAccounting()
    if (loose.runs > 0) {
      console.log(
        `\nnot in any sprint: ${loose.stories} stories, ${loose.runs} runs, ` +
          `${formatUsd(loose.costUsd)} (${formatCoverage(loose.runsWithUsd, loose.runs)}), ` +
          `${formatWidgets(loose.costWidgets)}, ${formatDuration(loose.runSeconds)} of lane time`,
      )
    }
    return
  }

  if (command === 'show') {
    const number = Number(rest[0])
    if (!Number.isFinite(number)) usage()
    const sprint = await getSprintBoardByNumber(number)
    if (!sprint) {
      await fail(`sprint S${number} does not exist`)
      return
    }
    console.log(line(sprint))
    console.log(`  title      ${sprint.title}`)
    if (sprint.theme) console.log(`  theme      ${sprint.theme}`)
    console.log(`  owner      ${sprint.owner ?? '(unowned)'}`)
    console.log(`  started    ${sprint.startedAt ?? '(not recorded)'}`)
    console.log(`  target end ${sprint.targetEndAt ?? '(not set)'}`)
    if (sprint.closedAt) console.log(`  closed     ${sprint.closedAt}`)
    if (sprint.outcome) console.log(`  outcome    ${sprint.outcome}`)
    console.log('  ---- accounting (from the runs the engine recorded)')
    console.log(`  vendor USD   ${formatUsd(sprint.costUsd)} across ${formatCoverage(sprint.runsWithUsd, sprint.runs)}`)
    console.log(`  widgets      ${formatWidgets(sprint.costWidgets)} across ${formatCoverage(sprint.runsWithWidgets, sprint.runs)}`)
    console.log(
      `  tokens       ${sprint.tokensInput ?? 0} in / ${sprint.tokensOutput ?? 0} out`,
    )
    console.log(`  lane time    ${formatDuration(sprint.runSeconds)} (summed run time)`)
    console.log(`  wall time    ${formatDuration(sprint.wallSeconds)} (first run start to last run end)`)
    console.log(
      `  cycle time   ${formatDuration(sprint.meanCycleSeconds)} mean over ${sprint.storiesWithCycle} of ${sprint.stories} stories`,
    )
    for (const caveat of accountingCaveats(sprint)) console.log(`  ! ${caveat}`)
    if (sprint.notes) console.log(`  notes      ${sprint.notes}`)
    return
  }

  if (command === 'snapshot') {
    const number = Number(rest[0])
    if (!Number.isFinite(number)) usage()
    const snapped = await snapshotSprint(number)
    if (!snapped) {
      await fail(`sprint S${number} does not exist`)
      return
    }
    console.log(
      `snapshot taken for S${number}: ${formatUsd(snapped.costUsd)} USD, ${formatWidgets(snapped.costWidgets)}, ${formatDuration(snapped.runSeconds)} of lane time`,
    )
    console.log('  the live figures will keep moving as invoices arrive; this records what we knew now.')
    return
  }

  if (command === 'open') {
    const number = Number(rest[0])
    const title = rest[1] ?? titleFromRemainder()
    if (!Number.isFinite(number) || !title) usage()
    const created = await openSprint({
      number,
      title,
      goal: flag('goal'),
      theme: flag('theme'),
      owner: flag('owner'),
    })
    console.log(`opened ${created.id}: ${created.title} (${created.status})`)
    if (!created.goal) {
      console.log('  NOTE: no goal recorded. A sprint with no goal cannot be judged at close — add one.')
    }
    return
  }

  if (command === 'goal') {
    const number = Number(rest[0])
    const goal = rest.slice(1).join(' ') || flag('goal')
    if (!Number.isFinite(number) || !goal) usage()
    const updated = await setSprintGoal(number, goal)
    console.log(`${updated.id} goal: ${updated.goal}`)
    return
  }

  if (command === 'close') {
    const number = Number(rest[0])
    const outcome = flag('outcome') ?? rest.slice(1).join(' ')
    if (!Number.isFinite(number)) usage()
    const closed = await closeSprint(number, outcome ?? '')
    console.log(`closed ${closed.id}: ${closed.outcome}`)
    console.log(
      `  ${closed.storiesComplete}/${closed.stories} stories Complete on the board — the outcome is yours to write; this is the count at close.`,
    )
    return
  }

  usage()
}

/** `open 101 Sprint 101` without quotes still reads as a title. */
function titleFromRemainder(): string | null {
  const words = rest.slice(1).filter((w) => !w.startsWith('--'))
  return words.length > 0 ? words.join(' ') : null
}

main().catch((error) => {
  if (error instanceof PortalWriteError) {
    console.error(error.message)
    process.exit(1)
  }
  console.error(String(error instanceof Error ? error.message : error))
  process.exit(1)
})
