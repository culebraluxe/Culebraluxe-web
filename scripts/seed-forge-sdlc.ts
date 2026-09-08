#!/usr/bin/env node
// -----------------------------------------------------------------------------
// Seed/upsert the active FORGE_SDLC definition (v4) into a control-plane DB.
//
// The engine starts stories with version = FORGE_SDLC_VERSION and loads the
// parsed graph from process_definitions(key, version). This script makes the
// v4 definition present in DEV or PROD so flipping FORGE_SDLC_VERSION to 4 can
// never start a story against a missing definition. It is idempotent for the
// FORGE_SDLC v4 row only (prior immutable v1/v2/v3 rows are never touched).
//
// Usage:
//   node --env-file=.env.local scripts/seed-forge-sdlc.ts [dev|prod]
// -----------------------------------------------------------------------------
import { Pool } from '@neondatabase/serverless'
import { parseForgeSdlcV4 } from '../workflow_app/definitions/forge-sdlc'

const which = (process.argv[2] ?? (process.env.APP_ENV === 'production' ? 'prod' : 'dev')).toLowerCase()
if (which !== 'prod' && which !== 'dev') {
  console.error(`unknown target: ${which}`)
  process.exit(2)
}
const url = which === 'prod' ? process.env.DATABASE_URL_PROD : process.env.DATABASE_URL_DEV
if (!url) {
  console.error(`no ${which.toUpperCase()} DATABASE_URL configured`)
  process.exit(2)
}

const graph = parseForgeSdlcV4().graph

async function main() {
  const pool = new Pool({ connectionString: url })
  try {
    // Idempotent for v4 only. (null tenant + unique(tenant_id,key,version) treats
    // NULLs as distinct in Postgres, so clear any prior v4 row before inserting.)
    await pool.query(`delete from process_definitions where key = 'FORGE_SDLC' and version = 4`)
    await pool.query(
      `insert into process_definitions (key, version, name, description, definition, status, created_by)
       values ($1, $2, $3, $4, $5::jsonb, 'active', 'forge-seed')
       on conflict (tenant_id, key, version) do nothing`,
      [
        'FORGE_SDLC',
        4,
        'Forge Software Delivery Lifecycle',
        'End-state Forge orchestration workflow (FAST lane + Architect review park).',
        JSON.stringify(graph),
      ],
    )
    console.log(`seeded FORGE_SDLC v4 -> ${which} control plane (${Object.keys(graph.nodes).length} nodes)`)
  } finally {
    await pool.end()
  }
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
