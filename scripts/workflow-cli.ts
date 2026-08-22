// ---------------------------------------------------------------------------
// ENG-07 — canonical DEV/operator lifecycle command.
//
// One boring entry point that delegates 1:1 to existing typed seams. No new
// workflow logic lives here; no daemon, no scheduler/cron manager, no process
// supervisor, no web UI. Deployment/execution stays with the agent loop
// (ENG-06) and any future Vercel Cron.
//
//   pnpm workflow status                 read-only diagnostics snapshot + anomaly count
//   pnpm workflow reconcile              reconcileWorkflows (start-missing + materialize)
//   pnpm workflow reclaim [batch]        reclaim stale job leases (CRM-14F); default batch 20
//   pnpm workflow poll [worker] [batch]  one bounded runDueJobs pass (CRM-14F);
//                                        default worker "workflow-cli", batch 10
//   pnpm workflow test                   delegate to `pnpm test`
//   pnpm workflow test:persistence       delegate to `pnpm test:persistence` (ENG-04)
//   pnpm workflow reset:dev --yes        DEV-only workflow reset (refuses outside
//                                        APP_ENV=development; requires explicit --yes)
//
// Authority / environment awareness (all enforced by the seams, not new logic):
//   - reset:dev refuses unless APP_ENV === 'development' (assertDevResetAllowed)
//   - status is read-only (getWorkflowDiagnosticsSnapshot)
//   - reconcile / reclaim / poll are idempotent bounded passes (the seams'
//     contract; the engine never encodes an internal loop)
// ---------------------------------------------------------------------------

import { pathToFileURL } from 'node:url'
import type { WorkflowDiagnosticsSnapshot } from '../workflow_app/diagnostics'
import type { ReconcileReport } from '../workflow_app/reconcile'
import type { ResetResult } from '../workflow_app/reset'

export const DEFAULT_POLL_WORKER_ID = 'workflow-cli'
export const DEFAULT_RECLAIM_BATCH = 20
export const DEFAULT_POLL_BATCH = 10

export type PollPassResult = {
  reclaimed: number
  claimed: number
  fired: number
  completed: number
  failed: number
}

/** The existing typed seams the CLI delegates to — injectable for tests. */
export type WorkflowCliDeps = {
  status: () => Promise<WorkflowDiagnosticsSnapshot>
  reconcile: () => Promise<ReconcileReport>
  reclaimStaleJobs: (batch: number) => Promise<number>
  runDueJobsPass: (workerId: string, batch: number) => Promise<PollPassResult>
  runPnpmScript: (script: string) => Promise<number>
  resetDev: () => Promise<ResetResult[]>
}

export type WorkflowCliOutcome = { code: number; text: string }

export function usage(): string {
  return [
    'Usage: pnpm workflow <subcommand>',
    '',
    '  status               read-only diagnostics snapshot + anomaly count',
    '  reconcile            reconcile workflows (start-missing instances + materialize tasks)',
    '  reclaim [batch]      reclaim stale job leases (idempotent; default batch 20)',
    '  poll [worker] [batch]  one bounded runDueJobs pass (idempotent; default worker "workflow-cli", batch 10)',
    '  test                 delegate to `pnpm test`',
    '  test:persistence     delegate to `pnpm test:persistence`',
    '  reset:dev --yes      DEV-only workflow reset (refuses outside APP_ENV=development)',
    '',
  ].join('\n')
}

/**
 * Dispatch one invocation. Every subcommand maps 1:1 to an existing typed
 * function; this function only parses arguments and formats output.
 */
export async function runWorkflowCliCore(
  deps: WorkflowCliDeps,
  argv: string[],
): Promise<WorkflowCliOutcome> {
  const [subcommand, ...rest] = argv

  switch (subcommand) {
    case 'status': {
      const snapshot = await deps.status()
      return { code: 0, text: formatStatus(snapshot) }
    }
    case 'reconcile': {
      const report = await deps.reconcile()
      return { code: 0, text: formatReconcile(report) }
    }
    case 'reclaim': {
      const batch = parseBatch(rest[0], DEFAULT_RECLAIM_BATCH)
      if (batch === null) return { code: 1, text: usage() }
      const reclaimed = await deps.reclaimStaleJobs(batch)
      return { code: 0, text: `reclaimed ${reclaimed} stale job lease(s)\n` }
    }
    case 'poll': {
      const workerId = rest[0] ?? DEFAULT_POLL_WORKER_ID
      const batch = parseBatch(rest[1], DEFAULT_POLL_BATCH)
      if (batch === null) return { code: 1, text: usage() }
      const pass = await deps.runDueJobsPass(workerId, batch)
      return { code: 0, text: formatPoll(pass, workerId, batch) }
    }
    case 'test':
    case 'test:persistence': {
      const script = subcommand === 'test' ? 'test' : 'test:persistence'
      const code = await deps.runPnpmScript(script)
      return { code, text: '' }
    }
    case 'reset:dev': {
      if (!rest.includes('--yes')) {
        return {
          code: 1,
          text: 'reset:dev is destructive and DEV-only. Re-run with --yes to confirm.\n',
        }
      }
      const results = await deps.resetDev()
      return { code: 0, text: formatReset(results) }
    }
    default:
      return { code: 1, text: usage() }
  }
}

