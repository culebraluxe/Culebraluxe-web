// What actually breaks the Flight Recorder read: try the REAL recent instances, per environment.
//
// Picks the newest process_instances and calls the same read model the route calls, so a schema
// drift (undefined_table / undefined_column) shows up as a named error instead of a 503 with a
// blank screen. Also prints the app_error noise histogram for the last hour: a route that captures
// six errors per second is a loop, not a failure.
import { Pool } from 'pg'
import { readFileSync } from 'node:fs'

type Target = { label: string; url: string }

function loadTargets(): Target[] {
  const text = readFileSync('.env.local', 'utf8')
  const out: Target[] = []
  for (const line of text.split('\n')) {
    const match = /^(DATABASE_URL(?:_DEV|_PROD)?)=(.*)$/.exec(line.trim())
    if (!match) continue
    const value = match[2].trim().replace(/^["']|["']$/g, '')
    if (value) out.push({ label: match[1], url: value })
  }
  return out
}

async function main(): Promise<void> {
  const wanted = (process.env.APP_ENV ?? 'development') === 'production' ? 'PROD' : 'DEV'
  const target = loadTargets().find((t) => t.label.endsWith(`_${wanted}`))
  if (!target) {
    console.log(`no target for ${wanted}`)
    return
  }
  console.log(`=== ${target.label} ===`)
  const pool = new Pool({ connectionString: target.url, max: 2 })
  try {
    const noise = await pool.query(
      `select coalesce(route, '(null)') as route, count(*)::int as n,
              min(created_at)::text as first_seen, max(created_at)::text as last_seen
         from app_error
        where created_at > now() - interval '2 hours'
        group by route order by n desc limit 6`,
    )
    console.log('--- app_error noise, last 2h ---')
    for (const row of noise.rows) {
      console.log(`${row.n}  ${row.route}  ${row.first_seen} -> ${row.last_seen}`)
    }

    const only = process.env.RECORDER_PROBE_ID
    if (only !== undefined) {
      const { getFlightRecorderTransaction } = await import('../workflow_app/flight-recorder-read')
      try {
        const tx = await getFlightRecorderTransaction(only)
        console.log(`id ${JSON.stringify(only)} -> ${tx ? 'FOUND' : 'null (route answers 404)'}`)
      } catch (err) {
        const e = err as { code?: string; message?: string; name?: string }
        console.log(
          `id ${JSON.stringify(only)} -> THREW ${e.name ?? 'Error'}/${e.code ?? '-'}: ${String(e.message ?? '').slice(0, 200)}` +
            '  (route answers 503)',
        )
      }
      return
    }

    const newest = await pool.query(
      `select id::text, subject_type, created_at::text
         from process_instances order by created_at ${process.env.RECORDER_PROBE_ORDER === 'asc' ? 'asc' : 'desc'} limit 4`,
    )
    console.log('--- newest instances + the read model against each ---')
    const { getFlightRecorderTransaction } = await import('../workflow_app/flight-recorder-read')
    for (const inst of newest.rows) {
      const started = Date.now()
      try {
        const tx = await getFlightRecorderTransaction(String(inst.id))
        console.log(
          `OK   ${inst.id} (${inst.subject_type}, ${inst.created_at}) ${Date.now() - started}ms ` +
            `workflows=${tx?.workflows?.length ?? 0} events=${tx?.events?.length ?? 0}`,
        )
      } catch (err) {
        const e = err as { code?: string; message?: string; name?: string }
        console.log(
          `FAIL ${inst.id} (${inst.subject_type}, ${inst.created_at}) ${Date.now() - started}ms ` +
            `${e.name ?? 'Error'}/${e.code ?? '-'}: ${String(e.message ?? '').slice(0, 260)}`,
        )
      }
    }
  } finally {
    await pool.end().catch(() => undefined)
  }
}

main().catch((err) => {
  console.error('probe failed:', err)
  process.exitCode = 1
})
