#!/usr/bin/env node
// ---------------------------------------------------------------------------
// Schema parity gate: DEV vs PROD structural comparison (read-only).
//
//   node --env-file=.env.local --import tsx scripts/check-schema-parity.ts
//   pnpm db:parity
//
// Compares tables, columns, indexes and FKs. Exits non-zero on ANY drift so it
// can gate a release. Implementation lives in lib/schema-parity.ts and is shared
// with the Forge DEV_OPS gate (workflow_app/forge/release-operations.ts).
// ---------------------------------------------------------------------------
import { checkSchemaParity } from '../workflow_app/forge/schema-parity'

async function main() {
  const devUrl = process.env.DATABASE_URL_DEV
  const prodUrl = process.env.DATABASE_URL_PROD
  if (!devUrl || !prodUrl) {
    console.error('check-schema-parity: DATABASE_URL_DEV and DATABASE_URL_PROD are required')
    process.exit(2)
  }

  const report = await checkSchemaParity(devUrl, prodUrl)

  console.log(`tables only in DEV : ${report.tablesOnlyDev.join(', ') || '(none)'}`)
  console.log(`tables only in PROD: ${report.tablesOnlyProd.join(', ') || '(none)'}`)
  console.log(`column drift: ${report.columnDrift.length}`)
  for (const l of report.columnDrift) console.log(`  ${l}`)
  console.log(`index drift : ${report.indexDrift.length}`)
  for (const l of report.indexDrift) console.log(`  ${l}`)
  console.log(`fk drift    : ${report.fkDrift.length}`)
  for (const l of report.fkDrift) console.log(`  ${l}`)

  console.log(report.clean ? '\nPARITY OK' : '\nDRIFT FOUND')
  process.exit(report.clean ? 0 : 1)
}

main().catch((error) => {
  console.error('check-schema-parity failed:', error)
  process.exit(2)
})
