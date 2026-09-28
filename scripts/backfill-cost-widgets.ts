#!/usr/bin/env node
// ---------------------------------------------------------------------------
// ⚠ BROKEN ON PURPOSE — DO NOT FIX, DO NOT IMPORT, DO NOT CALL, DO NOT REVIVE.
// The TypeScript engine and its libraries were deleted in the 2026-09 Rust port, so this
// file cannot load: the modules it imports from are gone. It is kept as REFERENCE ONLY,
// so the behaviour it describes can be translated into Rust when that behaviour is wanted.
// Reviving it in place is forbidden — see AGENTS.md ("legacy/ is out of scope") and
// docs/agent/BROKEN-TS-INVENTORY.md for the priority list and each capability's Rust home.
// ---------------------------------------------------------------------------
// -----------------------------------------------------------------------------
// Backfill cost widgets onto historical finished runs (DEV by default).
//
// The harness previously left cost_usd null (the vendor invoices late). Widgets
// = model weight x elapsed minutes give calibration data NOW. Real-money columns
// and PROD business data are never touched.
//
// Usage:
//   node --env-file=.env.local scripts/backfill-cost-widgets.ts [dev|prod]
// -----------------------------------------------------------------------------
import { forgeDb, forgeDbTargetForUrl } from '@/legacy/db/forge-db'
import { modelWidgetWeight } from '@/legacy/workflow_app/forge/forge-estimator'

async function main() {
  const which = (process.argv[2] ?? 'dev').toLowerCase()
  if (which !== 'dev' && which !== 'prod') throw new Error(`unknown target: ${which}`)
  const url = which === 'prod' ? process.env.DATABASE_URL_PROD : process.env.DATABASE_URL_DEV
  const pool = forgeDb.forTarget(forgeDbTargetForUrl(url))
  try {
    const rows = await pool.query(
      `select id, model_used, started_at, ended_at, cost_widgets
       from storyboard_story_run
       where cost_widgets is null and model_used is not null and ended_at is not null`,
    )
    let updated = 0
    for (const row of rows.rows) {
      const weight = modelWidgetWeight(String(row.model_used))
      if (weight === null) continue
      const minutes =
        (new Date(row.ended_at).getTime() - new Date(row.started_at).getTime()) / 60000
      if (!Number.isFinite(minutes) || minutes < 0) continue
      const widgets = Math.round(weight * minutes * 100) / 100
      await pool.query(`update storyboard_story_run set cost_widgets = $1, cost_source = 'widgets' where id = $2`, [widgets, row.id])
      updated += 1
    }
    console.log(`backfilled ${updated} cost-widget rows -> ${which}`)
  } finally {
    await pool.end()
  }
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
