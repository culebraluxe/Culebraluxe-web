// ---------------------------------------------------------------------------
// forge:roi — the thin session rollup in a terminal (Phase 4, Object 4).
//
//   pnpm forge:roi               # last 7 days, by kind
//   pnpm forge:roi --days 30 --format json
//
// READ-ONLY: one query, no writes, no guard needed beyond the environment it reads. It exists because the
// cockpit strip is only visible to someone who has the page open, and because "what did the night batch
// actually cost" is a question asked from a shell at 2am.
//
// It prints WIDGETS and says so. There is no widgets-to-dollars rate in this system, and inventing one in a
// terminal would be worse than inventing one on screen: a number in a log gets quoted later.
// ---------------------------------------------------------------------------

import { ROI_DEFAULT_WINDOW_DAYS, listRoiAttempts } from '@/legacy/db/forge-roi'
import { describeRoiRow, summarizeRoi } from '@/lib/forge-roi'
import { describeControlPlane } from '@/lib/execution-target'

export function parseArgs(argv: string[]): { days: number; json: boolean } {
  let days = ROI_DEFAULT_WINDOW_DAYS
  let json = false
  for (let i = 0; i < argv.length; i += 1) {
    if (argv[i] === '--days') days = Number(argv[i + 1])
    else if (argv[i] === '--format') json = argv[i + 1] === 'json'
  }
  return { days, json }
}

async function main(): Promise<number> {
  const options = parseArgs(process.argv.slice(2))
  const plane = describeControlPlane()
  const attempts = await listRoiAttempts(options.days)
  const summary = summarizeRoi(attempts, options.days)

  if (options.json) {
    console.log(JSON.stringify({ target: plane.target, ...summary }, null, 2))
    return 0
  }

  console.log(`forge:roi — last ${summary.windowDays} days (APP_ENV=${plane.appEnv} → ${plane.target})`)
  console.log(`  unit: ${summary.unit}`)
  if (summary.rows.length === 0) {
    console.log('  no finished attempts in the window')
    return 0
  }
  for (const row of summary.rows) console.log(`  ${describeRoiRow(row)}`)
  console.log(
    `\n  totals: ${summary.totals.attempts} attempt(s) · ${summary.totals.completed} done · ` +
      `${summary.totals.failed} failed · ${summary.totals.costWidgets} widgets`,
  )
  console.log(
    `  coverage: cost captured on ${summary.coverage.costKnown}/${summary.coverage.attempts}` +
      ` · wall time on ${summary.coverage.wallTimeKnown}/${summary.coverage.attempts}`,
  )
  console.log(`  ${summary.note}`)
  return 0
}

if (process.argv[1] && /(^|\/)forge-roi\.ts$/.test(process.argv[1])) {
  void main().then((code) => process.exit(code))
}
