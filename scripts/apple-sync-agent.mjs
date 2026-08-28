#!/usr/bin/env node
// ---------------------------------------------------------------------------
// scripts/apple-sync-agent.mjs — manage the macOS LaunchAgent that runs the
// canonical Apple Messages production sync twice daily (08:00 / 18:00 local).
//
//   pnpm apple:sync:install      build launcher, deploy wrapper, render+install plist
//   pnpm apple:sync:status       loaded/PID/last exit/last success/log tail
//   pnpm apple:sync:run          run the exact launchd path once (manual sync)
//   pnpm apple:sync:verify-tcc   prove the launchd binary can open chat.db READ-ONLY
//   pnpm apple:sync:stop         kill switch: boot out + persist disabled
//   pnpm apple:sync:uninstall    stop + delete the plist + deployed copies
//
// The LaunchAgent invokes a compiled, FDA-grantable Swift launcher
// (apple-messages-export/.build/release/apple-sync-launcher) deployed to the
// support dir (outside the TCC-protected ~/Documents folder). That launcher
// execs a deployed copy of scripts/apple-sync.sh. The ONE manual step for Full
// Disk Access is granting it to the deployed launcher in System Settings; TCC
// responsibility propagates so the chain can read ~/Library/Messages/chat.db.
//
// No secrets live in tracked files or the generated plist (DATABASE_URL_PROD is
// read by the intake from .env.local at run time).
// ---------------------------------------------------------------------------

import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

