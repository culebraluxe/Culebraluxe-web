#!/usr/bin/env node
import { readdirSync } from 'node:fs'
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
  console.error(`runtime boundary: FAIL (${roots.length} entrypoints)`)
  process.exit(run.status ?? 1)
}
console.log(`runtime boundary: PASS (${roots.length} entrypoints; no legacy/workflow_engine/agent-runtime dependency)`)
