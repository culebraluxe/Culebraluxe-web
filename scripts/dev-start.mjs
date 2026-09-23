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
//   6. starts EXACTLY ONE `next dev --webpack -p 3000`
//
// Local DEV intentionally uses Webpack. Next 16 Turbopack has produced an
// intermittent internal Google-font resolver failure in this project
// (`@vercel/turbopack-next/internal/font/google/font`). Webpack is the stable
// local path and matches the build mode used for targeted verification.
//
// It never kills unrelated Node applications.
//
//   pnpm dev            → loads .env.local, starts fresh Rust API, then this launcher (Webpack)
//   pnpm dev:raw        → direct `next dev` (Next default / Turbopack)
// ---------------------------------------------------------------------------
import { execSync, spawn } from 'node:child_process'
import { existsSync, readdirSync, rmSync, statSync } from 'node:fs'
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

/**
 * The process state field (`ps -o stat=`): `T` means STOPPED, which is what Ctrl-Z leaves behind.
 *
 * Worth its own function because it changes which signal works, not just how a message reads: a stopped process never
 * runs a SIGTERM handler, so the port stays held and the next dev server fails to bind — while the browser keeps
 * talking to a server that accepts a connection and never answers.
 */
function processState(pid) {
  return sh(`ps -o stat= -p ${pid}`).trim()
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

function pidExists(pid) {
  if (!pid) return false
  try {
    process.kill(Number(pid), 0)
    return true
  } catch {
    return false
  }
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
  // A STOPPED PROCESS CANNOT BE KILLED BY SIGTERM. Ctrl-Z suspends the dev server with SIGTSTP, and a suspended process
  // does not run its signal handler — so SIGTERM sits queued, the process keeps the port, and the next `pnpm dev` fails
  // to bind while the browser talks to something that accepts a connection and never answers. That presents as a blank
  // white page in a normal AND a private window, which is indistinguishable from a broken UI and cost an hour today.
  // SIGKILL cannot be caught or deferred, so it works on a stopped process; it is the right tool for exactly one case.
  if (processState(pid).includes('T')) {
    out(`  ✓ Suspended Next server PID ${pid} (Ctrl-Z) force-stopped`)
    try {
      process.kill(Number(pid), 'SIGKILL')
    } catch {
      /* already gone */
    }
  }
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

// D1. Rust UI freshness: the browser runs the WASM, so a WASM older than `rust/ui/src/**` is a bug in whatever screen
// you are looking at, and it presents as a Rust bug you cannot reproduce in the source you are reading.
//
// EVERY UI BUG IN THIS PORT WAS DIAGNOSED AGAINST STALE WASM AT LEAST ONCE. `cargo check` proves the source compiles;
// it says nothing about what the browser is running, and neither does a green build of a crate nobody rebuilt. So the
// comparison is made here, every dev session, and it is a FAILURE rather than a warning: a warning that scrolls past
// in startup output is how this went unnoticed, and the failure costs ten seconds to clear.
{
  const gluePath = resolve(ROOT, 'lib/rust-ui/ui.js')
  const wasmPath = resolve(ROOT, 'public/rust-ui/ui_bg.wasm')
  const sourceDir = resolve(ROOT, 'rust/ui')

  /** The newest mtime under a directory, for files the predicate accepts. */
  const newestIn = (dir, accept) => {
    let newest = { path: '', mtime: 0 }
    const walk = (current) => {
      for (const entry of readdirSync(current, { withFileTypes: true })) {
        if (entry.name === 'target' || entry.name === 'node_modules') continue
        const full = resolve(current, entry.name)
        if (entry.isDirectory()) {
          walk(full)
          continue
        }
        if (!accept(entry.name)) continue
        const { mtimeMs } = statSync(full)
        if (mtimeMs > newest.mtime) newest = { path: full, mtime: mtimeMs }
      }
    }
    walk(dir)
    return newest
  }

  const source = newestIn(sourceDir, (name) => name.endsWith('.rs') || name === 'Cargo.toml')
  // Both generated files matter: the glue is what imports the wasm, so stale glue loads a stale module just as happily.
  const artifacts = [
    { label: 'lib/rust-ui/ui.js', path: gluePath },
    { label: 'public/rust-ui/ui_bg.wasm', path: wasmPath },
  ]
  const missing = artifacts.filter((artifact) => !existsSync(artifact.path))
  const stale = artifacts.filter(
    (artifact) => existsSync(artifact.path) && statSync(artifact.path).mtimeMs < source.mtime,
  )

  if (source.path && (missing.length > 0 || stale.length > 0)) {
    err('  ✗ The Rust UI the browser will run is not built from the source in this repo.')
    err(`    newest source:  ${source.path.replace(`${ROOT}/`, '')}  (${new Date(source.mtime).toISOString()})`)
    for (const artifact of missing) err(`    missing:        ${artifact.label}`)
    for (const artifact of stale) {
      err(
        `    stale:          ${artifact.label}  (${new Date(statSync(artifact.path).mtimeMs).toISOString()})`,
      )
    }
    err('    Build it, and commit the artifacts with the source:')
    err('      pnpm ui:build:release')
    if (process.env.RUST_UI_ALLOW_STALE === '1') {
      err('    RUST_UI_ALLOW_STALE=1 is set, so starting anyway. What you are testing is NOT this source.')
    } else {
      err('    To start anyway (not recommended): RUST_UI_ALLOW_STALE=1 pnpm dev')
      process.exit(1)
    }
  } else {
    out('  ✓ Rust UI WASM is newer than rust/ui/src')
  }
}

// D2. Bounce the Rust API: the cut-over routes call it through lib/rust-api/client.ts, so it must be RUNNING and
// FRESH for every dev session. An already-listening instance is stopped and restarted rather than reused, because a
// stale build of that binary behaves exactly like a bug in the screen that called it.
const RUST_API_PORT = Number(process.env.RUST_API_PORT ?? 8080)
const LOCAL_APP_ENV =
  process.env.APP_ENV?.trim() || process.env.VERCEL_ENV?.trim() ? undefined : 'dev'
let rustApi = null

const RUST_API_RE = /culebraluxe rust api|--bin http|\/http\b|target\/[^/]+\/http\b/

for (const pid of listenersOn(RUST_API_PORT)) {
  if (RUST_API_RE.test(commandOf(pid))) {
    out(`  ✓ Bouncing stale Rust API PID ${pid} on ${RUST_API_PORT}`)
    killPid(pid)
  } else {
    err(`  ✗ Port ${RUST_API_PORT} is held by a non-Rust-API process PID ${pid} (${commandOf(pid)}).`)
    err('    Refusing to terminate an unrelated process. Free that port, then re-run pnpm dev.')
    process.exit(1)
  }
}

if (listenersOn(RUST_API_PORT).length > 0) {
  err(`  ✗ Port ${RUST_API_PORT} could not be reclaimed for the Rust API.`)
  process.exit(1)
}

{
  const cargoTarget = process.env.CARGO_TARGET_DIR ?? resolve(ROOT, 'rust/target')
  const rustEnv = {
    ...process.env,
    ...(LOCAL_APP_ENV ? { APP_ENV: LOCAL_APP_ENV } : {}),
    CARGO_TARGET_DIR: cargoTarget,
    RUST_API_BIND: `127.0.0.1:${RUST_API_PORT}`,
  }

  if (LOCAL_APP_ENV) {
    out('  ✓ No APP_ENV/VERCEL_ENV declared; local Rust API will use APP_ENV=dev')
  }

  rustApi = spawn('cargo', ['run', '--quiet', '-p', 'server', '--bin', 'http'], {
    cwd: resolve(ROOT, 'rust'),
    env: rustEnv,
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  rustApi.stdout?.on('data', (chunk) => process.stdout.write(`  [rust-api] ${chunk}`))
  rustApi.stderr?.on('data', (chunk) => process.stderr.write(`  [rust-api] ${chunk}`))
  rustApi.on('error', (error) => err('  ✗ Failed to start the Rust API:', error.message))
  out(`  → Starting Rust API on ${RUST_API_PORT} (fresh; first run compiles)`)

  // Do not start Next against a dead bridge. The old launcher spawned Rust and immediately continued, so a missing
  // APP_ENV could kill the API while localhost:3000 still looked healthy. Flight Recorder then surfaced only "fetch
  // failed", which falsely looked like a screen/data defect. Wait for the listener and fail the entire DEV launch if
  // the Rust child exits first.
  const deadline = Date.now() + 90_000
  while (Date.now() < deadline) {
    if (listenersOn(RUST_API_PORT).length > 0) break
    if (!pidExists(rustApi.pid)) {
      err('  ✗ Rust API exited before opening its port. DEV startup aborted.')
      err('    Read the [rust-api] error above; Next was intentionally not started.')
      process.exit(1)
    }
    sleepMs(250)
  }

  if (listenersOn(RUST_API_PORT).length === 0) {
    err(`  ✗ Rust API did not open port ${RUST_API_PORT} within the startup window.`)
    stopRustApi()
    process.exit(1)
  }
  out(`  ✓ Rust API listening on ${RUST_API_PORT}`)
}

function stopRustApi() {
  if (rustApi && rustApi.exitCode === null) {
    try {
      rustApi.kill('SIGTERM')
    } catch {
      /* already gone */
    }
  }
}

let shuttingDown = false
rustApi?.on('exit', (code, signal) => {
  if (shuttingDown) return
  err(`  ✗ Rust API stopped while DEV was running (code=${code ?? 'null'}, signal=${signal ?? 'none'}).`)
  err('    Stopping DEV rather than serving a half-alive application.')
  process.exit(code ?? 1)
})

process.on('SIGINT', () => {
  shuttingDown = true
  stopRustApi()
  process.exit(0)
})
process.on('SIGTERM', () => {
  shuttingDown = true
  stopRustApi()
  process.exit(0)
})

// E. Start exactly one Next dev server on port 3000 using Webpack.
out(`  → Starting http://localhost:${PORT} (Webpack)`)
const nextBin = resolve(ROOT, 'node_modules/next/dist/bin/next')
const child = spawn(process.execPath, [nextBin, 'dev', '--webpack', '-p', String(PORT)], {
  cwd: ROOT,
  env: process.env,
  stdio: 'inherit',
})
child.on('exit', (code) => {
  stopRustApi()
  process.exit(code ?? 0)
})
child.on('error', (error) => {
  err('  ✗ Failed to start Next dev:', error.message)
  process.exit(1)
})
