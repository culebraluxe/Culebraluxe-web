#!/usr/bin/env node
// ---------------------------------------------------------------------------
// Pull PROD data down into DEV (PROD -> DEV, the safe direction).
//
//   node --env-file=.env.local scripts/pull-prod-to-dev.mjs            # plan only
//   node --env-file=.env.local scripts/pull-prod-to-dev.mjs --apply    # execute
//
// PRESERVED ON DEV (never touched):
//   authorization : app_user, app_user_role, auth_identity, security_role,
//                   role, role_alias, authority, role_authority
//   workspace     : project, wbs_item, wbs_project   (DEV Projects work)
//   forge         : agent_work_item, storyboard_*    (DEV engine state)
//   diagnostics   : app_error
//
// Partitioned parents are copied; partitions are skipped (inserting through the
// parent routes rows to the right partition, so copying both would duplicate).
// Tables are deleted child-first and inserted parent-first so FKs stay valid.
// ---------------------------------------------------------------------------
import { Pool } from '@neondatabase/serverless'

const APPLY = process.argv.includes('--apply')
const dev = new Pool({ connectionString: process.env.DATABASE_URL_DEV })
const prod = new Pool({ connectionString: process.env.DATABASE_URL_PROD })

// Authorization rows are never copied, and they do NOT seed the preserve
// closure: business tables legitimately FK to app_user/security_role/authority,
// and those ids are IDENTICAL in DEV and PROD, so the references resolve.
const PRESERVE_AUTH = new Set([
  'app_user', 'app_user_role', 'auth_identity', 'security_role', 'role', 'role_alias', 'authority', 'role_authority',
])

// Structural state owned by DEV. Anything that depends on these must also stay
// on DEV (e.g. forge_engine_task_execution -> agent_work_item), otherwise PROD
// rows would reference rows that do not exist here.
const PRESERVE_STRUCT_EXACT = new Set(['project', 'wbs_item', 'wbs_project', 'agent_work_item', 'app_error'])
const PRESERVE_STRUCT_PREFIX = ['storyboard_']
const structural = (t) => PRESERVE_STRUCT_EXACT.has(t) || PRESERVE_STRUCT_PREFIX.some((p) => t.startsWith(p))


async function tables(pool) {
  const r = await pool.query(`
    select c.relname as name
    from pg_class c join pg_namespace n on n.oid = c.relnamespace
    where n.nspname = 'public' and c.relkind in ('r','p') and not c.relispartition
    order by 1`)
  return r.rows.map((x) => x.name)
}

async function fkEdges(pool, targets) {
  const r = await pool.query(`
    select con.conrelid::regclass::text as child, con.confrelid::regclass::text as parent
    from pg_constraint con
    where con.contype = 'f' and con.conrelid <> con.confrelid`)
  return r.rows
    .map(({ child, parent }) => ({ child: child.split('.').pop(), parent: parent.split('.').pop() }))
    .filter((e) => targets.has(e.child) && targets.has(e.parent))
}

/** parent-first insertion order; children deleted in reverse. */
function topoSort(nodes, edges) {
  const order = []
  const state = new Map()
  const visit = (n) => {
    if (state.get(n) === 'done' || state.get(n) === 'visiting') return
    state.set(n, 'visiting')
    for (const e of edges) if (e.child === n) visit(e.parent)
    state.set(n, 'done')
    order.push(n)
  }
  for (const n of nodes) visit(n)
  return order
}

const lit = (v) => {
  if (v === null || v === undefined) return 'NULL'
  if (typeof v === 'number') return String(v)
  if (typeof v === 'boolean') return v ? 'true' : 'false'
  if (v instanceof Date) return `'${v.toISOString()}'`
  if (Array.isArray(v) || typeof v === 'object') return `'${JSON.stringify(v).replace(/'/g, "''")}'`
  return `'${String(v).replace(/'/g, "''")}'`
}

