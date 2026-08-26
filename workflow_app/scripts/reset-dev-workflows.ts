// ---------------------------------------------------------------------------
// DEV workflow reset command (CRM-14M).
//
// Usage (DEV only):
//
//   node_modules/.bin/tsx workflow_app/scripts/reset-dev-workflows.ts --dry-run
//   node_modules/.bin/tsx workflow_app/scripts/reset-dev-workflows.ts --yes
//
// Resets workflow ENGINE runtime state (instances, tokens, engine tasks, jobs,
// events, commands, correlations, command receipts, and the workflow-
// materialized canonical tasks) to a clean slate. process_definitions and all
// other canonical application data are preserved.
//
// Hard guard: refuses unless APP_ENV=development.
// ---------------------------------------------------------------------------

import { assertDevResetAllowed, resetDevWorkflowsCore, RESET_STEPS } from '../reset'
import { listDefinitions, listInstances, listCommandReceipts } from '../diagnostics'

async function main(): Promise<void> {
  assertDevResetAllowed(process.env.APP_ENV)

  const args = process.argv.slice(2)
  const dryRun = args.includes('--dry-run')
  const yes = args.includes('--yes')

  if (!dryRun && !yes) {
    console.error(
      'Usage: tsx workflow_app/scripts/reset-dev-workflows.ts [--dry-run | --yes]',
    )
    process.exit(1)
  }

  if (dryRun) {
    const [defs, instances, receipts] = await Promise.all([
      listDefinitions(),
      listInstances(),
      listCommandReceipts(),
    ])
    console.log('DRY RUN — the following workflow state WOULD be reset:')
    console.log(`  definitions (preserved): ${defs.length}`)
    console.log(`  instances  (deleted):    ${instances.length}`)
    console.log(`  command receipts (deleted): ${receipts.length}`)
    console.log('\nOrdered delete steps:')
    for (const step of RESET_STEPS) console.log(`  - ${step.table}`)
    console.log('\nNo database write performed.')
    return
  }

  const { neon } = await import('@neondatabase/serverless')
  const { getDatabaseUrl } = await import('../../db/client')
  const exec = (s: string) =>
    neon(getDatabaseUrl()).unsafe(s) as unknown as Promise<unknown[]>
  const results = await resetDevWorkflowsCore(exec)
  console.log('DEV workflow reset complete:')
  for (const r of results) {
    console.log(`  ${r.table}: ${r.deleted} row(s) deleted`)
  }
}

main().catch((err) => {
  console.error('Reset failed:', err)
  process.exit(1)
})
