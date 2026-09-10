// Schema parity: structural comparison of two control-plane databases.
//
// This module is PURE on purpose (snapshot in -> report out): unit testable
// without databases, and it lives in lib/ without importing the Neon driver
// (ARCH rule: only db/database-gateway.ts and lib/neon-interactive.ts may).
// The database reader lives in workflow_app/forge/schema-parity.ts and serves
// both `pnpm db:parity` and the Forge DEV_OPS gate.
//
// Compares four axes: tables, columns, indexes, foreign keys. Indexes are not
// cosmetic — the Forge dispatch lock lives in a partial unique index, so a
// missing index silently changes engine behavior.

export type SchemaSnapshot = {
  tables: string[]
  columns: Map<string, Map<string, string>>
  indexes: Map<string, string>
  fks: Map<string, string>
}

export type ParityReport = {
  tablesOnlyDev: string[]
  tablesOnlyProd: string[]
  columnDrift: string[]
  indexDrift: string[]
  fkDrift: string[]
  clean: boolean
}

export function compareSnapshots(dev: SchemaSnapshot, prod: SchemaSnapshot): ParityReport {
  const tablesOnlyDev = dev.tables.filter((t) => !prod.tables.includes(t))
  const tablesOnlyProd = prod.tables.filter((t) => !dev.tables.includes(t))

  const columnDrift: string[] = []
  for (const t of dev.tables.filter((x) => prod.tables.includes(x))) {
    const dc = dev.columns.get(t) ?? new Map<string, string>()
    const pc = prod.columns.get(t) ?? new Map<string, string>()
    const devOnly = [...dc.keys()].filter((c) => !pc.has(c))
    const prodOnly = [...pc.keys()].filter((c) => !dc.has(c))
    const typeDrift = [...dc.keys()].filter((c) => pc.has(c) && pc.get(c) !== dc.get(c))
    if (devOnly.length) columnDrift.push(`${t}: DEV-only cols ${devOnly.join(', ')}`)
    if (prodOnly.length) columnDrift.push(`${t}: PROD-only cols ${prodOnly.join(', ')}`)
    for (const c of typeDrift) columnDrift.push(`${t}.${c}: DEV=${dc.get(c)} PROD=${pc.get(c)}`)
  }

  const indexDrift: string[] = []
  for (const k of new Set([...dev.indexes.keys(), ...prod.indexes.keys()])) {
    const dv = dev.indexes.get(k)
    const pv = prod.indexes.get(k)
    if (dv === pv) continue
    if (!dv) indexDrift.push(`${k}: PROD-only`)
    else if (!pv) indexDrift.push(`${k}: DEV-only`)
    else indexDrift.push(`${k}: definition differs`)
  }

  const fkDrift: string[] = []
  for (const k of new Set([...dev.fks.keys(), ...prod.fks.keys()])) {
    const dv = dev.fks.get(k)
    const pv = prod.fks.get(k)
    if (dv === pv) continue
    fkDrift.push(`${k}: DEV=${dv ?? '-'} PROD=${pv ?? '-'}`)
  }

  return {
    tablesOnlyDev,
    tablesOnlyProd,
    columnDrift,
    indexDrift,
    fkDrift,
    clean:
      tablesOnlyDev.length === 0 &&
      tablesOnlyProd.length === 0 &&
      columnDrift.length === 0 &&
      indexDrift.length === 0 &&
      fkDrift.length === 0,
  }
}
