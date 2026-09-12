// ---------------------------------------------------------------------------
// DB BOUNDARY — one pool, and nobody else opens a connection.
//
// Captain's directive, 2026-09-12: "create our own ForgeDB Wrapper that uses this
// DB Connection Pool for the entire application. No class may have its own raw SQL
// without using the pool."
//
// This test ENFORCES that rule instead of agreeing with it. It fails the build:
//   1. if anything other than db/forge-db.ts imports `pg`;
//   2. if anything constructs a pool/client of its own outside db/forge-db.ts;
//   3. if the set of files still importing the old Neon driver GROWS.
//
// Rule 3 is a RATCHET, not an exemption. 38 files imported the Neon driver when the
// wrapper landed — each one deciding its own environment — so they were frozen in a
// list that may only SHRINK. As of 2026-09-12 that list is EMPTY: every one of them
// was swept onto ForgeDB, including `workflow_engine/lib/workflow/db.ts` (the
// engine's own client, and the most important connection in the system) once the
// captain unfroze it. An empty ratchet means ANY new importer fails the build.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'

const ROOT = process.cwd()

/** Directories worth scanning; dot-dirs and vendored clones are excluded. */
const SCAN_DIRS = [
  'app',
  'lib',
  'services',
  'db',
  'ui',
  'workflow_app',
  'workflow_engine',
  'agent-runtime',
  'scripts',
  'testv2',
]

const SKIP_DIRS = new Set(['node_modules', '.next', '.git', 'dist', 'build'])
const SOURCE_EXT = /\.(ts|tsx|mts|cts|mjs|cjs|js|jsx)$/

/** The ONLY module allowed to touch a driver. */
const POOL_OWNER = 'db/forge-db.ts'

/**
 * FROZEN — EMPTY, and it should stay that way.
 *
 * This listed the files still importing the old Neon driver. `workflow_engine`'s
 * db.ts was the last entry; the captain unfroze it on 2026-09-12 and it now uses
 * the shared pool. With no entries, ANY file importing `@neondatabase/serverless`
 * fails the build immediately.
 */
const FROZEN_NEON_IMPORTERS: readonly string[] = []

function walk(dir: string, out: string[] = []): string[] {
  let entries: string[]
  try {
    entries = readdirSync(dir)
  } catch {
    return out
  }
  for (const entry of entries) {
    if (SKIP_DIRS.has(entry) || entry.startsWith('.')) continue
    const full = join(dir, entry)
    if (statSync(full).isDirectory()) {
      walk(full, out)
      continue
    }
    if (SOURCE_EXT.test(entry)) out.push(full)
  }
  return out
}

function sourceFiles(): string[] {
  const files: string[] = []
  for (const dir of SCAN_DIRS) walk(join(ROOT, dir), files)
  return files
}

/** Import/require of a module specifier — static or dynamic. */
function importsModule(source: string, moduleName: string): boolean {
  const escaped = moduleName.replace(/[/\\]/g, (m) => `\\${m}`)
  const fromSpecifier = new RegExp(`from\\s+['"]${escaped}['"]`)
  const bareImport = new RegExp(`(^|\\n)\\s*import\\s+['"]${escaped}['"]`)
  const call = new RegExp(`(import|require)\\s*\\(\\s*['"]${escaped}['"]\\s*\\)`)
  return fromSpecifier.test(source) || bareImport.test(source) || call.test(source)
}

function offenders(match: (source: string) => boolean): string[] {
  return sourceFiles()
    .filter((file) => match(readFileSync(file, 'utf8')))
    .map((file) => relative(ROOT, file))
    .sort()
}

test('DB boundary: only ForgeDB may import the pg driver', () => {
  const importers = offenders((source) => importsModule(source, 'pg'))
  assert.deepEqual(
    importers,
    [POOL_OWNER],
    `\`pg\` may only be imported by ${POOL_OWNER}. Route the query through forgeDb instead.`,
  )
})

test('DB boundary: no NEW importer of the old Neon driver', () => {
  const importers = offenders((source) =>
    importsModule(source, '@neondatabase/serverless'),
  )
  const unexpected = importers.filter((f) => !FROZEN_NEON_IMPORTERS.includes(f))
  assert.deepEqual(
    unexpected,
    [],
    'A new file imports @neondatabase/serverless. The application has ONE pool now ' +
      '(db/forge-db.ts) — this ratchet may only shrink, so use forgeDb rather than adding an exemption.',
  )
})

test('DB boundary: the burn-down list has no stale entries', () => {
  const importers = offenders((source) =>
    importsModule(source, '@neondatabase/serverless'),
  )
  const stale = FROZEN_NEON_IMPORTERS.filter((f) => !importers.includes(f))
  assert.deepEqual(
    stale,
    [],
    'These files no longer import the Neon driver — remove them from FROZEN_NEON_IMPORTERS so the ' +
      'ratchet reflects reality and cannot hide a later re-introduction.',
  )
})

test('DB boundary: nothing outside ForgeDB constructs a pool or client', () => {
  const constructors = offenders(
    (source) =>
      /\bnew\s+Pool\s*\(/.test(source) ||
      /\bnew\s+Client\s*\(/.test(source) ||
      /\bcreatePool\s*\(/.test(source),
  ).filter((f) => f !== POOL_OWNER && !FROZEN_NEON_IMPORTERS.includes(f))
  assert.deepEqual(
    constructors,
    [],
    'A connection is being constructed outside ForgeDB. Use forgeDb.sql / forgeDb.pool(), or ' +
      'forgeDb.forTarget(target) when a script must name its environment explicitly.',
  )
})

test('DB boundary: ForgeDB owns the pool and is the exported entry point', () => {
  const owner = readFileSync(join(ROOT, POOL_OWNER), 'utf8')
  assert.match(owner, /export const forgeDb\b/, 'forgeDb must be the exported entry point')
  assert.match(owner, /new Pool\(/, 'ForgeDB must own the pg.Pool')
  assert.match(owner, /declareControlPlane/, 'the pool target must come from the single declaration')
})

test('DB boundary: the wrapper never binds a pool at module load', () => {
  // Importing db/forge-db.ts must stay free of a declared environment and a
  // configured URL, or every module that imports it becomes unimportable in tests
  // and in any process that has not declared where it runs.
  const owner = readFileSync(join(ROOT, POOL_OWNER), 'utf8')
  const loadTimePoolAssignment = /=\s*forgeDbPool\s*\(\s*\)/.test(owner)
  assert.equal(
    loadTimePoolAssignment,
    false,
    'forgeDbPool() must not be called at module scope — resolve the pool per call.',
  )
})

