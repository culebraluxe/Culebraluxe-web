#!/usr/bin/env node
// ---------------------------------------------------------------------------
// scripts/calendar-sync-agent.mjs — manage the macOS LaunchAgent that
// periodically invokes the existing EventKit snapshot bridge.
//
// Calendar deliberately reuses the EXISTING FDA-grantable apple-sync-launcher
// as its trusted macOS/TCC execution boundary. This file never builds, replaces,
// or modifies that proven launcher or the canonical Apple Messages sync.
//
//   pnpm calendar:sync:install     deploy wrapper + install/bootstrap (idempotent)
//   pnpm calendar:sync:status      loaded/enabled state + last invocation log
//   pnpm calendar:sync:run         run the exact same launcher path once
//   pnpm calendar:sync:stop        kill switch: boot out + persist disabled
//   pnpm calendar:sync:uninstall   stop + delete Calendar plist/wrapper only
// ---------------------------------------------------------------------------

import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

export const LABEL = 'com.culebraluxe.calendar-sync'
export const CADENCE_SECONDS = Number(
  process.env.CALENDAR_SYNC_CADENCE_SECONDS || 1800,
)
const LAUNCHCTL_TIMEOUT_MS = 10000

export function repoRoot() {
  return join(dirname(fileURLToPath(import.meta.url)), '..')
}

export function machinePaths(env = process.env) {
  const home = env.HOME || homedir()
  const launchAgentsDir =
    env.CULEBRALUXE_LAUNCHAGENTS_DIR || join(home, 'Library', 'LaunchAgents')
  const supportDir =
    env.CULEBRALUXE_SUPPORT_DIR ||
    join(home, 'Library', 'Application Support', 'CulebraLuxe')
  const logDir =
    env.CULEBRALUXE_CALENDAR_LOG_DIR || join(home, 'Library', 'Logs', 'CulebraLuxe')
  const snapshot = env.MAC_BRIDGE_CALENDAR_JSON || '/tmp/culebraluxe-calendar.json'
  const uid = typeof process.getuid === 'function' ? process.getuid() : 501
  return {
    repo: repoRoot(),
    home,
    launchAgentsDir,
    supportDir,
    logDir,
    snapshot,
    plistPath: join(launchAgentsDir, `${LABEL}.plist`),
    invocationLog: join(logDir, 'calendar-sync.invocations.log'),
    wrapper: join(repoRoot(), 'scripts', 'macbridge', 'sync-calendar-eventkit.sh'),
    deployedWrapper: join(supportDir, 'calendar-sync-once.sh'),
    appleLauncher: join(supportDir, 'apple-sync-launcher'),
    uid,
    target: `gui/${uid}`,
    job: `gui/${uid}/${LABEL}`,
  }
}

export function escapeXml(value) {
  return String(value).replace(/[<>&'\"]/g, (ch) => {
    switch (ch) {
      case '<': return '&lt;'
      case '>': return '&gt;'
      case '&': return '&amp;'
      case "'": return '&apos;'
      case '\"': return '&quot;'
      default: return ch
    }
  })
}

export function renderPlist({
  repo,
  home,
  logDir,
  supportDir,
  snapshot,
  appleLauncher,
  label = LABEL,
  cadenceSeconds = CADENCE_SECONDS,
  template,
}) {
  return template
    .replace(/\{\{LABEL\}\}/g, escapeXml(label))
    .replace(/\{\{REPO_ROOT\}\}/g, escapeXml(repo))
    .replace(/\{\{SUPPORT_DIR\}\}/g, escapeXml(supportDir))
    .replace(/\{\{HOME\}\}/g, escapeXml(home))
    .replace(/\{\{LOG_DIR\}\}/g, escapeXml(logDir))
    .replace(/\{\{SNAPSHOT\}\}/g, escapeXml(snapshot))
    .replace(/\{\{APPLE_LAUNCHER\}\}/g, escapeXml(appleLauncher))
    .replace(/\{\{CADENCE_SECONDS\}\}/g, String(cadenceSeconds))
}

