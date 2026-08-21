#!/usr/bin/env node
// ---------------------------------------------------------------------------
// scripts/agent-scheduler.mjs — manage the local launchd LaunchAgent that
// periodically invokes `pnpm agent:work` (scripts/agent-worker-once.sh).
//
//   pnpm agent:scheduler:install     render + install + bootstrap (idempotent)
//   pnpm agent:scheduler:status      loaded/enabled state + last invocation
//   pnpm agent:scheduler:run         run the exact same wrapper once
//   pnpm agent:scheduler:stop        kill switch: boot out + persist disabled
//   pnpm agent:scheduler:uninstall   stop + delete the plist
//
// The scheduler owns NO queue logic. The database (migration 025) and
// `pnpm agent:work` own Ready discovery, single-worker enforcement, claiming,
// ordering, the run lifecycle, and story execution state. This tool only
// maintains the periodic wake-up.
//
// No secrets live in tracked files or in the generated plist. Environment
// overrides (documented in docs/agent/AGENT_WORKER_SCHEDULER.md):
//   CULEBRALUXE_LAUNCHAGENTS_DIR  - where the plist is installed
//   AGENT_WORKER_LOG_DIR          - where logs and the lock live
// ---------------------------------------------------------------------------

import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

export const LABEL = 'com.culebraluxe.agent-worker'
export const CADENCE_SECONDS = 300

export function repoRoot() {
  return join(dirname(fileURLToPath(import.meta.url)), '..')
}

