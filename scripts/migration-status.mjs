#!/usr/bin/env node
// ---------------------------------------------------------------------------
// Migration ledger status: what is recorded as applied, where, and what is not.
//
//   node --env-file=.env.local scripts/migration-status.mjs
//
// "Unrecorded" is honest, not alarming: the ledger is authoritative only from
// the 2026-09-10 baseline forward. Files that predate it show up there by design.
// ---------------------------------------------------------------------------
import { readdir } from 'node:fs/promises'
import { Pool } from '@neondatabase/serverless'

const dev = new Pool({ connectionString: process.env.DATABASE_URL_DEV })
const prod = new Pool({ connectionString: process.env.DATABASE_URL_PROD })

try {
  const files = (await readdir('db/migrations')).filter((f) => f.endsWith('.sql')).sort()
  const devRows = new Map(
    (await dev.query('select filename, checksum, target, applied_at, note from schema_migration')).rows
      .filter((r) => r.target === 'dev')
      .map((r) => [r.filename, r]),
  )
  const prodRows = new Map(
    (await prod.query('select filename, checksum, target, applied_at, note from schema_migration')).rows
      .filter((r) => r.target === 'prod')
      .map((r) => [r.filename, r]),
  )
  const ledger = [...devRows.values(), ...prodRows.values()].sort(
    (a, b) => new Date(b.applied_at).getTime() - new Date(a.applied_at).getTime(),
  )

  console.log(`ledger: ${ledger.length} rows   migrations on disk: ${files.length}`)
  console.log(`  dev  recorded: ${devRows.size}`)
  console.log(`  prod recorded: ${prodRows.size}`)

  const recent = ledger.filter((r) => r.filename !== '<baseline>').slice(0, 12)
  console.log('\nrecently recorded:')
  for (const r of recent) {
    console.log(`  ${String(r.applied_at).slice(0, 10)}  ${r.target.padEnd(4)}  ${r.filename}${r.note ? `  — ${r.note}` : ''}`)
  }
  const baseline = ledger.filter((r) => r.filename === '<baseline>')
  if (baseline.length) {
    console.log('\nbaseline rows:')
    for (const b of baseline) console.log(`  ${String(b.applied_at).slice(0, 10)}  ${b.target.padEnd(4)}  ${b.note}`)
  }

  const unrecorded = files.filter((f) => !devRows.has(`db/migrations/${f}`) && !prodRows.has(`db/migrations/${f}`))
  console.log(`\nunrecorded (pre-baseline or never applied here): ${unrecorded.length}`)
  for (const f of unrecorded.slice(0, 15)) console.log(`  ${f}`)
  if (unrecorded.length > 15) console.log(`  … and ${unrecorded.length - 15} more`)

  const oneSided = files.filter((f) => {
    const d = devRows.has(`db/migrations/${f}`)
    const p = prodRows.has(`db/migrations/${f}`)
    return (d || p) && !(d && p)
  })
  if (oneSided.length) {
    console.log(`\nrecorded for ONE target only (check whether that is intended): ${oneSided.length}`)
    for (const f of oneSided) {
      const d = devRows.has(`db/migrations/${f}`)
      console.log(`  ${f}  [${d ? 'dev' : 'prod'} only]`)
    }
  }
} finally {
  await dev.end()
  await prod.end()
}