function launchctl(args) {
  return spawnSync('/bin/launchctl', args, {
    encoding: 'utf8',
    timeout: LAUNCHCTL_TIMEOUT_MS,
  })
}

function commandDetail(result) {
  if (result?.error) return result.error.message
  return result?.stderr?.trim() || result?.stdout?.trim() || `exit ${result?.status ?? 'unknown'}`
}

function failTimedOutLaunchctl(step, result) {
  if (result?.error?.code === 'ETIMEDOUT') {
    console.error(`[calendar-sync] ${step} timed out after ${LAUNCHCTL_TIMEOUT_MS / 1000}s`)
    process.exit(1)
  }
}

function deployWrapper(p) {
  if (!existsSync(p.wrapper)) {
    console.error(`calendar-sync wrapper missing: ${p.wrapper}`)
    process.exit(1)
  }
  mkdirSync(p.supportDir, { recursive: true })
  const source = readFileSync(p.wrapper, 'utf8')
  writeFileSync(p.deployedWrapper, source, { mode: 0o755 })
}

function requireSharedAppleLauncher(p) {
  if (!existsSync(p.appleLauncher)) {
    console.error(`existing Apple sync launcher missing: ${p.appleLauncher}`)
    console.error('Calendar install will not rebuild or replace the proven Apple launcher.')
    process.exit(1)
  }
}

function install() {
  const p = machinePaths()

  console.log('[calendar-sync] 1/6 preparing directories')
  mkdirSync(p.logDir, { recursive: true })

  console.log('[calendar-sync] 2/6 deploying Calendar wrapper + verifying shared Apple launcher')
  deployWrapper(p)
  requireSharedAppleLauncher(p)

  console.log('[calendar-sync] 3/6 rendering LaunchAgent plist')
  const templatePath = join(
    p.repo,
    'scripts',
    'com.culebraluxe.calendar-sync.plist.template',
  )
  const template = readFileSync(templatePath, 'utf8')
  const plist = renderPlist({
    repo: p.repo,
    home: p.home,
    logDir: p.logDir,
    supportDir: p.supportDir,
    snapshot: p.snapshot,
    appleLauncher: p.appleLauncher,
    template,
  })
  writeFileSync(p.plistPath, plist, { mode: 0o644 })

  console.log('[calendar-sync] 4/6 validating plist')
  const lint = spawnSync('/usr/bin/plutil', ['-lint', p.plistPath], {
    encoding: 'utf8',
    timeout: 10000,
  })
  if (lint.error?.code === 'ETIMEDOUT') {
    console.error('[calendar-sync] plutil timed out after 10s')
    process.exit(1)
  }
  if (lint.status !== 0) {
    console.error(`plutil rejected generated plist:\n${lint.stdout}${lint.stderr}`)
    process.exit(1)
  }

  console.log('[calendar-sync] 5/6 resetting prior Calendar LaunchAgent state')
  const bootout = launchctl(['bootout', p.job])
  failTimedOutLaunchctl('launchctl bootout', bootout)

  const enable = launchctl(['enable', p.job])
  failTimedOutLaunchctl('launchctl enable', enable)
  if (enable.status !== 0) {
    console.error(`[calendar-sync] launchctl enable failed: ${commandDetail(enable)}`)
    process.exit(1)
  }

  console.log('[calendar-sync] 6/6 bootstrapping Calendar LaunchAgent via shared Apple launcher')
  const boot = launchctl(['bootstrap', p.target, p.plistPath])
  failTimedOutLaunchctl('launchctl bootstrap', boot)
  if (boot.status !== 0) {
    console.error(`[calendar-sync] launchctl bootstrap failed: ${commandDetail(boot)}`)
    process.exit(1)
  }

  console.log('[calendar-sync] installed + enabled')
  printStatus()
}

