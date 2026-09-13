#!/usr/bin/env node
// ---------------------------------------------------------------------------
// THE CHAINSAW — what to reach for when something is already past the fence.
//
// Database ceilings stop the NEXT zombie. They do nothing about one already
// running: Neon kills an idle-in-transaction session, PgBouncer kills a pool
// wait, but a query that is actually EXECUTING runs until someone stops it.
// This is that someone.
//
// Usage:
//   node --import tsx --env-file=.env.local scripts/db-zombies.mjs
//        list long-running work (default; read-only)
//   ... --min-seconds 30        only show work older than 30s
//   ... --cancel <pid>          polite: cancel the query, session survives
//   ... --terminate <pid>       chainsaw: kill the backend connection
//
// ENVIRONMENT IS NOT A CHOICE: the pool manager decides it, and this tool refuses
// to act unless PROD is declared — the same rule as every other Forge ops tool.
// `list` is always safe and is allowed anywhere.
// ---------------------------------------------------------------------------
import { forgeDbPool } from '../db/forge-db.ts'

const args = process.argv.slice(2)
const flag = (name) => {
  const i = args.indexOf(`--${name}`)
  return i >= 0 ? args[i + 1] ?? null : null
}
const minSeconds = Number.parseInt(flag('min-seconds') ?? '5', 10) || 5
const cancelPid = Number.parseInt(flag('cancel') ?? '', 10)
const terminatePid = Number.parseInt(flag('terminate') ?? '', 10)
const acting = Number.isFinite(cancelPid) || Number.isFinite(terminatePid)

if (acting && (process.env.APP_ENV ?? 'development') !== 'production') {
  console.error('db-zombies: refusing to cancel/terminate without APP_ENV=production declared.')
  process.exit(2)
}

const pool = forgeDbPool()

const rows = await pool.query(
  `
  select pid, usename as usr, state, wait_event_type, wait_event,
         now() - query_start as duration,
         round(extract(epoch from (now() - query_start)))::int as seconds,
         left(regexp_replace(coalesce(query, ''), '\\s+', ' ', 'g'), 110) as query
  from pg_stat_activity
  where state <> 'idle'
    and pid <> pg_backend_pid()
    and query_start is not null
    and extract(epoch from (now() - query_start)) >= $1
  order by query_start
  `,
  [minSeconds],
)

const active = rows.rows.filter((r) => r.state !== 'idle')
console.log(`long-running work (>= ${minSeconds}s): ${active.length}`)
for (const r of active) {
  console.log(
    `- pid=${r.pid} ${r.seconds}s state=${r.state} wait=${r.wait_event_type ?? '-'}/${r.wait_event ?? '-'} :: ${r.query}`,
  )
}

if (Number.isFinite(cancelPid)) {
  const r = await pool.query('select pg_cancel_backend($1) as ok', [cancelPid])
  console.log(`pg_cancel_backend(${cancelPid}) ->`, JSON.stringify(r.rows[0]))
}
if (Number.isFinite(terminatePid)) {
  const r = await pool.query('select pg_terminate_backend($1) as ok', [terminatePid])
  console.log(`pg_terminate_backend(${terminatePid}) ->`, JSON.stringify(r.rows[0]))
}

await pool.end()
