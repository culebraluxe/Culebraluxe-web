// ONE-OFF: normalize the story priority vocabulary on the board.
//
//   node --env-file=.env.local --import tsx scripts/normalize-story-priorities.ts
//
// WHY: 'Reference' was never added to the app's declared priority vocabulary, so
// whoever created the reference rows invented the value — and two spellings
// appeared: 'REFERENCE' (five Forge history rows) and 'Reference' (ARCH-HANDOFF,
// SOP1, DEEP1, PORTAL-06). The board sorts on the literal string, so the two
// spellings sorted apart. lib/story-priority.ts now owns the vocabulary and
// declares 'Reference' as a real member; this script brings the stored rows in
// line with it.
//
// WHY THIS IS SAFE: storyboard_story.priority is plain `text` with NO check
// constraint (verified against information_schema + pg_constraint), so this is a
// data normalization, not a schema change — no migration is needed and none is
// invented. It is deliberately NON-DESTRUCTIVE: no column, table or history is
// removed, and it is idempotent (a second run finds nothing to change). It does
// NOT guess at 'P2' (one row, ENG-DB-RESILIENCE-01, status Hold) — that value's
// intent is unknown and mapping it would be inventing business semantics; the
// script reports it instead.
//
// NOTE: explicit PROD pool, not the application gateway — db/database-gateway.ts
// binds its executor at MODULE LOAD, so flipping APP_ENV inside a script body
// does not change the connection. See DEEP1 section 6.
import { createPoolExecutor } from './lib/pool-executor'

const CANONICAL = 'Reference'
const LEGACY = 'REFERENCE'

async function main() {
  const url = process.env.DATABASE_URL_PROD
  if (!url) throw new Error('DATABASE_URL_PROD is not set (fail closed)')
  if (url === process.env.DATABASE_URL_DEV) throw new Error('PROD URL equals DEV URL (fail closed)')

  // The Neon branch token lives only in the HOST; print both so a misroute is
  // visible rather than inferred from a resolved-target diagnostic.
  console.log(`prod host: ${new URL(url).host}`)
  console.log(
    `dev  host: ${process.env.DATABASE_URL_DEV ? new URL(process.env.DATABASE_URL_DEV).host : '(unset)'}`,
  )

  const pool = createPoolExecutor(url)
  try {
    const stale = await pool.execute`
      select id, left(title, 55) as title from storyboard_story
       where priority = ${LEGACY}
       order by id
    `

    if (stale.length === 0) {
      console.log(`nothing to do: no story carries priority '${LEGACY}'`)
    } else {
      console.log(`rewriting ${stale.length} row(s): '${LEGACY}' -> '${CANONICAL}'`)
      for (const row of stale) console.log(`  ${String(row.id)}  ${String(row.title)}`)

      const updated = await pool.execute`
        update storyboard_story
           set priority = ${CANONICAL}, updated_at = now()
         where priority = ${LEGACY}
     returning id
      `
      console.log(`updated ${updated.length} row(s)`)
    }

    // Report — never rewrite — values outside the declared vocabulary. These are
    // someone's intent, and guessing it is not a script's job.
    const orphans = await pool.execute`
      select priority, id, status from storyboard_story
       where priority not in (
         'Critical','High','High-ish','Medium-High','Medium','Low','Later',
         'High-value polish','Reference'
       )
       order by priority, id
    `
    if (orphans.length === 0) {
      console.log('vocabulary check: no rows outside the declared priority set')
    } else {
      console.log(`vocabulary check: ${orphans.length} row(s) OUTSIDE the declared set (left untouched):`)
      for (const row of orphans) {
        console.log(`  [${String(row.priority)}] ${String(row.id)} (${String(row.status)})`)
      }
    }

    const counts = await pool.execute`
      select priority, count(*)::int as n from storyboard_story
       group by 1 order by 2 desc, 1
    `
    console.log('priority distribution after the pass:')
    for (const row of counts) {
      console.log(`  ${String(row.priority).padEnd(20)} ${String(row.n)}`)
    }
  } finally {
    await pool.end()
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exitCode = 1
})