async function main() {
  const prodTables = await tables(prod)
  const devSet = new Set(
    (await dev.query("select table_name from information_schema.tables where table_schema='public'")).rows.map((r) => r.table_name),
  )

  const kept = prodTables.filter((t) => PRESERVE_AUTH.has(t) || structural(t))
  const missingInDev = prodTables.filter((t) => !PRESERVE_AUTH.has(t) && !structural(t) && !devSet.has(t))

  // Preserve closure from STRUCTURAL tables only (forge/projects subtree).
  const preservedSet = new Set([...prodTables.filter(structural), ...prodTables.filter((t) => PRESERVE_AUTH.has(t))])
  const allEdges = await fkEdges(prod, new Set(prodTables))
  for (let changed = true; changed; ) {
    changed = false
    for (const e of allEdges) {
      const parentIsStructural = structural(e.parent)
      if (parentIsStructural && !preservedSet.has(e.child)) {
        preservedSet.add(e.child)
        changed = true
      }
    }
  }
  const closureAdded = [...preservedSet].filter((t) => !structural(t) && !PRESERVE_AUTH.has(t))

  const tSet = new Set(prodTables.filter((t) => !preservedSet.has(t) && devSet.has(t)))
  const edges = await fkEdges(prod, tSet)
  const targets = topoSort([...tSet], edges)

  const plan = []
  let total = 0
  for (const t of targets) {
    const n = (await prod.query(`select count(*)::int n from "${t}"`)).rows[0].n
    plan.push([t, n])
    total += n
  }

  console.log(`pull PROD -> DEV  (${APPLY ? 'APPLY' : 'PLAN ONLY'})`)
  console.log(`  tables to copy   : ${targets.length}`)
  console.log(`  rows to copy     : ${total}`)
  console.log(`  preserved on DEV : ${kept.length} (${kept.slice(0, 10).join(', ')}${kept.length > 10 ? ', …' : ''})`)
  if (closureAdded.length) console.log(`  + FK closure kept: ${closureAdded.length} (${closureAdded.slice(0, 10).join(', ')}${closureAdded.length > 10 ? ', …' : ''})`)
  if (missingInDev.length) console.log(`  not in DEV       : ${missingInDev.join(', ')}`)
  const big = plan.filter(([, n]) => n > 1000).map(([t, n]) => `${t}(${n})`)
  if (big.length) console.log(`  large tables     : ${big.join(', ')}`)

  if (!APPLY) {
    console.log('\nplan only — re-run with --apply to execute')
    return
  }

  console.log('\nclearing DEV tables (child-first)…')
  for (const t of [...targets].reverse()) await dev.query(`delete from "${t}"`)

  console.log('copying PROD rows (parent-first)…')
  for (const t of targets) {
    const meta = (await prod.query(
      "select column_name, data_type from information_schema.columns where table_schema='public' and table_name=$1 order by ordinal_position",
      [t],
    )).rows
    const cols = meta.map((m) => m.column_name)
    const rows = (await prod.query(`select * from "${t}"`)).rows
    if (!rows.length) { console.log(`  ${t}: 0`); continue }
    const chunk = Math.max(1, Math.min(400, Math.floor(30000 / Math.max(1, cols.length))))
    for (let i = 0; i < rows.length; i += chunk) {
      const params = []
      const tuples = rows.slice(i, i + chunk).map((row) => {
        const ph = meta.map((m) => {
          const v = row[m.column_name]
          if (m.data_type === 'json' || m.data_type === 'jsonb') {
            params.push(v === null || v === undefined ? null : JSON.stringify(v))
            return `$${params.length}::${m.data_type}`
          }
          params.push(v === undefined ? null : v)
          return `$${params.length}`
        })
        return `(${ph.join(', ')})`
      })
      await dev.query(
        `insert into "${t}" (${cols.map((c) => `"${c}"`).join(', ')}) values ${tuples.join(',')}`,
        params,
      )
    }
    console.log(`  ${t}: ${rows.length}`)
  }

  console.log('\nverifying counts…')
  let mismatch = 0
  for (const [t, n] of plan) {
    const d = (await dev.query(`select count(*)::int n from "${t}"`)).rows[0].n
    if (d !== n) { mismatch++; console.log(`  MISMATCH ${t}: DEV=${d} PROD=${n}`) }
  }
  console.log(mismatch === 0 ? 'OK - every copied table now matches PROD' : `${mismatch} mismatches`)
}

main()
  .catch((e) => { console.error('PULL FAILED:', e); process.exitCode = 1 })
  .finally(async () => { await dev.end(); await prod.end() })

