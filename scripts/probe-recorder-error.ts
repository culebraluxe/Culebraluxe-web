// Read the CAPTURED failure behind the Flight Recorder's 503, from whichever environment holds it.
//
// The route returns 503 only after the read model THREW, and that throw is captured durably, so
// this asks the error store rather than re-deriving a theory. Run per environment:
//   APP_ENV=production node --env-file=.env.local --import tsx scripts/probe-recorder-error.ts
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
  console.log(`=== app_error rows on ${target.label} ===`)
  const pool = new Pool({ connectionString: target.url, max: 2 })
  try {
    const res = await pool.query(
      `select created_at::text, level, kind, route, operation, code, message,
              left(coalesce(stack, ''), 900) as stack_head
         from app_error
        where coalesce(route, '') ilike '%flight%'
           or coalesce(message, '') ilike '%flight%'
           or coalesce(stack, '') ilike '%flight%'
        order by created_at desc
        limit 6`,
    )
    if (res.rows.length === 0) {
      console.log('(no captured rows mentioning flight — the throw may be captured under another route)')
      const any = await pool.query(
        `select created_at::text, route, code, message
           from app_error order by created_at desc limit 6`,
      )
      for (const row of any.rows) console.log(JSON.stringify(row))
    }
    for (const row of res.rows) {
      console.log(`\n--- ${row.created_at} [${row.level}/${row.kind}] ${row.route ?? ''} ${row.code ?? ''}`)
      console.log(String(row.message ?? '').slice(0, 500))
      if (row.operation) console.log(`operation: ${row.operation}`)
      console.log(String(row.stack_head ?? '').slice(0, 900))
    }
  } finally {
    await pool.end().catch(() => undefined)
  }
}

main().catch((err) => {
  console.error('probe failed:', err)
  process.exitCode = 1
})
