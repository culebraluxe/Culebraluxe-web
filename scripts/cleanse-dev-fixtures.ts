// ---------------------------------------------------------------------------
// ENG-20 DEV test-data hygiene — CLI wrapper for the surgical preflight
// cleanse (db/fixture-cleanup.ts).
//
// Usage:
//   APP_ENV=development pnpm tsx scripts/cleanse-dev-fixtures.ts --yes
//
// Guard rail: refuses unless APP_ENV is development/test AND --yes is passed.
// Deletes ONLY explicitly namespaced fixture stories (TMP-* / TUNIT* / TEST-* /
// DOGFOOD-* / *-DOGFOOD-*) plus their runs and work items in safe FK order.
// Real Story Board stories and real execution history are never touched.
// ---------------------------------------------------------------------------

import { cleanseDevFixtures } from '../db/fixture-cleanup'

async function main(): Promise<void> {
  const yes = process.argv.includes('--yes')
  if (!yes) {
    console.error(
      'refusing: pass --yes to confirm this DEV-only fixture cleanse.\n' +
        'Example: APP_ENV=development node scripts/cleanse-dev-fixtures.ts --yes',
    )
    process.exit(2)
  }

  const appEnv = (process.env.APP_ENV ?? 'development').trim().toLowerCase()
  if (!(appEnv === 'development' || appEnv === 'dev' || appEnv === 'test')) {
    console.error(`refusing: cleanse is DEV/test only; APP_ENV=${JSON.stringify(process.env.APP_ENV)}`)
    process.exit(2)
  }

  const result = await cleanseDevFixtures({ appEnv })
  if (result.refused) {
    console.error('refused:', result.reason)
    process.exit(2)
  }

  console.log('=== DEV fixture cleanse ===')
  console.log('deleted stories:', result.deletedStories.length, result.deletedStories.join(', ') || '(none)')
  console.log('deleted work items:', result.deletedWorkItems)
  console.log('deleted runs:', result.deletedRuns)
  console.log('preserved story rows:', result.preservedStoryCount)
  console.log('active test-owned work items after:', result.activeFixtureCountAfter)
  console.log('fixture patterns:', 'TMP-* TUNIT* TEST-* DOGFOOD-* *-DOGFOOD-*')
}

main().catch((e) => {
  console.error(String(e).slice(0, 3000))
  process.exit(1)
})
