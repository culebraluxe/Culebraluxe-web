#!/usr/bin/env node
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
import { Pool } from '@neondatabase/serverless'
import { modelWidgetWeight } from '../workflow_app/forge/forge-estimator'

async function main() {
  const which = (process.argv[2] ?? 'dev').toLowerCase()
  if (which !== 'dev' && which !== 'prod') throw new Error(`unknown target: ${which}`)
  const url = which === 'prod' ? process.env.DATABASE_URL_PROD : process.env.DATABASE_URL_DEV
  const pool = new Pool({ connectionString: url })
  try {
    const rows = await pool.query(
      `select id, model_used, started_at, ended_at, cost_usd
       from storyboard_story_run
       where cost_usd is null and model_used is not null and ended_at is not null`,
    )
    let updated = 0
    for (const row of rows.rows) {
      const weight = modelWidgetWeight(String(row.model_used))
      if (weight === null) continue
      const minutes =
        (new Date(row.ended_at).getTime() - new Date(row.started_at).getTime()) / 60000
      if (!Number.isFinite(minutes) || minutes < 0) continue
      const widgets = Math.round(weight * minutes * 100) / 100
      await pool.query(`update storyboard_story_run set cost_usd = $1 where id = $2`, [widgets, row.id])
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
