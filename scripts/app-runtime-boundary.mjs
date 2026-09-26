#!/usr/bin/env node
import { readFileSync, readdirSync } from 'node:fs'
import { join, relative } from 'node:path'
import { spawnSync } from 'node:child_process'

const ENTRY = /^(page|layout|route|error|not-found|loading|default)\.(?:ts|tsx|js|jsx)$/

function walk(dir) {
  const out = []
  for (const item of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, item.name)
    if (item.isDirectory()) out.push(...walk(full))
    else if (ENTRY.test(item.name)) out.push(relative(process.cwd(), full))
  }
  return out
}

const roots = walk(join(process.cwd(), 'app')).sort()

const TRACE_TARGETS = [
  'lib/service-runtime',
  'lib/commands/index',
  'lib/neon-interactive',
  'lib/mq/outbox-repository',
  'lib/property-reads',
  'lib/vault-io',
]

function traceDirectImporters() {
  const codeEntry = /\.(?:ts|tsx|js|jsx)$/
  const matches = []
  const scan = (dir) => {
    for (const item of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, item.name)
      if (item.isDirectory()) {
        scan(full)
        continue
      }
      if (!codeEntry.test(item.name)) continue
      const body = readFileSync(full, 'utf8')
      for (const target of TRACE_TARGETS) {
        if (body.includes(target)) {
          matches.push({ file: relative(process.cwd(), full), target })
        }
      }
    }
  }
  for (const root of ['app', 'lib']) scan(join(process.cwd(), root))
  if (matches.length === 0) return
  console.error('runtime boundary direct-root importers:')
  for (const match of matches.sort((a, b) => a.file.localeCompare(b.file) || a.target.localeCompare(b.target))) {
    console.error(`  ${match.file} -> ${match.target}`)
  }
}
if (roots.length === 0) {
  console.error('runtime boundary: no App Router entrypoints found')
  process.exit(1)
}

const run = spawnSync(
  'pnpm',
  ['exec', 'depcruise', ...roots, '--config', '.dependency-cruiser.runtime.js', '--output-type', 'err'],
  {
    cwd: process.cwd(),
    encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024,
  },
)

process.stdout.write(run.stdout ?? '')
process.stderr.write(run.stderr ?? '')
if (run.status !== 0) {
  traceDirectImporters()
  console.error(`runtime boundary: FAIL (${roots.length} entrypoints)`)
  process.exit(run.status ?? 1)
}
console.log(`runtime boundary: PASS (${roots.length} entrypoints; no legacy/workflow_engine/agent-runtime dependency)`)