export function machinePaths(env = process.env) {
  const home = env.HOME || homedir()
  const launchAgentsDir =
    env.CULEBRALUXE_LAUNCHAGENTS_DIR || join(home, 'Library', 'LaunchAgents')
  const supportDir =
    env.CULEBRALUXE_SUPPORT_DIR || join(home, 'Library', 'Application Support', 'CulebraLuxe')
  const logDir = env.AGENT_WORKER_LOG_DIR || join(home, 'Library', 'Logs', 'CulebraLuxe')
  const uid = typeof process.getuid === 'function' ? process.getuid() : 501
  return {
    repo: repoRoot(),
    home,
    launchAgentsDir,
    supportDir,
    plistPath: join(launchAgentsDir, `${LABEL}.plist`),
    logDir,
    invocationLog: join(logDir, 'agent-worker.invocations.log'),
    outLog: join(logDir, 'agent-worker.out.log'),
    errLog: join(logDir, 'agent-worker.err.log'),
    lockDir: join(logDir, 'agent-worker.lock'),
    wrapper: join(repoRoot(), 'scripts', 'agent-worker-once.sh'),
    deployedWrapper: join(supportDir, 'agent-worker-once.sh'),
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
    .replace(/\{\{CADENCE_SECONDS\}\}/g, String(cadenceSeconds))
}

function launchctl(args) {
  const r = spawnSync('/bin/launchctl', args, { encoding: 'utf8' })
  return { status: r.status, stdout: r.stdout ?? '', stderr: r.stderr ?? '' }
}

function isLoaded(p) {
  return launchctl(['print', p.job]).status === 0
}

function isDisabled(p) {
  const out = launchctl(['print-disabled', p.target])
  if (out.status !== 0) return false
  const m = out.stdout.match(new RegExp(`"${LABEL}"\\s*=>\\s*(disabled|enabled)`))
  return m ? m[1] === 'disabled' : false
}

function currentWorker(p) {
  if (!existsSync(p.lockDir)) return null
  const pidFile = join(p.lockDir, 'pid')
  if (!existsSync(pidFile)) return null
  let pid = ''
  try {
    pid = readFileSync(pidFile, 'utf8').trim()
  } catch {
    return null
  }
  if (!pid) return null
  const alive = spawnSync('/bin/kill', ['-0', pid]).status === 0
  return alive ? { pid } : null
}

function lastInvocations(p, count = 4) {
  if (!existsSync(p.invocationLog)) return '(no invocations logged yet)'
  let lines = []
  try {
    lines = readFileSync(p.invocationLog, 'utf8').trim().split('\n')
  } catch {
    return '(invocation log unreadable)'
  }
  if (lines.length === 0) return '(no invocations logged yet)'
  return lines.slice(-count).join('\n')
}

export function printStatus() {
  const p = machinePaths()
  const plistPresent = existsSync(p.plistPath)

  let summary
  if (!plistPresent) {
    summary = 'not installed (plist missing)'
  } else if (isLoaded(p)) {
    summary = 'installed + loaded'
  } else {
    summary = 'installed (plist present, not loaded)'
  }

  const worker = currentWorker(p)
  console.log('CulebraLuxe agent worker scheduler')
  console.log(`  status:     ${summary}`)
  console.log(`  label:      ${LABEL}`)
  console.log(`  cadence:    every ${CADENCE_SECONDS}s (5 minutes)`)
  console.log(`  plist:      ${p.plistPath}${plistPresent ? '' : ' (missing)'}`)
  console.log(`  disabled:   ${plistPresent ? (isDisabled(p) ? 'yes' : 'no') : 'n/a'}`)
  console.log(`  deployed:   ${p.deployedWrapper}${existsSync(p.deployedWrapper) ? '' : ' (missing)'}`)
  console.log(`  repo:       ${p.repo}/scripts/agent-worker-once.sh`)
  console.log(`  running:    ${worker ? `yes (pid ${worker.pid})` : 'no'}`)
  console.log(`  logs:       ${p.logDir}/agent-worker.{out,err,invocations}.log`)
  console.log('  last invocations:')
  for (const line of lastInvocations(p).split('\n')) {
    console.log(`    ${line}`)
  }
}

export function install() {
  const p = machinePaths()
  mkdirSync(p.launchAgentsDir, { recursive: true })
  mkdirSync(p.supportDir, { recursive: true })
  mkdirSync(p.logDir, { recursive: true })

  // Deploy a copy of the wrapper OUTSIDE the TCC-protected ~/Documents folder.
  // macOS does not allow launchd-spawned processes to execute files under
  // Documents; the deployed copy receives AGENT_WORKER_REPO and enters the
  // repository itself. Manual `scheduler:run` keeps using the repo wrapper.
  const wrapperSource = readFileSync(p.wrapper, 'utf8')
  writeFileSync(p.deployedWrapper, wrapperSource, { mode: 0o755 })

  const templatePath = join(p.repo, 'scripts', `${LABEL}.plist.template`)
  const template = readFileSync(templatePath, 'utf8')
  const plist = renderPlist({
    repo: p.repo,
    home: p.home,
    logDir: p.logDir,
    supportDir: p.supportDir,
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

  // Idempotent (re)enable: unload any existing instance, clear any persisted
  // disabled flag, then bootstrap.
  launchctl(['bootout', p.job]) // "No such process" is expected on fresh install
  launchctl(['enable', p.job]) // cancel any previously persisted `disable`
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

export function runOnce() {
  const p = machinePaths()
  if (!existsSync(p.wrapper)) {
    console.error(`agent-worker wrapper missing: ${p.wrapper}`)
    process.exit(1)
  }
  // Exactly the same wrapper the launchd schedule invokes.
  const r = spawnSync('/bin/bash', [p.wrapper], { stdio: 'inherit' })
  process.exit(r.status ?? 1)
}

export function stop() {
  const p = machinePaths()
  launchctl(['bootout', p.job]) // stop now (ignore "No such process")
  launchctl(['disable', p.job]) // persist across login/reboot
  console.log(
    `stopped: no future scheduled invocations (plist kept at ${p.plistPath}).`,
  )
  console.log('Story Board data untouched. Re-enable with `pnpm agent:scheduler:install`.')
}

export function uninstall() {
  const p = machinePaths()
  launchctl(['bootout', p.job])
  launchctl(['disable', p.job])
  if (existsSync(p.plistPath)) rmSync(p.plistPath)
  if (existsSync(p.deployedWrapper)) rmSync(p.deployedWrapper)
  console.log('uninstalled: launchd job removed, plist + deployed wrapper deleted.')
  console.log('Story Board data untouched. Logs kept at ' + p.logDir)
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
      console.log(`usage: node scripts/agent-scheduler.mjs <command>
commands:
  install     render + install + bootstrap the LaunchAgent (idempotent)
  status      show loaded/enabled state, running worker, last invocations
  run         run the exact same wrapper once (manual single-story claim)
  stop        kill switch: boot out + persist disabled (no future runs)
  uninstall   stop + delete the plist`)
      break
    default:
      console.error(`unknown command: ${command ?? '(none)'}`)
      console.error('try: node scripts/agent-scheduler.mjs help')
      process.exit(2)
  }
}

if (fileURLToPath(import.meta.url) === process.argv[1]) {
  main()
}
