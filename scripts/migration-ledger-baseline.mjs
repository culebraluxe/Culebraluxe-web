#!/usr/bin/env node
// ---------------------------------------------------------------------------
// Baseline the migration ledger (run ONCE, 2026-09-10).
//
// The ledger is authoritative only from the baseline forward. This records:
//   * the migrations actually applied during the 2026-09-10 drift repair;
//   * the DEV-only structural migrations applied that night;
//   * a <baseline> row per target documenting the verified parity state.
//
// Everything not listed stays "pre-baseline / unrecorded" — which migration-status
// reports honestly rather than pretending to know.
//
//   node --import tsx --env-file=.env.local --env-file=.env.local scripts/migration-ledger-baseline.mjs
// ---------------------------------------------------------------------------
import { readFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { forgeDb, forgeDbTargetForUrl } from '../db/forge-db.ts'

const BASELINE_NOTE =
  '2026-09-10 baseline: DEV and PROD verified identical across tables, columns, indexes and FKs (pnpm db:parity). Pre-baseline application history is unknown and intentionally not claimed.'

const APPLIED_2026_09_10 = {
  prod: [
    'db/migrations/116_property_address_line.sql',
    'db/migrations/117_firm_relation_role.sql',
    'db/migrations/118_security_role_business_role.sql',
    'db/migrations/119_contract_persistence.sql',
    'db/migrations/120_pns_seller_representative_role.sql',
    'db/migrations/121_contract_form_document_lineage.sql',
    'db/migrations/122_showing_report_form_lineage.sql',
    'db/migrations/138_regrid_culebra_parcel.sql',
    'db/migrations/144_schema_migration_ledger.sql',
  ],
  dev: [
    'db/migrations/142_forge_dispatch_columns.sql',
    'db/migrations/143_forge_dispatch_parallel_lock.sql',
    'db/migrations/144_schema_migration_ledger.sql',
  ],
}

// Objects verified present in DEV at baseline (applied before the ledger existed).
const DEV_PRESENT_AT_BASELINE = [
  'db/migrations/116_property_address_line.sql',
  'db/migrations/117_firm_relation_role.sql',
  'db/migrations/118_security_role_business_role.sql',
  'db/migrations/119_contract_persistence.sql',
  'db/migrations/120_pns_seller_representative_role.sql',
  'db/migrations/121_contract_form_document_lineage.sql',
  'db/migrations/122_showing_report_form_lineage.sql',
  'db/migrations/138_regrid_culebra_parcel.sql',
]

const checksumOf = async (file) =>
  `sha256:${createHash('sha256').update(await readFile(file, 'utf8')).digest('hex')}`

const dev = forgeDb.forTarget(forgeDbTargetForUrl(process.env.DATABASE_URL_DEV))
const prod = forgeDb.forTarget(forgeDbTargetForUrl(process.env.DATABASE_URL_PROD))

async function upsert(pool, filename, checksum, target, note) {
  await pool.query(
    `insert into schema_migration (filename, checksum, target, note)
     values ($1, $2, $3, $4)
     on conflict (filename, target) do update set note = excluded.note, checksum = excluded.checksum`,
    [filename, checksum, target, note],
  )
}

try {
  for (const [target, pool] of [['dev', dev], ['prod', prod]]) {
    for (const file of APPLIED_2026_09_10[target]) {
      await upsert(pool, file, await checksumOf(file), target, 'applied 2026-09-10 during the drift repair')
    }
    await upsert(pool, '<baseline>', 'parity-ok-2026-09-10', target, BASELINE_NOTE)
    console.log(`${target}: recorded ${APPLIED_2026_09_10[target].length} applied + baseline`)
  }

  for (const file of DEV_PRESENT_AT_BASELINE) {
    await upsert(dev, file, await checksumOf(file), 'dev', 'objects present in DEV at the 2026-09-10 baseline (applied pre-ledger)')
  }
  console.log(`dev: recorded ${DEV_PRESENT_AT_BASELINE.length} pre-baseline (objects verified present)`)
} finally {
  await dev.end()
  await prod.end()
}
