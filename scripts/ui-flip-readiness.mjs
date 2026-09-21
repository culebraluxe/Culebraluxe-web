#!/usr/bin/env node
// Which portal screens can be flipped to the Rust UI, and why the rest cannot be — computed from the code.
//
// The UI layer doc states three conditions for a flip. Two of them are mechanical, so this script checks them instead
// of asking anyone to remember:
//
//   rows     does `app/api/portal/rust-ui/rows/route.ts` have a loader for this screen, i.e. does the Rust screen get
//            real data? No loader means the Rust screen would render structure and "Nothing to show yet".
//   markers  interactive hooks in the TypeScript component the page renders (state, forms, dialogs, paging). A flip
//            removes whatever the Rust body does not reproduce, so markers > 0 means something would be lost.
//
// The third condition — does the Rust body carry the same *information* — is a judgement about `rust/ui/src/view.rs`
// and is deliberately not guessed at here. `dashboard` is the example: it passes both checks and is still a judgement
// call, because it is the Cockpit and has a layout of its own.
//
// Usage: node scripts/ui-flip-readiness.mjs
import { readFileSync, existsSync } from 'node:fs'
import { execSync } from 'node:child_process'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const model = readFileSync(`${root}/rust/ui/src/model.rs`, 'utf8')
const route = readFileSync(`${root}/app/api/portal/rust-ui/rows/route.ts`, 'utf8')
const publicRoute = readFileSync(`${root}/app/api/rust-ui/public-rows/route.ts`, 'utf8')

// Each screen is one line: Screen { key: "...", title: "...", path: "/portal/...", surface: ..., }
const screens = []
for (const line of model.split('\n')) {
  if (!line.includes('Screen {')) continue
  const key = line.match(/key:\s*"([^"]+)"/)?.[1]
  if (!key) continue
  screens.push({ key, path: line.match(/path:\s*"([^"]+)"/)?.[1] ?? null })
}

const loaders = new Set([
  ...[...route.matchAll(/^\s{2}'?([a-z][a-z0-9-]*)'?:\s*async/gm)].map((m) => m[1]),
  ...[...publicRoute.matchAll(/case '([a-z][a-z0-9-]*)':/g)].map((m) => m[1]),
])
const MARKERS = 'useState|useReducer|onSubmit|<form|onClick|Dialog|Modal|Drawer|onChange'

const rows = []
for (const { key, path } of screens) {
  const hasLoader = loaders.has(key)
  const page = path ? `${root}/app${path}/page.tsx` : null
  if (!page || !existsSync(page)) {
    rows.push({ key, path: path ?? '-', rows: hasLoader, markers: hasLoader ? -1 : null })
    continue
  }
  const src = readFileSync(page, 'utf8')
  // A page that already mounts the host is flipped: it has no TypeScript body left to lose.
  const flipped = src.includes('RustUiHost')
  const components = flipped ? [] : [...src.matchAll(/from\s+"(@\/components\/[^"]+)"/g)].map((m) => m[1])
  let markers = 0
  for (const c of components) {
    const file = `${root}/${c.replace('@/', '')}.tsx`
    if (!existsSync(file)) continue
    markers += Number(execSync(`grep -cE '${MARKERS}' '${file}' || true`).toString().trim() || 0)
  }
  rows.push({ key, path: path ?? '-', rows: hasLoader, markers, flipped })
}

const byMarkers = (a, b) => (a.markers ?? 99) - (b.markers ?? 99) || a.key.localeCompare(b.key)
const flipped = rows.filter((r) => r.flipped).sort(byMarkers)
const ready = rows.filter((r) => !r.flipped && r.rows && r.markers === 0).sort(byMarkers)
const blocked = rows.filter((r) => !r.flipped && r.rows && (r.markers ?? 0) > 0).sort(byMarkers)
const unwired = rows.filter((r) => !r.rows).sort(byMarkers)

console.log(`SCREENS: ${screens.length}   with a rows loader: ${rows.filter((r) => r.rows).length}\n`)
console.log(`FLIPPED (the route renders the Rust screen): ${flipped.length}`)
for (const r of flipped) console.log(`  ${r.key.padEnd(24)} ${r.path}`)
console.log(`\nREADY (real rows, nothing interactive to lose): ${ready.length}`)
for (const r of ready) console.log(`  ${r.key.padEnd(24)} ${r.path}`)
console.log(`\nBLOCKED BY TYPESCRIPT INTERACTION: ${blocked.length}`)
for (const r of blocked) console.log(`  ${r.key.padEnd(24)} ${r.path.padEnd(36)} ${r.markers} markers`)
console.log(`\nNO ROWS LOADER (structure only): ${unwired.length}`)
console.log('  ' + unwired.map((r) => r.key).join(', '))
