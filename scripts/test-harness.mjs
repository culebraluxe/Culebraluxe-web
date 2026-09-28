#!/usr/bin/env node
// ---------------------------------------------------------------------------
// WHICH SUITES THE HARNESS RUNS — derived, not listed.
//
// `pnpm test:harness` used to be `node --import tsx --test scripts/*.test.ts`. That glob was written when
// every file under `scripts/` could load. The 2026-09 Rust port deleted the modules six of them import, so
// the glob ran six suites that die at import with ERR_MODULE_NOT_FOUND — and a harness that is red for a
// reason nobody can fix is a harness people learn to ignore. Six permanent failures is how a gate gets
// switched off.
//
// A HAND LIST WOULD ROT. The dead files already carry the marker the repository uses for this exact fact
// (`⚠ BROKEN ON PURPOSE`, the banner `scripts/scan-broken-ts.mjs` writes and `pnpm broken:ts:sweep` counts),
// so this runner DERIVES the list from the marker: marked files are excluded and named with their reason,
// unmarked files are run. A file that is repaired stops being excluded automatically, and a new dead file is
// excluded the moment it is marked — no second list to forget.
//
//   node scripts/test-harness.mjs           # run the live suites
//   node scripts/test-harness.mjs --list    # say what would run, and what is skipped, and why
// ---------------------------------------------------------------------------
import { execFileSync } from 'node:child_process'
import { readFileSync, readdirSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const MARKER = 'BROKEN ON PURPOSE'
/** The banner is the file's first block; a mention in prose further down is not a marker. */
const MARKER_LINES = 12

const suites = readdirSync(join(root, 'scripts'))
  .filter((name) => name.endsWith('.test.ts'))
  .sort()
  .map((name) => {
    const path = `scripts/${name}`
    const head = readFileSync(join(root, path), 'utf8').split('\n', MARKER_LINES).join('\n')
    return { path, dead: head.includes(MARKER) }
  })

const live = suites.filter((suite) => !suite.dead)
const dead = suites.filter((suite) => suite.dead)
const listOnly = process.argv.includes('--list')

console.log(`test:harness — ${live.length} live suite(s), ${dead.length} marked dead and skipped`)
for (const suite of dead) console.log(`  skip  ${suite.path}  (${MARKER} — its subject is Rust now)`)
for (const suite of live) console.log(`  run   ${suite.path}`)

if (listOnly) process.exit(0)
if (live.length === 0) {
  console.error('test:harness — every suite under scripts/ is marked dead: that is a finding, not a pass')
  process.exit(1)
}

try {
  execFileSync('node', ['--import', 'tsx', '--test', ...live.map((suite) => suite.path)], {
    cwd: root,
    stdio: 'inherit',
  })
} catch (error) {
  process.exit(typeof error.status === 'number' ? error.status : 1)
}
