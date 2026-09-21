import { spawnSync } from 'node:child_process'
import { resolve } from 'node:path'

import { recoverStaleAgentWorkIndustrial } from '@/legacy/db/agent-work-recovery'
import { fireDueForgeBatches } from '@/legacy/db/forge-batch'
import { runLearnPass } from '../agent-runtime/learn-loop'
import { captureServerLog } from '../lib/server-error-capture'
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

  // THE LEARN LOOP, right after the batch fire (ENG-FORGE-FACTORY-01 Phase 3).
  //
  // After the night batch is on its way, the pass looks at what changed since it last looked: the
  // silent-failure hunter over the changed files, plus the stale-claim query. It may file AT MOST ONE learn
  // item, and it never files a second one for a pattern that already has an open item (a partial unique
  // index enforces that, not this code).
  //
  // It runs AFTER the fire so a learn item filed now can be staged into the batch that is about to run,
  // rather than waiting for the next one.
  //
  // NEVER FATAL. The worker was woken to run work; a learning pass that throws must not stop that. The
  // failure is captured rather than swallowed - one of the decisions in force is that a silent refusal is a
  // defect, and this is exactly the shape it warns about.
  try {
    const learned = await runLearnPass({ root: process.cwd(), apply: true })
    if (learned.filed) {
      console.log(
        `learn: filed ${learned.filed.storyId} (${learned.filed.key}, ${learned.filed.severity}, via ${learned.filed.via})`,
      )
    }
    if (learned.skipped.length) console.log(`learn: skipped ${learned.skipped.join(', ')} (already open)`)
    if (learned.deferred.length) console.log(`learn: deferred ${learned.deferred.join(', ')} (cap is one per pass)`)
  } catch (error) {
    captureServerLog('warn', 'forge-learn-pass-failed', error instanceof Error ? error.message : String(error))
  }

  // WHICH PATH RUNS THIS PASS (see workflow_app/forge/worker-dispatch.ts).
  //
  // `planForgeNight().driveEngine` used to be read by NOTHING, so a Ready story sat until a human ran
  // `pnpm forge:engine --story …` by hand — and the lane path it fell through to cannot finish a story
  // (no deliverable capture: the architect reports Complete and leaves no brief, so the Lead handoff
  // refuses). This is the flag's consumer.
  const { listAgentWorkItems } = await import('@/legacy/db/agent-work')
  const { planForgeNight } = await import('@/legacy/workflow_app/forge/forge-night-driver')
  const { chooseWorkerDispatch } = await import('@/legacy/workflow_app/forge/worker-dispatch')
  const readyItems = (await listAgentWorkItems().catch(() => null)) ?? []
  const dispatch = chooseWorkerDispatch({
    plan: planForgeNight(),
    ready: readyItems
      .filter((item) => item.state === 'Ready')
      .map((item) => ({ storyId: item.storyId, queuedAt: item.queuedAt, kind: item.kind })),
  })
  console.log(`dispatch: ${dispatch.kind} — ${dispatch.reason}`)

  if (dispatch.kind === 'engine') {
    const engineWorker = resolve(process.cwd(), 'scripts/forge-engine-worker.ts')
    const engine = spawnSync(
      process.execPath,
      ['--import', 'tsx', engineWorker, '--story', dispatch.storyId, '--work-type', dispatch.workType],
      {
        cwd: process.cwd(),
        env: process.env,
        stdio: 'inherit',
        // One bounded engine drive per pass. The engine loops roles (and its own repair cycles) inside
        // this; the bound exists so a pathological run cannot hold the scheduler forever.
        timeout: 45 * 60 * 1000,
      },
    )
    if (engine.error) throw engine.error
    if (engine.signal) {
      console.error(`forge:engine child terminated by signal ${engine.signal}`)
      return 1
    }
    return engine.status ?? 1
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
