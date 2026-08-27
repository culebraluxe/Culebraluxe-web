#!/usr/bin/env node
// ---------------------------------------------------------------------------
// scripts/calendar-sync-agent.mjs — manage the macOS LaunchAgent that
// periodically invokes the EXISTING EventKit snapshot bridge
// (scripts/macbridge/CalendarEventKit.swift, a.k.a. `pnpm calendar:sync`).
//
//   pnpm calendar:sync:install     render + install + bootstrap (idempotent)
//   pnpm calendar:sync:status      loaded/enabled state + last invocation log
//   pnpm calendar:sync:run         run the exact same wrapper once
//   pnpm calendar:sync:stop        kill switch: boot out + persist disabled
//   pnpm calendar:sync:uninstall   stop + delete the plist
//
// This tool only maintains the periodic wake-up and lightweight logs. The web
// app's Catch-Up adapter (lib/catchup/eventkit.ts) consumes the snapshot and
// already degrades gracefully when it is missing/bad — the LaunchAgent being
// stopped, permission denied, or a bad snapshot NEVER blocks CulebraLuxe.
//
// No secrets live in tracked files or the generated plist. Env controls:
//   CALENDAR_SYNC_CADENCE_SECONDS  - StartInterval (default 1800 = 30 min)
//   CULEBRALUXE_LAUNCHAGENTS_DIR   - where the plist is installed
//   CULEBRALUXE_SUPPORT_DIR        - where the deployed wrapper lives
//   CULEBRALUXE_CALENDAR_LOG_DIR   - where logs live
//   MAC_BRIDGE_CALENDAR_JSON       - snapshot path (default /tmp/culebraluxe-calendar.json)
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
    uid,
    target: `gui/${uid}`,
    job: `gui/${uid}/${LABEL}`,
  }
}

export function escapeXml(value) {
  return String(value).replace(/[<>&'"]/g, (ch) => {
    switch (ch) {
      case '<':
        return '&lt;'
      case '>':
        return '&gt;'
      case '&':
        return '&amp;'
      case "'":
        return '&apos;'
      case '"':
        return '&quot;'
      default:
        return ch
    }
  })
}

export function renderPlist({
  repo,
  home,
  logDir,
  supportDir,
  snapshot,
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
    .replace(/\{\{CADENCE_SECONDS\}\}/g, String(cadenceSeconds))
}

function launchctl(args) {
  return spawnSync('/bin/launchctl', args, { encoding: 'utf8' })
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

function install() {
  const p = machinePaths()
  mkdirSync(p.logDir, { recursive: true })
  deployWrapper(p)

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
    template,
  })
  writeFileSync(p.plistPath, plist, { mode: 0o644 })

  const lint = spawnSync('/usr/bin/plutil', ['-lint', p.plistPath], {
    encoding: 'utf8',
  })
  if (lint.status !== 0) {
    console.error(`plutil rejected generated plist:\n${lint.stdout}${lint.stderr}`)
    process.exit(1)
  }

  // Idempotent (re)enable.
  launchctl(['bootout', p.job])
  launchctl(['enable', p.job])
  const boot = launchctl(['bootstrap', p.target, p.plistPath])
  if (boot.status !== 0) {
    console.error(
      `launchctl bootstrap failed:\n${boot.stderr.trim() || boot.stdout.trim()}`,
    )
    process.exit(1)
  }

  console.log('installed + enabled:')
  printStatus()
}

function printStatus() {
  const p = machinePaths()
  const loaded = launchctl(['print', p.job])
  console.log('label:', LABEL)
  console.log('job:', p.job)
  console.log('plist:', p.plistPath)
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
    })
    if (out.status === 0) console.log(out.stdout.trim())
    else console.log('snapshot exists (generated-at unavailable)')
  } else {
    console.log('snapshot: (missing) — web app degrades gracefully')
  }
}


function runOnce() {
  const p = machinePaths()
  // Prefer the deployed copy (exactly what launchd invokes); fall back to the
  // repo-resident wrapper before install.
  const wrapper = existsSync(p.deployedWrapper) ? p.deployedWrapper : p.wrapper
  const env = existsSync(p.deployedWrapper)
    ? { ...process.env, CULEBRALUXE_REPO: p.repo }
    : process.env
  if (!existsSync(wrapper)) {
    console.error(`calendar-sync wrapper missing: ${wrapper}`)
    process.exit(1)
  }
  const r = spawnSync('/bin/bash', [wrapper], { stdio: 'inherit', env })
  process.exit(r.status ?? 1)
}

function stop() {
  const p = machinePaths()
  launchctl(['bootout', p.job])
  launchctl(['disable', p.job])
  console.log(
    `stopped: no future scheduled invocations (plist kept at ${p.plistPath}).`,
  )
}

function uninstall() {
  const p = machinePaths()
  launchctl(['bootout', p.job])
  launchctl(['disable', p.job])
  if (existsSync(p.plistPath)) rmSync(p.plistPath)
  if (existsSync(p.deployedWrapper)) rmSync(p.deployedWrapper)
  console.log('uninstalled: launchd job removed, plist + deployed wrapper deleted.')
}

function main() {
  const command = process.argv[2]
  switch (command) {
    case 'install':
      install()
      break
    case 'status':
      printStatus()
      break
    case 'run':
      runOnce()
      break
    case 'stop':
      stop()
      break
    case 'uninstall':
      uninstall()
      break
    case 'help':
    case '--help':
    case '-h':
      console.log(`usage: node scripts/calendar-sync-agent.mjs <command>
commands:
  install     render + install + bootstrap the LaunchAgent (idempotent)
  status      show loaded/enabled state, snapshot, last invocation log
  run         run the exact same wrapper once (manual sync)
  stop        kill switch: boot out + persist disabled (no future runs)
  uninstall   stop + delete the plist`)
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

