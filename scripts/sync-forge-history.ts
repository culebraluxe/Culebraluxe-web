// Sync Forge history (stories, runs, work items) from DEV into PROD.
//
// Idempotent and additive: never updates or deletes anything in PROD. Safe to
// re-run any time - for example after stomping DEV. Usage: pnpm forge:sync-history
//
// Why: the Forge history for a series lived in DEV while the board lives in PROD, so
// shipping work left PROD unable to show its own numbers. This copies the missing
// rows ADDITIVELY and IDEMPOTENTLY (on conflict do nothing) - it never updates or
// deletes anything in PROD.
//
// Order matters: runs first, then work items (work items reference story_run_id).
// FK safety: a row is skipped (and reported) if a parent it needs is absent from PROD.
import { Pool } from '@neondatabase/serverless'

const dev = new Pool({ connectionString: process.env.DATABASE_URL_DEV })
const prod = new Pool({ connectionString: process.env.DATABASE_URL_PROD })

type Row = Record<string, unknown>

async function copyMissing(table: string, parentChecks: (row: Row, prodIds: Record<string, Set<string>>) => string | null) {
  const devRows = (await dev.query(`select * from ${table}`)).rows as Row[]
  const prodRows = (await prod.query(`select id from ${table}`)).rows as Array<{ id: string }>
  const prodIds: Record<string, Set<string>> = {
    [table]: new Set(prodRows.map((r) => r.id)),
  }
  // parent id sets we may need
  if (table !== 'storyboard_story_run') {
    const runs = (await prod.query('select id from storyboard_story_run')).rows as Array<{ id: string }>
    prodIds.storyboard_story_run = new Set(runs.map((r) => r.id))
  }
  const stories = (await prod.query('select id from storyboard_story')).rows as Array<{ id: string }>
  prodIds.storyboard_story = new Set(stories.map((r) => r.id))

  const missing = devRows.filter((r) => !prodIds[table].has(String(r.id)))
  console.log(`\n${table}: DEV ${devRows.length} | PROD ${prodRows.length} | missing ${missing.length}`)

  let inserted = 0
  const skipped: string[] = []
  for (const row of missing) {
    const problem = parentChecks(row, prodIds)
    if (problem) {
      skipped.push(`${row.id}: ${problem}`)
      continue
    }
    const columns = Object.keys(row)
    const placeholders = columns.map((_, i) => `$${i + 1}`).join(', ')
    try {
      await prod.query(
        `insert into ${table} (${columns.map((c) => `"${c}"`).join(', ')}) values (${placeholders}) on conflict (id) do nothing`,
        columns.map((c) => row[c]),
      )
      inserted += 1
    } catch (error) {
      skipped.push(`${row.id}: ${String((error as Error).message).slice(0, 90)}`)
    }
  }
  console.log(`  inserted ${inserted}${skipped.length ? ` | skipped ${skipped.length}` : ''}`)
  for (const s of skipped.slice(0, 8)) console.log('    skip:', s)
  return { inserted, skipped: skipped.length, missing: missing.length }
}

async function main() {
  console.log('BEFORE:')
  for (const t of ['storyboard_story_run', 'agent_work_item']) {
    const d = (await dev.query(`select count(*)::int n from ${t}`)).rows[0].n
    const p = (await prod.query(`select count(*)::int n from ${t}`)).rows[0].n
    console.log(`  ${t.padEnd(24)} DEV ${d} | PROD ${p}`)
  }

  // Parents first: a story that exists only in DEV (the dogfoods, anchors, ripwire)
  // has to land in PROD before its runs and work items can reference it.
  const stories = await copyMissing('storyboard_story', () => null)
  console.log(`\nstories: missing ${stories.missing}, inserted ${stories.inserted}, skipped ${stories.skipped}`)

  const runs = await copyMissing('storyboard_story_run', (row, ids) =>
    ids.storyboard_story.has(String(row.story_id)) ? null : `story ${row.story_id} absent from PROD`,
  )
  const items = await copyMissing('agent_work_item', (row, ids) => {
    if (!ids.storyboard_story.has(String(row.story_id))) return `story ${row.story_id} absent from PROD`
    if (row.story_run_id && !ids.storyboard_story_run.has(String(row.story_run_id))) {
      return `story run ${row.story_run_id} absent from PROD`
    }
    return null
  })

  console.log('\nAFTER:')
  for (const t of ['storyboard_story_run', 'agent_work_item']) {
    const d = (await dev.query(`select count(*)::int n from ${t}`)).rows[0].n
    const p = (await prod.query(`select count(*)::int n from ${t}`)).rows[0].n
    console.log(`  ${t.padEnd(24)} DEV ${d} | PROD ${p}`)
  }
  console.log(`\nruns: missing ${runs.missing}, inserted ${runs.inserted}, skipped ${runs.skipped}`)
  console.log(`work items: missing ${items.missing}, inserted ${items.inserted}, skipped ${items.skipped}`)
}

void main()
  .then(async () => {
    await dev.end()
    await prod.end()
    process.exit(0)
  })
  .catch(async (error) => {
    console.error(error)
    process.exit(1)
  })
