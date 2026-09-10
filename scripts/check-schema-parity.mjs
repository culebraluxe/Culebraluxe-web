#!/usr/bin/env node
// ---------------------------------------------------------------------------
// Schema parity check: DEV vs PROD (read-only).
//
// The 116-122/138 role + contract refactor reached DEV but not PROD for weeks
// because nothing compared the two schemas and there is no migration ledger.
// Run this before promoting migrations and after applying them.
//
// Usage:
//   node --env-file=.env.local scripts/check-schema-parity.mjs
//
// Exits non-zero when drift is found, so it can gate a release.
// ---------------------------------------------------------------------------
import { Pool } from '@neondatabase/serverless'

const devUrl = process.env.DATABASE_URL_DEV
const prodUrl = process.env.DATABASE_URL_PROD
if (!devUrl || !prodUrl) {
  console.error('check-schema-parity: DATABASE_URL_DEV and DATABASE_URL_PROD are required')
  process.exit(2)
}

const dev = new Pool({ connectionString: devUrl })
const prod = new Pool({ connectionString: prodUrl })

async function schema(pool) {
  const tables = await pool.query(
    "select table_name from information_schema.tables where table_schema='public' and table_type='BASE TABLE' order by 1",
  )
  const cols = await pool.query(
    "select table_name, column_name, data_type, is_nullable from information_schema.columns where table_schema='public' order by 1,2",
  )
  const map = new Map()
  for (const c of cols.rows) {
    if (!map.has(c.table_name)) map.set(c.table_name, new Map())
    map.get(c.table_name).set(c.column_name, `${c.data_type}${c.is_nullable === 'NO' ? ' NOT NULL' : ''}`)
  }
  return { tables: tables.rows.map((r) => r.table_name), map }
}

try {
  const d = await schema(dev)
  const p = await schema(prod)
  const onlyDev = d.tables.filter((t) => !p.tables.includes(t))
  const onlyProd = p.tables.filter((t) => !d.tables.includes(t))

  console.log(`tables: DEV=${d.tables.length}  PROD=${p.tables.length}`)
  console.log(`only in DEV : ${onlyDev.length ? onlyDev.join(', ') : '(none)'}`)
  console.log(`only in PROD: ${onlyProd.length ? onlyProd.join(', ') : '(none)'}`)

  let drift = 0
  for (const t of d.tables.filter((x) => p.tables.includes(x))) {
    const dc = d.map.get(t) ?? new Map()
    const pc = p.map.get(t) ?? new Map()
    const devOnly = [...dc.keys()].filter((c) => !pc.has(c))
    const prodOnly = [...pc.keys()].filter((c) => !dc.has(c))
    const typeDrift = [...dc.keys()].filter((c) => pc.has(c) && pc.get(c) !== dc.get(c))
    if (devOnly.length || prodOnly.length || typeDrift.length) {
      drift++
      console.log(`  ${t}:`)
      if (devOnly.length) console.log(`    DEV-only cols  : ${devOnly.join(', ')}`)
      if (prodOnly.length) console.log(`    PROD-only cols : ${prodOnly.join(', ')}`)
      if (typeDrift.length) {
        console.log(`    type drift     : ${typeDrift.map((c) => `${c} DEV=${dc.get(c)} PROD=${pc.get(c)}`).join('; ')}`)
      }
    }
  }
  console.log(`column-drift tables: ${drift}`)

  const clean = onlyDev.length === 0 && onlyProd.length === 0 && drift === 0
  console.log(clean ? '\nPARITY OK' : '\nDRIFT FOUND')
  process.exitCode = clean ? 0 : 1
} finally {
  await dev.end()
  await prod.end()
}
