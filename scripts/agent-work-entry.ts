import { spawnSync } from 'node:child_process'
import { resolve } from 'node:path'

import { recoverStaleAgentWorkIndustrial } from '../db/agent-work-recovery'
import { fireDueForgeBatches } from '../db/forge-batch'
import { interactiveSql } from '../lib/neon-interactive'

function staleAfterMinutes(): number {
  const raw = process.env.AGENT_WORKER_STALE_AFTER_MINUTES ?? '10'
  const value = Number(raw)
  if (!Number.isFinite(value) || value < 0) {
    throw new Error(
      `AGENT_WORKER_STALE_AFTER_MINUTES must be a non-negative number; received ${JSON.stringify(raw)}`,
    )
  }
  return value
}

async function main(): Promise<number> {
  if ((process.env.APP_ENV ?? 'development') !== 'production') {
    throw new Error(
      'agent:work must target the PRODUCTION Forge control plane. Run with APP_ENV=production.',
    )
  }

  // Recovery is part of Forge itself, not launchd shell ceremony. Every normal
  // worker pass first heals orphaned durable runtime ownership, then runs the
  // existing claim/orchestration command. Runtime child failures are already
  // converted to Interrupted at the repository seam; this preflight covers a
  // whole worker/host disappearing between heartbeats.
  const recovered = await recoverStaleAgentWorkIndustrial(
    staleAfterMinutes(),
    interactiveSql as never,
  )
  for (const result of recovered) {
    console.log(
      `recovery ${result.workItem.id}: ${result.disposition}; ` +
        `state=${result.workItem.state}; attempts=${result.workItem.attempts}/${result.workItem.maxAttempts}; ` +
        `interrupted_run=${result.interruptedRunId ?? 'none'}`,
    )
  }

  // FIRE ANY BATCH THAT IS DUE — this is what makes "stage it and go to sleep" real.
  //
  // It lives HERE, at the top of the unattended worker pass, rather than in a second daemon or a cron
  // entry: the launchd scheduler already wakes this command every 3 minutes, so a batch scheduled for
  // 02:00 fires on the first pass after 02:00 with nobody awake and nothing else to remember. This
  // also means the night run needs no redeploy of the launchd wrapper.
  const dueBatches = await fireDueForgeBatches()
  for (const fired of dueBatches) {
    console.log(
      `batch ${fired.batchId}: queued=${fired.queued}` +
        (fired.failed.length ? ` skipped=${fired.failed.length}` : ''),
    )
  }

  const tsxCli = resolve(process.cwd(), 'node_modules/tsx/dist/cli.mjs')
  const agentWork = resolve(process.cwd(), 'scripts/agent-work.ts')
  const child = spawnSync(
    process.execPath,
    [tsxCli, agentWork, ...process.argv.slice(2)],
    {
      cwd: process.cwd(),
      env: process.env,
      stdio: 'inherit',
    },
  )

  if (child.error) throw child.error
  if (child.signal) {
    console.error(`agent:work child terminated by signal ${child.signal}`)
    return 1
  }
  return child.status ?? 1
}

main()
  .then((code) => {
    // interactiveSql owns a lazy Neon Pool. This is deliberately a bounded
    // one-shot worker command, so terminate explicitly once the child Forge
    // pass returns rather than leaving the scheduler alive on an idle pool.
    process.exit(code)
  })
  .catch((error) => {
    console.error(String((error as Error)?.stack ?? error))
    process.exit(1)
  })
