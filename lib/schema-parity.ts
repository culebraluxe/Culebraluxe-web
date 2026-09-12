// Schema parity: structural comparison of two control-plane databases.
//
// This module is PURE on purpose (snapshot in -> report out): unit testable
// without databases, and it lives in lib/ without importing the Neon driver
// (ARCH rule: only db/database-gateway.ts and lib/neon-interactive.ts may).
// The database reader lives in workflow_app/forge/schema-parity.ts and serves
// both `pnpm db:parity` and the Forge DEV_OPS gate.
//
// Compares five axes: tables, columns, indexes, foreign keys, check constraints.
// Indexes are not cosmetic — the Forge dispatch lock lives in a partial unique
// index, so a missing index silently changes engine behavior.
//
// FORGE-PARITY-CHECK-01: the fifth axis is new, and it was the one carrying real
// drift. A CHECK constraint is enforcement, not decoration: on 2026-09-12 PROD
// carried `agent_work_item_parallel_shape_check` on the split lane's parallel
// shape and DEV did not, and this gate called the two databases identical. A
// constraint is also asymmetric in a way the other axes are not — PROD being
// STRICTER than DEV is the dangerous direction (a worker passes in DEV and fails
// in PROD), so validity is part of the compared value: a NOT VALID constraint
// enforces nothing for existing rows and must not read as equal to a validated one.

/** `${table}.${constraintName}` -> normalized definition, with NOT VALID marked. */
export type CheckConstraints = Map<string, string>

/**
 * The compared value of one CHECK constraint: its definition with runs of
 * whitespace collapsed, and `NOT VALID` appended when Postgres is not enforcing it
 * for existing rows.
 *
 * This lives here, not in the database reader, so the format has ONE owner: the
 * reader supplies a definition and the comparison's meaning stays pure and
 * testable. A NOT VALID constraint enforces nothing retroactively, so treating it
 * as equal to a validated one would hide exactly the "PROD is stricter than DEV"
 * case this axis exists to catch.
 */
export function checkValue(definition: string, validated: boolean): string {
  const normalized = definition.replace(/\s+/g, ' ').trim()
  return validated ? normalized : `${normalized} NOT VALID`
}

export type SchemaSnapshot = {
  tables: string[]
  columns: Map<string, Map<string, string>>
  indexes: Map<string, string>
  fks: Map<string, string>
  /** `${table}.${constraintName}` -> normalized definition, with NOT VALID marked. */
  checks: Map<string, string>
}

export type ParityReport = {
  tablesOnlyDev: string[]
  tablesOnlyProd: string[]
  columnDrift: string[]
  indexDrift: string[]
  fkDrift: string[]
  checkDrift: string[]
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

  /**
   * Check constraints. Keyed by name (like indexes and FKs, so a rename is
   * visible rather than silently treated as equivalent) and compared on the
   * normalized definition plus validity, because two constraints with the same
   * name and different definitions is precisely the drift this axis exists for.
   * A constraint present on one side only prints as `DEV=-` / `PROD=-`.
   */
  const checkDrift: string[] = []
  for (const k of new Set([...dev.checks.keys(), ...prod.checks.keys()])) {
    const dv = dev.checks.get(k)
    const pv = prod.checks.get(k)
    if (dv === pv) continue
    checkDrift.push(`${k}: DEV=${dv ?? '-'} PROD=${pv ?? '-'}`)
  }

  return {
    tablesOnlyDev,
    tablesOnlyProd,
    columnDrift,
    indexDrift,
    fkDrift,
    checkDrift,
    clean:
      tablesOnlyDev.length === 0 &&
      tablesOnlyProd.length === 0 &&
      columnDrift.length === 0 &&
      indexDrift.length === 0 &&
      fkDrift.length === 0 &&
      checkDrift.length === 0,
  }
}
