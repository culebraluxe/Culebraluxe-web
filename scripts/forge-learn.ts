// ---------------------------------------------------------------------------
// forge:learn — run ONE learn pass by hand.
//
//   pnpm forge:learn                     # dry run against DEV: what a pass would file
//   pnpm forge:learn --apply             # file it (DEV unless APP_ENV=production + --prod)
//   pnpm forge:learn --window-hours 6 --format json
//
// The unattended worker calls `runLearnPass` directly (this is the same function), so this command exists
// for the two things a worker cannot do: show a human what the loop WOULD file before it files it, and let
// a probe exercise the real path on a real database.
//
// Why the PROD guard: filing a learn item creates a story and either stages it or opens a Ready work item.
// On PROD that is real work for the live worker, which is exactly what the night run is supposed to do — so
// the guard is on the CLI (a human at a keyboard), not on the loop (the thing that is meant to do it).
// ---------------------------------------------------------------------------

import { describeControlPlane } from '@/lib/execution-target'
import { runLearnPass } from '../agent-runtime/learn-loop'

export function parseArgs(argv: string[]): { apply: boolean; prod: boolean; json: boolean; windowHours?: number } {
  const options = { apply: false, prod: false, json: false, windowHours: undefined as number | undefined }
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]
    if (arg === '--apply') options.apply = true
    else if (arg === '--prod') options.prod = true
    else if (arg === '--format') options.json = argv[i + 1] === 'json'
    else if (arg === '--window-hours') options.windowHours = Number(argv[i + 1])
  }
  return options
}

async function main(): Promise<number> {
  const options = parseArgs(process.argv.slice(2))
  const plane = describeControlPlane()
  console.log(
    `forge:learn — target APP_ENV=${plane.appEnv} → ${plane.target}, ${options.apply ? 'APPLY' : 'DRY RUN'}`,
  )

  if (options.apply && plane.target === 'prod' && !options.prod) {
    console.error('REFUSING: --apply against PROD creates real stories and work items for the live worker.')
    console.error('          The unattended pass does this on purpose at night. From a keyboard, re-run with --prod.')
    return 1
  }

  const result = await runLearnPass({
    root: process.cwd(),
    apply: options.apply,
    ...(options.windowHours !== undefined ? { windowHours: options.windowHours } : {}),
  })

  if (options.json) {
    console.log(JSON.stringify(result, null, 2))
    return 0
  }

  console.log(`  window      ${result.windowStart} → ${result.windowEnd}`)
  console.log(`  files       ${result.filesScanned} changed code file(s) scanned`)
  console.log(`  candidates  ${result.candidates}`)
  if (result.filed) {
    console.log(
      `  FILED       ${result.filed.storyId} (${result.filed.key}, ${result.filed.severity}, via ${result.filed.via})` +
        `${result.filed.packetWritten ? ' + packet written' : ' + packet already current'}`,
    )
  } else if (result.wouldFile) {
    console.log(
      `  would file  ${result.wouldFile.key} (${result.wouldFile.severity}, ${result.wouldFile.hitCount} hit(s))` +
        ' — dry run, nothing written',
    )
  } else {
    console.log('  filed       nothing (no candidate needs an item)')
  }
  if (result.skipped.length) console.log(`  skipped     ${result.skipped.join(', ')} (already open)`)
  if (result.deferred.length) console.log(`  deferred    ${result.deferred.join(', ')} (cap is one per pass)`)
  if (!result.applied && result.wouldFile) console.log('\n  re-run with --apply to file it.')
  return 0
}

if (process.argv[1] && /(^|\/)forge-learn\.ts$/.test(process.argv[1])) {
  void main().then((code) => process.exit(code))
}
