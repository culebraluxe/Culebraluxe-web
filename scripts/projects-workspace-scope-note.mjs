#!/usr/bin/env node
// Normalize the PROJECTS-WORKSPACE board so Forge can dispatch it:
//   * maturity ("Partial") belongs in NOTES, not in board status (Forge dispatches
//     on work-item state, and `Partial` is a maturity claim, not a dispatch state);
//   * every story gets an explicit FORGE SCOPE line so the Lead plans only the
//     REMAINING work instead of rebuilding delivered foundations;
//   * machine acceptance vs human acceptance is restated so nothing is faked.
//
// Idempotent: marker-guarded. Dry-run unless --apply.

import { forgeDb, forgeDbTargetForUrl } from '../db/forge-db.ts'

const APPLY = process.argv.includes('--apply')
const NOTE =
  '\n\nFORGE SCOPE (2026-09-10): work ONLY the remaining acceptance items — the BASELINE STATUS ' +
  'above lists what the existing foundation already delivers; extend it, do not rebuild it. ' +
  'Machine acceptance must pass the FORGE MACHINE ASSAY commands; HUMAN GATE items are verified ' +
  'by the captain, never by Forge.'
const MARK = 'FORGE SCOPE (2026-09-10)'

for (const [label, url] of [
  ['PROD', process.env.DATABASE_URL_PROD],
  ['DEV', process.env.DATABASE_URL_DEV],
]) {
  const pool = forgeDb.forTarget(forgeDbTargetForUrl(url))
  const needNote = await pool.query(
    `select count(*)::int n from storyboard_story
      where id like 'PROJECTS-WORKSPACE-%' and coalesce(notes,'') not like $1`,
    [`%${MARK}%`],
  )
  const needStatus = await pool.query(
    `select count(*)::int n from storyboard_story
      where id like 'PROJECTS-WORKSPACE-%' and status = 'Partial'`,
  )
  console.log(`${label}: notes missing scope=${needNote.rows[0].n} | status Partial=${needStatus.rows[0].n}`)
  if (APPLY) {
    const a = await pool.query(
      `update storyboard_story set notes = coalesce(notes,'') || $1, updated_at = now()
        where id like 'PROJECTS-WORKSPACE-%' and coalesce(notes,'') not like $2`,
      [NOTE, `%${MARK}%`],
    )
    const b = await pool.query(
      `update storyboard_story set status = 'Planned', updated_at = now()
        where id like 'PROJECTS-WORKSPACE-%' and status = 'Partial'`,
    )
    console.log(`${label}: scope note added=${a.rowCount} | Partial->Planned=${b.rowCount}`)
  }
  await pool.end()
}
console.log(APPLY ? 'APPLIED' : 'DRY RUN — re-run with --apply')
