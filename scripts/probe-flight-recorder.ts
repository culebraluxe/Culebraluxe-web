// ONE Flight Recorder diagnostic, on the SANCTIONED database seam.
//
// The first version of this tool imported `pg` and parsed `.env.local` by hand, which broke the
// repository's DB boundary invariant (`legacy/workflow_app/tests/db-boundary.test.ts`: only ForgeDB may
// import the driver) and defeated the fail-closed environment declaration at the same time — it
// would happily compare databases without ever saying which one the machine believed it was in.
// Everything here goes through `forgeDbForTarget` / `dbTargetInfo` instead.
//
// What it answers, in the order the diagnostic ladder in
// `docs/agent/WORKFLOW-ARCHITECTURE.md` (Part IX) asks:
//   1. which environment is declared, and is each target actually configured
//   2. how much engine history each target holds
//   3. is the error store noisy (a route capturing many rows per second is a LOOP, not a failure)
//   4. does the read model still work against real instances, and how fast
//   5. does a malformed id fail cleanly instead of reaching the driver
//
// Run:  APP_ENV=production node --env-file=.env.local --import tsx scripts/probe-flight-recorder.ts
import { dbTargetInfo } from '@/legacy/db/database-gateway'
import { forgeDbForTarget, type ForgeDbTarget } from '@/legacy/db/forge-db'
import { getFlightRecorderTransaction } from '@/legacy/workflow_app/flight-recorder-read'

async function targetCounts(target: ForgeDbTarget): Promise<void> {
  let handle: ReturnType<typeof forgeDbForTarget>
  try {
    handle = forgeDbForTarget(target)
  } catch (err) {
    console.log(`${target}: not configured (${(err as Error).message.slice(0, 120)})`)
    return
  }
  try {
    const counts: string[] = []
    for (const table of ['process_instances', 'process_events', 'app_error']) {
      const rows = await handle.runText(`select count(*)::int as n from ${table}`)
      counts.push(`${table}=${String((rows[0] as { n?: unknown })?.n ?? '?')}`)
    }
    const latest = await handle.runText(
      'select max(created_at)::text as latest from process_instances',
    )
    console.log(`${target}: ${counts.join(' ')} newest=${String((latest[0] as { latest?: unknown })?.latest ?? '(none)')}`)
  } catch (err) {
    console.log(`${target}: query failed (${(err as Error).message.slice(0, 160)})`)
  } finally {
    await handle.end().catch(() => undefined)
  }
}

async function main(): Promise<void> {
  const info = dbTargetInfo()
  console.log(
    `declared: target=${info.target ?? 'UNDECLARED'} by=${info.declaredBy ?? 'nothing'} ` +
      `branch=${info.neonBranch ?? '?'}${info.undeclaredReason ? ` (${info.undeclaredReason})` : ''}`,
  )

  console.log('\n--- 1. engine history per target ---')
  for (const target of ['dev', 'prod'] as ForgeDbTarget[]) await targetCounts(target)

  const active = info.target ?? 'dev'
  const handle = forgeDbForTarget(active)
  try {
    console.log(`\n--- 2. error noise, last 2h (${active}) ---`)
    const noise = await handle.runText(
      `select coalesce(route, '(null)') as route, count(*)::int as n,
              min(created_at)::text as first_seen, max(created_at)::text as last_seen
         from app_error
        where created_at > now() - interval '2 hours'
        group by route order by n desc limit 6`,
    )
    for (const row of noise) {
      const r = row as { route?: unknown; n?: unknown; first_seen?: unknown; last_seen?: unknown }
      console.log(`${String(r.n)}  ${String(r.route)}  ${String(r.first_seen)} -> ${String(r.last_seen)}`)
    }

    console.log(`\n--- 3. the read model, against real instances (${active}) ---`)
    const newest = await handle.runText(
      `select id::text, subject_type, created_at::text
         from process_instances order by created_at ${process.env.PROBE_ORDER === 'asc' ? 'asc' : 'desc'} limit 4`,
    )
    for (const inst of newest) {
      const i = inst as { id?: unknown; subject_type?: unknown; created_at?: unknown }
      const started = Date.now()
      try {
        const tx = await getFlightRecorderTransaction(String(i.id))
        console.log(
          `OK   ${String(i.id)} (${String(i.subject_type)}) ${Date.now() - started}ms ` +
            `workflows=${tx?.workflows?.length ?? 0} events=${tx?.events?.length ?? 0}`,
        )
      } catch (err) {
        const e = err as { code?: string; name?: string; message?: string }
        console.log(
          `FAIL ${String(i.id)} ${Date.now() - started}ms ${e.name ?? 'Error'}/${e.code ?? '-'}: ${String(e.message ?? '').slice(0, 200)}`,
        )
      }
    }
  } finally {
    await handle.end().catch(() => undefined)
  }

  console.log('\n--- 4. a malformed id must not reach the driver ---')
  for (const bad of ['ENG-FORGE-TURN-BUDGET-01', 'undefined', '', 'FORGE-SMITH-DOOR-01']) {
    try {
      const tx = await getFlightRecorderTransaction(bad)
      console.log(`${JSON.stringify(bad)} -> ${tx ? 'FOUND' : 'null (route answers 400 before this)'}`)
    } catch (err) {
      const e = err as { code?: string; message?: string }
      console.log(`${JSON.stringify(bad)} -> THREW ${e.code ?? ''}: ${String(e.message ?? '').slice(0, 120)}`)
    }
  }
}

main().catch((err) => {
  console.error('probe failed:', err)
  process.exitCode = 1
})
