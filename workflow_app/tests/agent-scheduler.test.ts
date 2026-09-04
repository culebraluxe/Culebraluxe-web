import { test } from 'node:test'
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import {
  CADENCE_SECONDS,
  escapeXml,
  LABEL,
  renderPlist,
} from '../../scripts/agent-scheduler.mjs'

// ---------------------------------------------------------------------------
// Agent worker scheduler tests — the local launchd LaunchAgent + wrapper that
// periodically invokes `pnpm agent:work`. No database, no launchd, no network:
// the pure plist renderer and the wrapper's plumbing (logs, lock, exit-code
// propagation, dry-run) are exercised as subprocesses against temp dirs. Queue
// semantics themselves remain covered by db/agent-work tests (the DB is the
// authoritative concurrency guard).
// ---------------------------------------------------------------------------

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..')
const WRAPPER = join(REPO_ROOT, 'scripts', 'agent-worker-once.sh')
const SCHEDULER = join(REPO_ROOT, 'scripts', 'agent-scheduler.mjs')

function runWrapper(env: Record<string, string>) {
  return spawnSync('/bin/bash', [WRAPPER], {
    env: { ...process.env, ...env },
    encoding: 'utf8',
  })
}

function makeTempHome(): string {
  return mkdtempSync(join(tmpdir(), 'agent-scheduler-'))
}

function invocationLog(logDir: string): string {
  const p = join(logDir, 'agent-worker.invocations.log')
  assert.ok(existsSync(p), `expected invocation log at ${p}`)
  return readFileSync(p, 'utf8')
}