function printStatus() {
  const p = machinePaths()
  const loaded = launchctl(['print', p.job])
  failTimedOutLaunchctl('launchctl print', loaded)
  console.log('label:', LABEL)
  console.log('job:', p.job)
  console.log('plist:', p.plistPath)
  console.log('launcher:', p.appleLauncher, existsSync(p.appleLauncher) ? '(shared, present)' : '(shared, MISSING)')
  console.log('wrapper:', p.deployedWrapper)
  console.log('snapshot:', p.snapshot)
  console.log('cadenceSeconds:', CADENCE_SECONDS)
  console.log('loaded/enabled:', loaded.status === 0 ? 'yes' : 'no (not loaded or disabled)')
  if (existsSync(p.invocationLog)) {
    const lines = readFileSync(p.invocationLog, 'utf8').trim().split('\n')
    console.log('last invocation log lines:')
    for (const line of lines.slice(-6)) console.log('  ' + line)
  } else {
    console.log('log:', p.invocationLog, '(no invocations yet)')
  }
  if (existsSync(p.snapshot)) {
    const out = spawnSync('stat', ['-f', 'snapshot generated-at: %Sm', p.snapshot], {
      encoding: 'utf8',
      timeout: 10000,
    })
    if (out.status === 0) console.log(out.stdout.trim())
    else console.log('snapshot exists (generated-at unavailable)')
  } else {
    console.log('snapshot: (missing) — web app degrades gracefully')
  }
}

function runOnce() {
  const p = machinePaths()
  requireSharedAppleLauncher(p)
  if (!existsSync(p.deployedWrapper)) {
    console.error(`Calendar wrapper missing: ${p.deployedWrapper} (run calendar:sync:install first)`)
    process.exit(1)
  }
  const env = {
    ...process.env,
    CULEBRALUXE_REPO: p.repo,
    CULEBRALUXE_APPLE_SYNC_SCRIPT: p.deployedWrapper,
  }
  const r = spawnSync(p.appleLauncher, [], { stdio: 'inherit', env })
  process.exit(r.status ?? 1)
}

function stop() {
  const p = machinePaths()
  const bootout = launchctl(['bootout', p.job])
  failTimedOutLaunchctl('launchctl bootout', bootout)
  const disable = launchctl(['disable', p.job])
  failTimedOutLaunchctl('launchctl disable', disable)
  console.log(`stopped: no future scheduled Calendar invocations (plist kept at ${p.plistPath}).`)
}

function uninstall() {
  const p = machinePaths()
  const bootout = launchctl(['bootout', p.job])
  failTimedOutLaunchctl('launchctl bootout', bootout)
  const disable = launchctl(['disable', p.job])
  failTimedOutLaunchctl('launchctl disable', disable)
  if (existsSync(p.plistPath)) rmSync(p.plistPath)
  if (existsSync(p.deployedWrapper)) rmSync(p.deployedWrapper)
  console.log('uninstalled Calendar sync only: Calendar LaunchAgent + Calendar wrapper removed.')
  console.log('shared Apple launcher and canonical Apple sync were not touched.')
}

function main() {
  const command = process.argv[2]
  switch (command) {
    case 'install': install(); break
    case 'status': printStatus(); break
    case 'run': runOnce(); break
    case 'stop': stop(); break
    case 'uninstall': uninstall(); break
    case 'help':
    case '--help':
    case '-h':
      console.log(`usage: node scripts/calendar-sync-agent.mjs <command>
commands:
  install     deploy Calendar wrapper + install/bootstrap via existing Apple launcher
  status      show loaded/enabled state, shared launcher, snapshot, last invocation log
  run         run the exact shared-launcher Calendar path once
  stop        kill switch: boot out + persist disabled (no future Calendar runs)
  uninstall   remove Calendar job/wrapper only; never remove shared Apple launcher`)
      break
    default:
      console.error(`unknown command: ${command ?? '(none)'}`)
      console.error('try: node scripts/calendar-sync-agent.mjs help')
      process.exit(2)
  }
}

if (fileURLToPath(import.meta.url) === process.argv[1]) {
  main()
}
