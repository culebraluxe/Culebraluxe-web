// ---------------------------------------------------------------------------
// ENG-FORGE-OBS-SERIAL-01 probe — proves the observer seam can actually write.
//
// WHY THIS EXISTS. After the every-attempt hook landed (57d537e) the PROD trace
// table still held ZERO `forge_observer` rows: 174 rows, all `workflow_runtime`.
// `createPersistentTraceSink` swallows write failures by design ("a recorder that
// can fail a run is not an observer"), so a sink that writes nothing looks
// EXACTLY like a sink with no runs to record. This probe removes that ambiguity
// by pushing one real serial-candidate observation through the real seam into
// `workflow_execution_trace_event`.
//
// It also satisfies FORGE-OBS-SERIAL-01's last acceptance box: the scorecard's
// OBSERVER LAYER was empty and had to become non-empty from a real write rather
// than from a test double.
//
// ENVIRONMENT. `recordTraceEvent` resolves its target from APP_ENV and DEFAULTS
// TO DEV, so this prints the resolved target and refuses to write unless it is
// PROD — the scorecard reads PROD, and a probe that writes to DEV would prove
// nothing about the lane that runs.
//
// Read-only by default. `--apply` performs ONE idempotent write: the sink's
// source_event_id is deterministic and the table de-dupes on
// (source_system, source_event_id), so a re-run is a no-op. The synthetic story
// id makes the row unmistakable in the trace.
//
//   APP_ENV=production node --env-file=.env.local --import tsx scripts/probe-forge-observer.ts
//   APP_ENV=production node --env-file=.env.local --import tsx scripts/probe-forge-observer.ts --apply
// ---------------------------------------------------------------------------

import { resolveDbTarget, sql } from '../db/client'
import { recordTraceEvent } from '../db/workflow-trace'
import { createPersistentTraceSink, type TraceWrite } from '../workflow_app/forge/forge-observer'
import {
  drainAlerts,
  observeAttemptBegin,
  observeCandidateCommit,
  observeHold,
  observeMeasurementGap,
} from '../workflow_app/forge/forge-observer-seam'
import type { SmithExecutionContract } from '../workflow_app/forge/smith-contract'

const APPLY = process.argv.includes('--apply')
const CLEANUP = process.argv.includes('--cleanup')
const PROBE_STORY = 'FORGE-OBSERVER-PROBE'
const SHA = 'f'.repeat(40)
const ALLOWED = 'workflow_app/forge/agent-runtime-role-runner.ts'
const OUTSIDE = 'app/portal/projects/page.tsx'

const pinned: Parameters<TraceWrite>[0][] = []
const planOnly: TraceWrite = async (input) => {
  pinned.push(input)
}
const write: TraceWrite = APPLY ? (input) => recordTraceEvent(input as never) : planOnly

const base = {
  storyId: PROBE_STORY,
  processInstanceId: 'probe-process',
  taskId: 'probe-task',
  nodeId: 'smith',
  attempt: 1,
  worktreePath: process.cwd(),
  baseCommit: 'a'.repeat(40),
}

const contract: SmithExecutionContract = {
  identity: { storyId: PROBE_STORY, nodeId: 'smith', attempt: 1, owner: 'probe-assignment' },
  objective: 'prove the serial observer seam reaches PROD',
  requiredInputs: [],
  allowedScope: [ALLOWED],
  prohibitedScope: [],
  expectedOutputs: [],
  requiredEvidence: [],
  dependsOn: [],
}

async function main(): Promise<void> {
  const target = resolveDbTarget()
  console.log(`resolved db target: ${target.toUpperCase()}   mode: ${APPLY ? 'APPLY' : 'DRY RUN'}`)
  if (target !== 'prod') {
    console.error('FAIL CLOSED: this probe must write to PROD; set APP_ENV=production.')
    process.exit(1)
  }

  // The counterpart to --apply, so "keep the rows or clean them up" is a one-line
  // decision instead of a hand-written DELETE. Removes ONLY this probe's rows: the
  // synthetic story id prefix is the filter, so no real execution evidence is ever
  // touched. Once a real lane has run, its own observer events make these redundant
  // and this is the command that retires them.
  if (CLEANUP) {
    const probePrefix = `${PROBE_STORY}:%`
    const before = (await sql`
      select count(*)::int as n from workflow_execution_trace_event
       where source_system = 'forge_observer' and source_event_id like ${probePrefix}
    `) as unknown as Array<{ n: number }>
    const existing = before[0]?.n ?? 0
    // Same dry-run discipline as the write path: --cleanup alone only reports.
    if (!APPLY) {
      console.log(
        `\nDRY RUN: would remove ${existing} probe rows where source_event_id like ${probePrefix}. ` +
          'Re-run with --cleanup --apply.',
      )
      return
    }
    const deleted = await sql`
      delete from workflow_execution_trace_event
       where source_system = 'forge_observer' and source_event_id like ${probePrefix}
      returning id
    `
    console.log(`\nprobe rows before: ${existing}   deleted: ${deleted.length}`)
    const after = (await sql`
      select count(*)::int as n from workflow_execution_trace_event
       where source_system = 'forge_observer'
    `) as unknown as Array<{ n: number }>
    console.log(`forge_observer rows remaining: ${after[0]?.n ?? 0}`)
    return
  }

  const sink = createPersistentTraceSink({ write, traceId: 'probe-trace' })

  // Exactly the serial-lane hook: attempt begin, the candidate's commit and scope
  // verdict, a HOLD, and the alert drain.
  observeAttemptBegin(sink, base, { role: 'smith', route: 'SMITH' })
  observeCandidateCommit(sink, base, {
    candidateSha: SHA,
    changedFiles: [ALLOWED, OUTSIDE],
    contract,
  })
  observeHold(sink, base, {
    reasons: ['probe: candidate left its assignment'],
    sha: SHA,
    missReasons: ['probe: candidate left its assignment'],
  })
  drainAlerts(sink, base, PROBE_STORY)
  observeMeasurementGap(sink, base, { what: 'SERIAL_SCOPE', reason: 'probe: measurement gap is recorded, not swallowed' })
  await new Promise((resolve) => setTimeout(resolve, 400))

  console.log(`\nevents emitted: ${sink.snapshot().length}`)
  for (const e of sink.snapshot()) console.log(`  ${e.kind.padEnd(12)} seq=${e.seq}`)

  if (!APPLY) {
    console.log(`\nwould write ${pinned.length} rows as source_system=forge_observer:`)
    for (const row of pinned) console.log(`  ${row.eventType.padEnd(24)} ${row.sourceEventId}`)
    console.log('\nDRY RUN — nothing written. Re-run with --apply.')
    return
  }

  const rows = (await sql`
    select event_type, count(*)::int as events
      from workflow_execution_trace_event
     where source_system = 'forge_observer'
     group by 1 order by 2 desc
  `) as unknown as Array<{ event_type: string; events: number }>
  console.log('\nforge_observer rows now in the trace table:')
  for (const r of rows) console.log(`  ${r.event_type.padEnd(24)} ${r.events}`)
  console.log(`\nAPPLIED. The scorecard's OBSERVER LAYER reads this table.`)
}

void main().then(
  () => process.exit(0),
  (error) => {
    console.error(error)
    process.exit(1)
  },
)