test('scheduler plist template renders a 5-minute LaunchAgent without leftover placeholders', () => {
  const templatePath = join(REPO_ROOT, 'scripts', `${LABEL}.plist.template`)
  const template = readFileSync(templatePath, 'utf8')
  const out = renderPlist({
    repo: '/Users/alice/Culebraluxe-web',
    home: '/Users/alice',
    logDir: '/Users/alice/Library/Logs/CulebraLuxe',
    supportDir: '/Users/alice/Library/Application Support/CulebraLuxe',
    template,
  })
  assert.ok(!/{{/.test(out), 'render must leave no placeholders')
  assert.ok(out.includes(`<string>${LABEL}</string>`))
  assert.ok(out.includes(`<integer>${CADENCE_SECONDS}</integer>`))
  assert.ok(
    out.includes('<string>/Users/alice/Library/Application Support/CulebraLuxe/agent-worker-once.sh</string>'),
  )
  assert.ok(out.includes('<key>AGENT_WORKER_REPO</key>'))
  assert.ok(out.includes('<string>/Users/alice/Culebraluxe-web</string>'))
  assert.ok(out.includes('<string>/Users/alice/Library/Logs/CulebraLuxe/agent-worker.out.log</string>'))
})

test('renderPlist XML-escapes path values', () => {
  const template =
    '<string>{{REPO_ROOT}}</string><string>{{LOG_DIR}}</string><string>{{HOME}}</string><string>{{SUPPORT_DIR}}</string>'
  const out = renderPlist({
    repo: '/a&b<c>d',
    home: "/o'h",
    logDir: '/l"m',
    supportDir: '/s&p',
    template,
  })
  assert.equal(
    out,
    '<string>/a&amp;b&lt;c&gt;d</string><string>/l&quot;m</string><string>/o&apos;h</string><string>/s&amp;p</string>',
  )
})

test('escapeXml handles the five XML entities', () => {
  assert.equal(escapeXml(`<&>'"`), '&lt;&amp;&gt;&apos;&quot;')
  assert.equal(escapeXml('plain-path'), 'plain-path')
})

test('wrapper dry-run records the dry-run and exits 0 without claiming work', () => {
  const home = makeTempHome()
  const logDir = join(home, 'logs')
  try {
    const r = runWrapper({
      AGENT_WORKER_DRY_RUN: '1',
      AGENT_WORKER_LOG_DIR: logDir,
    })
    assert.equal(r.status, 0, r.stderr)
    const log = invocationLog(logDir)
    assert.match(log, /dry-run/)
    assert.ok(!/start: cwd/.test(log), 'dry-run exits before Forge work starts')
  } finally {
    rmSync(home, { recursive: true, force: true })
  }
})

test('deployed wrapper copy uses AGENT_WORKER_REPO to enter the repository', () => {
  const home = makeTempHome()
  const logDir = join(home, 'logs')
  const supportDir = join(home, 'Application Support', 'CulebraLuxe')
  try {
    mkdirSync(supportDir, { recursive: true })
    const deployed = join(supportDir, 'agent-worker-once.sh')
    const source = readFileSync(WRAPPER, 'utf8')
    writeFileSync(deployed, source, { mode: 0o755 })

    const r = spawnSync('/bin/bash', [deployed], {
      env: {
        ...process.env,
        AGENT_WORKER_DRY_RUN: '1',
        AGENT_WORKER_LOG_DIR: logDir,
        AGENT_WORKER_REPO: REPO_ROOT,
      },
      encoding: 'utf8',
    })
    assert.equal(r.status, 0, r.stderr)
    const log = invocationLog(logDir)
    assert.match(log, /dry-run/)
    assert.ok(!/start: cwd/.test(log), 'deployed dry-run does not start Forge work')
  } finally {
    rmSync(home, { recursive: true, force: true })
  }
})

test('wrapper local lock skips a second overlapping invocation', () => {
  const home = makeTempHome()
  const logDir = join(home, 'logs')
  try {
    const lockDir = join(logDir, 'agent-worker.lock')
    mkdirSync(lockDir, { recursive: true })
    writeFileSync(join(lockDir, 'pid'), String(process.pid))

    const r = runWrapper({
      AGENT_WORKER_DRY_RUN: '1',
      AGENT_WORKER_LOG_DIR: logDir,
    })
    assert.equal(r.status, 0, r.stderr)
    const log = invocationLog(logDir)
    assert.match(log, /skipped: another Forge worker invocation is still running/)
    assert.ok(!/start: cwd/.test(log), 'blocked invocation must not start work')
  } finally {
    rmSync(home, { recursive: true, force: true })
  }
})

test('wrapper reclaims a stale lock held by a dead pid', () => {
  const home = makeTempHome()
  const logDir = join(home, 'logs')
  try {
    const lockDir = join(logDir, 'agent-worker.lock')
    mkdirSync(lockDir, { recursive: true })
    writeFileSync(join(lockDir, 'pid'), '999999')
    const r = runWrapper({
      AGENT_WORKER_DRY_RUN: '1',
      AGENT_WORKER_LOG_DIR: logDir,
    })
    assert.equal(r.status, 0, r.stderr)
    const log = invocationLog(logDir)
    assert.match(log, /dry-run/)
    assert.ok(!existsSync(lockDir), 'stale lock must be cleaned up')
  } finally {
    rmSync(home, { recursive: true, force: true })
  }
})

test('wrapper propagates a pnpm-unavailable failure and records it without starting Forge', () => {
  const home = makeTempHome()
  const logDir = join(home, 'logs')
  try {
    const r = runWrapper({
      AGENT_WORKER_PATH: '/usr/bin:/bin',
      AGENT_WORKER_LOG_DIR: logDir,
    })
    assert.equal(r.status, 127, r.stderr)
    assert.match(r.stderr, /pnpm not found/)
    const log = invocationLog(logDir)
    assert.ok(!/start: cwd/.test(log), 'missing pnpm fails before Forge work starts')
    assert.match(log, /end: exit=127 pnpm-missing/)
  } finally {
    rmSync(home, { recursive: true, force: true })
  }
})

test('scheduler status reports not installed when no plist exists', () => {
  const home = makeTempHome()
  try {
    const r = spawnSync(process.execPath, [SCHEDULER, 'status'], {
      env: {
        ...process.env,
        CULEBRALUXE_LAUNCHAGENTS_DIR: join(home, 'LaunchAgents'),
        AGENT_WORKER_LOG_DIR: join(home, 'logs'),
      },
      encoding: 'utf8',
    })
    assert.equal(r.status, 0, r.stderr)
    assert.match(r.stdout, /not installed \(plist missing\)/)
  } finally {
    rmSync(home, { recursive: true, force: true })
  }
})
