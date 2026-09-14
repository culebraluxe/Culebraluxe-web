// Environment topology probe: WHERE does engine history actually live, and does the screen
// the operator looks at read the same database the engine writes?
//
// Prints only host + database name (never credentials), then counts the engine's own tables per
// distinct target. Two labels resolving to the same host+db is the "parallel dimension" bug.
import { readFileSync } from 'node:fs'
import { Pool } from 'pg'

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

function describe(url: string): { host: string; db: string } {
  try {
    const parsed = new URL(url)
    return { host: parsed.hostname, db: parsed.pathname.replace(/^\//, '') || '(default)' }
  } catch {
    return { host: '(unparseable)', db: '(unparseable)' }
  }
}

async function count(pool: Pool, table: string): Promise<string> {
  try {
    const res = await pool.query(`select count(*)::int as n from ${table}`)
    return String((res.rows[0] as { n?: unknown })?.n ?? '?')
  } catch (err) {
    return `(n/a: ${(err as { code?: string }).code ?? 'error'})`
  }
}

async function main(): Promise<void> {
  const targets = loadTargets()
  const seen = new Map<string, string[]>()

  for (const target of targets) {
    const place = describe(target.url)
    const key = `${place.host}/${place.db}`
    seen.set(key, [...(seen.get(key) ?? []), target.label])
  }

  console.log('=== DB TARGET IDENTITY (host + database only) ===')
  for (const [key, labels] of seen) {
    console.log(`${key}  <-  ${labels.join(', ')}`)
  }
  if (seen.size === 1) {
    console.log('VERDICT: every target resolves to ONE database. DEV and PROD are the same Neon DB.')
  }

  for (const target of targets) {
    const place = describe(target.url)
    const pool = new Pool({ connectionString: target.url, max: 2 })
    try {
      console.log(`\n=== ${target.label} -> ${place.host}/${place.db} ===`)
      for (const table of [
        'process_instances',
        'process_events',
        'process_definitions',
        'story',
        'app_error',
      ]) {
        console.log(`${table}: ${await count(pool, table)}`)
      }
      const latest = await pool.query(
        'select max(created_at)::text as latest from process_instances',
      )
      console.log(`process_instances.latest_created_at: ${String((latest.rows[0] as { latest?: unknown })?.latest ?? '(none)')}`)
    } catch (err) {
      console.log(`${target.label}: connection failed (${(err as Error).message})`)
    } finally {
      await pool.end().catch(() => undefined)
    }
  }
}

main().catch((err) => {
  console.error('probe failed:', err)
  process.exitCode = 1
})
