#!/usr/bin/env node
// ---------------------------------------------------------------------------
// Broken-TypeScript sweep — the health metric behind docs/agent/BROKEN-TS-INVENTORY.md
//
// Answers ONE question by static resolution, with no database and no network: which files in
// this repository cannot load because a module they import is gone?
//
// Two classes, because the distinction is real and was measured the hard way:
//   CANNOT LOAD — a value import (directly, or through another broken file) resolves to nothing.
//                 `import type` is deliberately ignored: tsx erases it, so it cannot break loading.
//   CANNOT WORK — the file loads, but a module it imports lazily is gone, so the code path it
//                 needs does not exist. It is still broken; it is not the same sentence.
//
// Exits 1 when a file is broken but unmarked, or marked but loadable — i.e. when the inventory in
// docs/agent/BROKEN-TS-INVENTORY.md has drifted from the tree. Exits 0 when the two agree.
//
// Usage: node scripts/broken-ts-sweep.mjs [roots...]   (default: scripts agent-runtime)
// ---------------------------------------------------------------------------
import fs from 'node:fs'
import path from 'node:path'

const root = process.cwd()
const ROOTS = process.argv.slice(2).length ? process.argv.slice(2) : ['scripts', 'agent-runtime']
const EXTS = ['', '.ts', '.tsx', '.mts', '.mjs', '.js', '.jsx', '.json']
// Assembled, not written literally: a file that contains the literal would otherwise mark
// itself as bannered (this tool did exactly that on its first run).
const MARKER = ['//', '⚠', 'BROKEN ON PURPOSE'].join(' ')

function resolveSpec(spec, fromDir) {
  if (spec.startsWith('node:') || spec.startsWith('bun:')) return { external: true }
  let base
  if (spec.startsWith('@/')) base = path.join(root, spec.slice(2))
  else if (spec.startsWith('.')) base = path.resolve(fromDir, spec)
  else {
    const first = spec.startsWith('@') ? spec.split('/').slice(0, 2).join('/') : spec.split('/')[0]
    if (fs.existsSync(path.join(root, 'node_modules', first))) return { external: true }
    base = path.join(root, spec)
  }
  for (const e of EXTS) {
    const c = base + e
    if (fs.existsSync(c) && fs.statSync(c).isFile()) return { file: fs.realpathSync(c) }
  }
  for (const e of EXTS.slice(1)) {
    const c = path.join(base, 'index' + e)
    if (fs.existsSync(c) && fs.statSync(c).isFile()) return { file: fs.realpathSync(c) }
  }
  return { missing: true }
}

const SPEC =
  /(?:^|[^.\w])import\s+(?:type\s+)?(?:[\s\S]*?)\s*from\s*['"]([^'"]+)['"]|^\s*import\s+['"]([^'"]+)['"]/gm
const DYNAMIC_SPEC = /\bimport\(\s*['"]([^'"]+)['"]\s*\)/g

function valueSpecs(file) {
  const text = fs.readFileSync(file, 'utf8')
  const out = []
  const re = new RegExp(SPEC.source, 'gm')
  let m
  while ((m = re.exec(text))) {
    const spec = m[1] || m[2]
    if (!spec) continue
    const lineStart = text.lastIndexOf('\n', m.index) + 1
    const stmt = text.slice(lineStart, text.indexOf('\n', m.index + m[0].length))
    if (/^\s*import\s+type\s/m.test(stmt)) continue
    out.push({ spec, line: text.slice(0, m.index).split('\n').length })
  }
  return out
}

function walk(dir, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name)
    if (e.isDirectory()) {
      if (e.name === 'node_modules' || e.name.startsWith('.')) continue
      walk(p, out)
    } else if (/\.(ts|mts|mjs|cjs)$/.test(e.name)) out.push(p)
  }
  return out
}

const all = ROOTS.map((r) => path.join(root, r)).flatMap((p) => (fs.existsSync(p) ? walk(p) : []))
const dead = new Map()
const alive = new Set()
const visiting = new Set()

function analyse(file) {
  if (alive.has(file)) return false
  if (dead.has(file)) return true
  if (visiting.has(file)) return false
  visiting.add(file)
  for (const { spec, line } of valueSpecs(file)) {
    const r = resolveSpec(spec, path.dirname(file))
    if (r.external) continue
    if (r.missing) {
      visiting.delete(file)
      dead.set(file, `missing module '${spec}' (line ${line})`)
      return true
    }
    if (analyse(r.file)) {
      visiting.delete(file)
      dead.set(file, `imports dead '${spec}' -> ${path.relative(root, r.file)} (line ${line})`)
      return true
    }
  }
  visiting.delete(file)
  alive.add(file)
  return false
}

for (const f of all) analyse(f)

const runtime = new Map()
for (const f of all) {
  if (dead.has(f)) continue
  const text = fs.readFileSync(f, 'utf8')
  const re = new RegExp(DYNAMIC_SPEC.source, 'g')
  let m
  while ((m = re.exec(text))) {
    const spec = m[1]
    if (!spec) continue
    const r = resolveSpec(spec, path.dirname(f))
    if (r.external) continue
    if (r.missing || (r.file && dead.has(r.file))) {
      runtime.set(f, `dynamic import '${spec}' is gone`)
      break
    }
  }
}

const marked = all.filter((f) => fs.readFileSync(f, 'utf8').includes(MARKER))
const expected = new Set([...dead.keys(), ...runtime.keys()])
const unmarked = all.filter((f) => expected.has(f) && !marked.includes(f))
const overmarked = marked.filter((f) => !expected.has(f))
const rel = (f) => path.relative(root, f)

console.log(`scanned                        : ${all.length} files`)
console.log(`cannot load                    : ${dead.size}`)
console.log(`loads, but lazy target gone     : ${runtime.size}`)
console.log(`marked "${MARKER}"    : ${marked.length}`)

if (unmarked.length) {
  console.log('\nDRIFT — broken but NOT marked:')
  for (const f of unmarked) console.log(`  ${rel(f)}  <- ${dead.get(f) || runtime.get(f)}`)
}
if (overmarked.length) {
  console.log('\nDRIFT — marked but loads fine:')
  for (const f of overmarked) console.log(`  ${rel(f)}`)
}
if (!unmarked.length && !overmarked.length) console.log('\nthe tree and the inventory agree')
process.exit(unmarked.length + overmarked.length ? 1 : 0)
