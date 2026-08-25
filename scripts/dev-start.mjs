#!/usr/bin/env node
// ---------------------------------------------------------------------------
// CulebraLuxe DEV launcher — deterministic, port-3000-safe startup.
//
// PROBLEM: stale/hung `next-server` dev processes kept port 3000 alive for
// hours, so a new `pnpm dev` silently bound an alternate port (3001/3002/...)
// while browsers/curl kept hitting the hung server on 3000.
//
// This launcher makes that impossible:
//   1. finds whatever is listening on port 3000
//   2. kills it if it is a CulebraLuxe Next dev / next-server process
//      (graceful SIGTERM first, SIGKILL after a short wait)
//   3. cleans up any other stale Next dev / next-server processes whose cwd is
//      this repository
//   4. verifies port 3000 is free (refuses to kill unrelated processes)
//   5. clears .next
//   6. starts EXACTLY ONE `next dev -p 3000`
//
// It never kills unrelated Node applications.
//
//   pnpm dev            → this launcher
//   pnpm dev:raw        → direct `next dev`
// ---------------------------------------------------------------------------
import { execSync, spawn } from 'node:child_process'
import { existsSync, rmSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const PORT = 3000
const NEXT_RE = /next dev|next-server|next[\\/]dist[\\/]bin[\\/]next/

const out = (...a) => console.log(...a)
const err = (...a) => console.error(...a)

function sh(cmd) {
  try {
    return execSync(cmd, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim()
  } catch {
    return ''
  }
}

function sleepMs(ms) {
  try {
    execSync(`sleep ${ms / 1000}`, { stdio: ['ignore', 'ignore', 'ignore'] })
  } catch {
    /* ignore */
  }
}

function commandOf(pid) {
  return sh(`ps -o command= -p ${pid}`)
}

function isNextProcess(pid) {
  return NEXT_RE.test(commandOf(pid))
}

/** cwd of a process (macOS via lsof). Empty string when it cannot be resolved. */
function processCwd(pid) {
  const raw = sh(`lsof -a -p ${pid} -d cwd -Fn`)
  const m = raw.match(/\nn(.+)/)
  return m ? m[1].trim() : ''
}

function listenersOn(port) {
  return sh(`lsof -tiTCP:${port} -sTCP:LISTEN -n -P`)
    .split('\n')
    .map((s) => s.trim())
    .filter(Boolean)
}

/** Next dev / next-server processes, flagged when they belong to this repo. */
function collectNextProcesses() {
  const rows = sh('ps -eo pid=,command=').split('\n')
  const found = []
  for (const line of rows) {
    const m = line.match(/^\s*(\d+)\s+(.*)$/)
    if (!m) continue
    const pid = m[1]
    const cmd = m[2]
    if (!NEXT_RE.test(cmd)) continue
    found.push({ pid, cmd, inRepo: processCwd(pid) === ROOT })
  }
  return found
}

function killPid(pid) {
  try {
    process.kill(Number(pid), 'SIGTERM')
  } catch {
    /* already gone */
  }
  const deadline = Date.now() + 5000
  while (Date.now() < deadline) {
    try {
      process.kill(Number(pid), 0)
      sleepMs(200)
    } catch {
      return // no longer exists
    }
  }
  try {
    execSync(`kill -9 ${pid}`, { stdio: ['ignore', 'ignore', 'ignore'] })
  } catch {
    /* ignore */
  }
}

out('CulebraLuxe DEV')

// A. Reclaim port 3000.
for (const pid of listenersOn(PORT)) {
  if (isNextProcess(pid)) {
    out(`  ✓ Stale Next server PID ${pid} stopped`)
    killPid(pid)
  } else {
    err(`  ✗ Port ${PORT} is held by a non-Next process PID ${pid} (${commandOf(pid)}).`)
    err('    Refusing to terminate an unrelated process. Free port 3000, then re-run pnpm dev.')
    process.exit(1)
  }
}

// B. Clean up any other stale Next dev processes belonging to this repo.
for (const p of collectNextProcesses()) {
  if (p.inRepo) {
    out(`  ✓ Stale Next server PID ${p.pid} stopped`)
    killPid(p.pid)
  }
}

// C. Verify port 3000 is free.
const still = listenersOn(PORT)
if (still.length > 0) {
  const pid = still[0]
  err(`  ✗ Port ${PORT} could not be reclaimed (PID ${pid}: ${commandOf(pid)}).`)
  err('    Manually free port 3000, then re-run pnpm dev.')
  process.exit(1)
}
out(`  ✓ Port ${PORT} available`)

// D. Clear .next.
if (existsSync(resolve(ROOT, '.next'))) {
  rmSync(resolve(ROOT, '.next'), { recursive: true, force: true })
}
out('  ✓ Cleared .next')

// E. Start exactly one Next dev server on port 3000.
out(`  → Starting http://localhost:${PORT}`)
const nextBin = resolve(ROOT, 'node_modules/next/dist/bin/next')
const child = spawn(process.execPath, [nextBin, 'dev', '-p', String(PORT)], {
  cwd: ROOT,
  env: process.env,
  stdio: 'inherit',
})
child.on('exit', (code) => process.exit(code ?? 0))
child.on('error', (error) => {
  err('  ✗ Failed to start Next dev:', error.message)
  process.exit(1)
})