function parseBatch(raw: string | undefined, fallback: number): number | null {
  if (raw === undefined) return fallback
  const n = Number(raw)
  if (!Number.isInteger(n) || n <= 0) return null
  return n
}

function formatStatus(snapshot: WorkflowDiagnosticsSnapshot): string {
  const s = snapshot.summary
  const lines = [
    'workflow status (read-only snapshot):',
    `  configured: ${snapshot.configured}`,
    `  definitions: ${s.definitionCount}`,
    `  instances: ${s.instanceTotal} (${s.instanceActive} active, ${s.instanceCompleted} completed, ${s.instanceFailed} failed, ${s.instanceOther} other)`,
    `  engine tasks ready: ${s.readyEngineTasks}`,
    `  correlated open canonical tasks: ${s.correlatedOpenCanonicalTasks}`,
    `  pending jobs: ${s.pendingJobs}`,
    `  pending command receipts: ${s.pendingReceipts}`,
    `  anomalies: ${s.anomalyCount}`,
  ]
  for (const a of snapshot.anomalies) {
    lines.push(
      `    - [${a.severity}] ${a.kind}${a.instanceId ? ` (instance ${a.instanceId})` : ''}: ${a.message}`,
    )
  }
  return lines.join('\n') + '\n'
}

function formatReconcile(report: ReconcileReport): string {
  return (
    [
      'reconcile pass:',
      `  started instances: ${report.startedInstances}`,
      `  materialized tasks: ${report.materializedTasks}`,
      `  skipped tasks: ${report.skippedTasks}`,
    ].join('\n') + '\n'
  )
}

function formatPoll(pass: PollPassResult, workerId: string, batch: number): string {
  return (
    [
      `poll pass (worker ${workerId}, batch ${batch}):`,
      `  reclaimed stale leases: ${pass.reclaimed}`,
      `  claimed jobs: ${pass.claimed}`,
      `  fired timers: ${pass.fired}`,
      `  completed jobs: ${pass.completed}`,
      `  failed jobs: ${pass.failed}`,
    ].join('\n') + '\n'
  )
}

function formatReset(results: ResetResult[]): string {
  const lines = ['DEV workflow reset complete:']
  for (const r of results) lines.push(`  ${r.table}: ${r.deleted} row(s) deleted`)
  return lines.join('\n') + '\n'
}

// ---------------------------------------------------------------------------
// Real wiring — each dep is exactly one existing typed function.
// ---------------------------------------------------------------------------

async function engineHandle() {
  const { WorkflowEngine } = await import('../workflow_engine/lib/workflow/engine')
  const { createApplicationPort } = await import('../workflow_app/application-port')
  const { engineSql } = await import('../workflow_app/engine-client')
  return new WorkflowEngine(engineSql(), { app: createApplicationPort() })
}

function createDeps(): WorkflowCliDeps {
  return {
    status: async () => {
      const { getWorkflowDiagnosticsSnapshot } = await import('../workflow_app/diagnostics')
      return getWorkflowDiagnosticsSnapshot()
    },
    reconcile: async () => {
      const { reconcileWorkflows } = await import('../workflow_app/reconcile')
      return reconcileWorkflows()
    },
    reclaimStaleJobs: async (batch) => {
      const engine = await engineHandle()
      return engine.reclaimStaleJobs(batch)
    },
    runDueJobsPass: async (workerId, batch) => {
      const engine = await engineHandle()
      const pass = await engine.runDueJobs(workerId, batch)
      return {
        reclaimed: pass.reclaimed,
        claimed: pass.claimed.length,
        fired: pass.fired,
        completed: pass.completed,
        failed: pass.failed,
      }
    },
    runPnpmScript: async (script) => {
      const { spawn } = await import('node:child_process')
      return await new Promise<number>((resolve, reject) => {
        const child = spawn('pnpm', [script], {
          stdio: 'inherit',
          shell: process.platform === 'win32',
        })
        child.on('error', reject)
        child.on('exit', (code) => resolve(code ?? 1))
      })
    },
    resetDev: async () => {
      const { assertDevResetAllowed, resetDevWorkflowsCore } = await import('../workflow_app/reset')
      assertDevResetAllowed(process.env.APP_ENV)
      const { sql } = await import('../db/client')
      const exec = (s: string) => sql.unsafe(s) as unknown as Promise<unknown[]>
      return resetDevWorkflowsCore(exec)
    },
  }
}

async function main(): Promise<void> {
  const outcome = await runWorkflowCliCore(createDeps(), process.argv.slice(2))
  if (outcome.text) process.stdout.write(outcome.text)
  process.exitCode = outcome.code
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((err) => {
    console.error(`workflow: ${String(err instanceof Error ? err.message : err)}`)
    process.exit(1)
  })
}