export const LABEL = 'com.culebraluxe.apple-sync'

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
    env.CULEBRALUXE_APPLE_LOG_DIR || join(home, 'Library', 'Logs', 'CulebraLuxe')
  const uid = typeof process.getuid === 'function' ? process.getuid() : 501
  return {
    repo: repoRoot(),
    home,
    launchAgentsDir,
    supportDir,
    logDir,
    plistPath: join(launchAgentsDir, `${LABEL}.plist`),
    logFile: join(logDir, 'apple-sync.log'),
    launcherSource: join(
      repoRoot(),
      'apple-messages-export',
      '.build',
      'release',
      'apple-sync-launcher',
    ),
    deployedLauncher: join(supportDir, 'apple-sync-launcher'),
    wrapper: join(repoRoot(), 'scripts', 'apple-sync.sh'),
    deployedWrapper: join(supportDir, 'apple-sync.sh'),
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

export function renderPlist({ repo, home, logDir, supportDir, launcher, script, pathEnv, template }) {
  return template
    .replace(/\{\{LABEL\}\}/g, escapeXml(LABEL))
    .replace(/\{\{REPO_ROOT\}\}/g, escapeXml(repo))
    .replace(/\{\{SUPPORT_DIR\}\}/g, escapeXml(supportDir))
    .replace(/\{\{HOME\}\}/g, escapeXml(home))
    .replace(/\{\{LOG_DIR\}\}/g, escapeXml(logDir))
    .replace(/\{\{LAUNCHER\}\}/g, escapeXml(launcher))
    .replace(/\{\{SCRIPT\}\}/g, escapeXml(script))
    .replace(/\{\{PATH\}\}/g, escapeXml(pathEnv))
}

function launchctl(args) {
  return spawnSync('/bin/launchctl', args, { encoding: 'utf8' })
}

function resolvePathEnv() {
  const which = spawnSync('command', ['-v', 'node'], { encoding: 'utf8' }).stdout.trim()
  const nodeDir = which ? dirname(which) : ''
  const parts = new Set(['/opt/homebrew/bin', '/usr/local/bin', '/usr/bin', '/bin', '/usr/sbin', '/sbin'])
  if (nodeDir) parts.add(nodeDir)
  return Array.from(parts).join(':')
}

function buildLauncher() {
  const p = machinePaths()
  console.log('building FDA-grantable launcher (swift build -c release)...')
  const b = spawnSync(
    '/usr/bin/swift',
    ['build', '-c', 'release', '--package-path', join(p.repo, 'apple-messages-export'), '--product', 'apple-sync-launcher'],
    { encoding: 'utf8' },
  )
  if (b.status !== 0) {
    console.error(`swift build failed:\n${b.stdout}${b.stderr}`)
    process.exit(1)
  }
  const cs = spawnSync('/usr/bin/codesign', ['--force', '--sign', '-', p.launcherSource], { encoding: 'utf8' })
  if (cs.status !== 0) {
    console.error(`codesign ad-hoc failed:\n${cs.stdout}${cs.stderr}`)
    process.exit(1)
  }
  console.log('launcher built + ad-hoc signed:', p.launcherSource)
}

function deployLauncher() {
  const p = machinePaths()
  mkdirSync(p.supportDir, { recursive: true })
  if (!existsSync(p.launcherSource)) {
    console.error(`launcher binary missing: ${p.launcherSource} (run install)`)
    process.exit(1)
  }
  const src = readFileSync(p.launcherSource)
  const changed = !existsSync(p.deployedLauncher) || !Buffer.from(readFileSync(p.deployedLauncher)).equals(src)
  writeFileSync(p.deployedLauncher, src, { mode: 0o755 })
  if (changed) {
    console.log('NOTE: deployed launcher changed at', p.deployedLauncher)
    console.log('      if Full Disk Access was already granted to it, re-grant after this reinstall.')
  }
}

function deployWrapper() {
  const p = machinePaths()
  if (!existsSync(p.wrapper)) {
    console.error(`canonical sync script missing: ${p.wrapper}`)
    process.exit(1)
  }
  mkdirSync(p.supportDir, { recursive: true })
  writeFileSync(p.deployedWrapper, readFileSync(p.wrapper, 'utf8'), { mode: 0o755 })
}


function install() {
  const p = machinePaths()
  mkdirSync(p.launchAgentsDir, { recursive: true })
  mkdirSync(p.logDir, { recursive: true })
  buildLauncher()
  deployLauncher()
  deployWrapper()

  const templatePath = join(p.repo, 'scripts', 'com.culebraluxe.apple-sync.plist.template')
  const template = readFileSync(templatePath, 'utf8')
  const plist = renderPlist({
    repo: p.repo,
    home: p.home,
    logDir: p.logDir,
    supportDir: p.supportDir,
    launcher: p.deployedLauncher,
    script: p.deployedWrapper,
    pathEnv: resolvePathEnv(),
    template,
  })
  writeFileSync(p.plistPath, plist, { mode: 0o644 })

  const lint = spawnSync('/usr/bin/plutil', ['-lint', p.plistPath], { encoding: 'utf8' })
  if (lint.status !== 0) {
    console.error(`plutil rejected generated plist:\n${lint.stdout}${lint.stderr}`)
    process.exit(1)
  }
  console.log('plist valid:', p.plistPath)

  // Idempotent (re)enable.
  launchctl(['bootout', p.job])
  launchctl(['enable', p.job])
  const boot = launchctl(['bootstrap', p.target, p.plistPath])
  if (boot.status !== 0) {
    console.error(`launchctl bootstrap failed:\n${boot.stderr.trim() || boot.stdout.trim()}`)
    process.exit(1)
  }

  console.log('installed + enabled (08:00 and 18:00 local):')
  printStatus()
}

function printStatus() {
  const p = machinePaths()
  const loaded = launchctl(['print', p.job])
  const loadedOk = loaded.status === 0
  console.log('label:', LABEL)
  console.log('job:', p.job)
  console.log('plist:', p.plistPath)
  console.log('launcher:', p.deployedLauncher)
  console.log('wrapper:', p.deployedWrapper)
  console.log('installed:', existsSync(p.plistPath) ? 'yes' : 'no')
  console.log('loaded/enabled:', loadedOk ? 'yes' : 'no (not loaded or disabled)')

  if (loadedOk) {
    const out = loaded.stdout
    const pid = out.match(/pid\s*=\s*(\d+)/)?.[1]
    const lastExit = out.match(/last exit code\s*=\s*(\S+)/)?.[1]
    console.log('pid:', pid || '(not running)')
    if (lastExit) console.log('last exit code:', lastExit)
  }

  if (existsSync(p.logFile)) {
    const lines = readFileSync(p.logFile, 'utf8').trim().split('\n')
    const lastSuccess = lines.findLast((l) => l.includes('status=SUCCESS'))
    if (lastSuccess) console.log('last successful sync:', lastSuccess.replace(/^.*APPLE SYNC END /, ''))
    console.log('tail of', p.logFile, ':')
    for (const line of lines.slice(-8)) console.log('  ' + line)
  } else {
    console.log('log:', p.logFile, '(no runs yet)')
  }
}

function runOnce() {
  const p = machinePaths()
  const runner = existsSync(p.deployedLauncher) ? p.deployedLauncher : p.launcherSource
  if (!existsSync(runner)) {
    console.error(`launcher missing: ${runner} (run install first)`)
    process.exit(1)
  }
  const r = spawnSync(runner, [], { stdio: 'inherit' })
  process.exit(r.status ?? 1)
}

function verifyTcc() {
  const p = machinePaths()
  const runner = existsSync(p.deployedLauncher) ? p.deployedLauncher : p.launcherSource
  if (!existsSync(runner)) {
    console.error(`launcher missing: ${runner} (run install first)`)
    process.exit(1)
  }
  const r = spawnSync(runner, ['--verify-tcc'], { stdio: 'inherit' })
  process.exit(r.status ?? 1)
}

function stop() {
  const p = machinePaths()
  launchctl(['bootout', p.job])
  launchctl(['disable', p.job])
  console.log(`stopped: no future scheduled invocations (plist kept at ${p.plistPath}).`)
}

function uninstall() {
  const p = machinePaths()
  launchctl(['bootout', p.job])
  launchctl(['disable', p.job])
  if (existsSync(p.plistPath)) rmSync(p.plistPath)
  if (existsSync(p.deployedWrapper)) rmSync(p.deployedWrapper)
  if (existsSync(p.deployedLauncher)) rmSync(p.deployedLauncher)
  console.log('uninstalled: launchd job removed; plist + deployed copies deleted.')
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
    case 'verify-tcc':
      verifyTcc()
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
      console.log(`usage: node scripts/apple-sync-agent.mjs <command>
commands:
  install     build launcher + deploy + render/install the LaunchAgent (idempotent)
  status      show loaded/PID/last exit/last success/log tail
  run         run the exact launchd path once (manual sync)
  verify-tcc  prove the launchd binary can open chat.db READ-ONLY
  stop        kill switch: boot out + persist disabled (no future runs)
  uninstall   stop + delete the plist + deployed copies`)
      break
    default:
      console.error(`unknown command: ${command ?? '(none)'}`)
      console.error('try: node scripts/apple-sync-agent.mjs help')
      process.exit(2)
  }
}

if (fileURLToPath(import.meta.url) === process.argv[1]) {
  main()
}

