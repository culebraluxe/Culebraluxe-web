import { recoverAgentWorkInterruption, recoverStaleAgentWorkIndustrial } from '../db/agent-work-recovery'
import { getAgentWorkItem } from '../db/agent-work'
import { interactiveSql } from '../lib/neon-interactive'

function value(args: string[], flag: string): string | undefined {
  const i = args.indexOf(flag)
  return i >= 0 ? args[i + 1] : undefined
}

async function main(): Promise<void> {
  if ((process.env.APP_ENV ?? 'development') !== 'production') {
    throw new Error(
      'forge-runtime-recover must target the PRODUCTION Forge control plane. Run with APP_ENV=production.',
    )
  }

  const args = process.argv.slice(2)
  const workItemId = value(args, '--work-item')
  const reason = value(args, '--reason') ?? 'operator-forced runtime recovery'

  if (workItemId) {
    const before = await getAgentWorkItem(workItemId, interactiveSql as never)
    if (!before) throw new Error(`work item ${workItemId} not found`)
    if (!['Claimed', 'Running', 'Paused'].includes(before.state)) {
      throw new Error(
        `work item ${workItemId} is ${before.state}; forced recovery requires Claimed, Running, or Paused`,
      )
    }
    const result = await recoverAgentWorkInterruption(
      workItemId,
      reason,
      interactiveSql as never,
    )
    console.log(
      `recovered ${result.workItem.id}: ${result.disposition}; ` +
        `state=${result.workItem.state}; attempts=${result.workItem.attempts}/${result.workItem.maxAttempts}; ` +
        `interrupted_run=${result.interruptedRunId ?? 'none'}`,
    )
    return
  }

  const staleText = value(args, '--stale-after')
  const staleAfterMinutes = staleText === undefined
    ? Number(process.env.AGENT_WORKER_STALE_AFTER_MINUTES ?? 60)
    : Number(staleText)
  if (!Number.isFinite(staleAfterMinutes) || staleAfterMinutes < 0) {
    throw new Error('--stale-after must be a non-negative number of minutes')
  }

  const results = await recoverStaleAgentWorkIndustrial(
    staleAfterMinutes,
    interactiveSql as never,
  )
  if (results.length === 0) {
    console.log('no stale runtime work to recover')
    return
  }
  for (const result of results) {
    console.log(
      `recovered ${result.workItem.id}: ${result.disposition}; ` +
        `state=${result.workItem.state}; attempts=${result.workItem.attempts}/${result.workItem.maxAttempts}; ` +
        `interrupted_run=${result.interruptedRunId ?? 'none'}`,
    )
  }
}

// This is a one-shot scheduler helper, not a long-lived service. interactiveSql
// owns a lazy Neon Pool whose open sockets intentionally keep normal server
// processes alive. Once recovery's awaited database work and console output are
// complete, terminate this helper explicitly so launchd can continue to the
// claim phase instead of hanging on an idle Pool.
main().then(
  () => process.exit(0),
  (error) => {
    console.error(String((error as Error)?.stack ?? error))
    process.exit(1)
  },
)
