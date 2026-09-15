#!/usr/bin/env node
// ---------------------------------------------------------------------------
// Immediate Apple gateway wake-up for the PROD web app.
//
// The web app already writes durable Calendar/Reminder commands to Neon. This
// listener adds only the missing fast path: a browser running on THIS Mac can
// POST /sync to loopback and wake the existing trusted Apple launcher now.
//
// No event/task payload crosses localhost. Neon remains the source of truth,
// and the normal 30-minute Calendar LaunchAgent remains the recovery/inbound
// reconciliation path for mobile saves, sleeping Macs, or a missed local kick.
// ---------------------------------------------------------------------------

import { spawn, spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { createServer } from 'node:http'
import { homedir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

export const LABEL = 'com.culebraluxe.apple-local-listener'
export const HOST = '127.0.0.1'
export const PORT = Number(process.env.CULEBRALUXE_APPLE_LOCAL_PORT || 47831)
const LAUNCHCTL_TIMEOUT_MS = 10000

const ALLOWED_ORIGINS = new Set([
  'https://www.culebraluxe.com',
  'https://culebraluxe.com',
  'http://localhost:3000',
  'http://127.0.0.1:3000',
])

function repoRoot() {
  return join(dirname(fileURLToPath(import.meta.url)), '..')
}

function paths(env = process.env) {
  const home = env.HOME || homedir()
  const repo = env.CULEBRALUXE_REPO || repoRoot()
  const launchAgentsDir = env.CULEBRALUXE_LAUNCHAGENTS_DIR || join(home, 'Library', 'LaunchAgents')
  const supportDir = env.CULEBRALUXE_SUPPORT_DIR || join(home, 'Library', 'Application Support', 'CulebraLuxe')
  const logDir = env.CULEBRALUXE_CALENDAR_LOG_DIR || join(home, 'Library', 'Logs', 'CulebraLuxe')
  const uid = typeof process.getuid === 'function' ? process.getuid() : 501
  return {
    home,
    repo,
    launchAgentsDir,
    supportDir,
    logDir,
    appleLauncher: join(supportDir, 'apple-sync-launcher'),
    sourceWrapper: join(repo, 'scripts', 'macbridge', 'apple-outbound-once.sh'),
    deployedWrapper: join(supportDir, 'apple-outbound-once.sh'),
    listenerScript: join(repo, 'scripts', 'apple-local-listener.mjs'),
    plistPath: join(launchAgentsDir, `${LABEL}.plist`),
    stdoutPath: join(logDir, 'apple-local-listener.out.log'),
    stderrPath: join(logDir, 'apple-local-listener.err.log'),
    target: `gui/${uid}`,
    job: `gui/${uid}/${LABEL}`,
  }
}

function xml(value) {
  return String(value).replace(/[<>&'\"]/g, (ch) => {
    if (ch === '<') return '&lt;'
    if (ch === '>') return '&gt;'
    if (ch === '&') return '&amp;'
    if (ch === "'") return '&apos;'
    return '&quot;'
  })
}

function renderPlist(p) {
  return `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>${xml(LABEL)}</string>
  <key>ProgramArguments</key>
  <array>
    <string>${xml(process.execPath)}</string>
    <string>${xml(p.listenerScript)}</string>
    <string>serve</string>
  </array>
  <key>WorkingDirectory</key>
  <string>${xml(p.repo)}</string>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>ProcessType</key>
  <string>Background</string>
  <key>ThrottleInterval</key>
  <integer>5</integer>
  <key>EnvironmentVariables</key>
  <dict>
    <key>HOME</key>
    <string>${xml(p.home)}</string>
    <key>CULEBRALUXE_REPO</key>
    <string>${xml(p.repo)}</string>
    <key>CULEBRALUXE_SUPPORT_DIR</key>
    <string>${xml(p.supportDir)}</string>
    <key>CULEBRALUXE_CALENDAR_LOG_DIR</key>
    <string>${xml(p.logDir)}</string>
  </dict>
  <key>StandardOutPath</key>
  <string>${xml(p.stdoutPath)}</string>
  <key>StandardErrorPath</key>
  <string>${xml(p.stderrPath)}</string>
</dict>
</plist>
`
}

function launchctl(args) {
  return spawnSync('/bin/launchctl', args, { encoding: 'utf8', timeout: LAUNCHCTL_TIMEOUT_MS })
}

function commandDetail(result) {
  if (result?.error) return result.error.message
  return result?.stderr?.trim() || result?.stdout?.trim() || `exit ${result?.status ?? 'unknown'}`
}

function requireFiles(p) {
  if (!existsSync(p.appleLauncher)) {
    throw new Error(`trusted Apple launcher missing: ${p.appleLauncher}`)
  }
  if (!existsSync(p.sourceWrapper)) {
    throw new Error(`outbound wrapper missing: ${p.sourceWrapper}`)
  }
}

function install() {
  const p = paths()
  requireFiles(p)
  mkdirSync(p.launchAgentsDir, { recursive: true })
  mkdirSync(p.supportDir, { recursive: true })
  mkdirSync(p.logDir, { recursive: true })

  writeFileSync(p.deployedWrapper, readFileSync(p.sourceWrapper, 'utf8'), { mode: 0o755 })
  writeFileSync(p.plistPath, renderPlist(p), { mode: 0o644 })

  const lint = spawnSync('/usr/bin/plutil', ['-lint', p.plistPath], { encoding: 'utf8', timeout: 10000 })
  if (lint.status !== 0) throw new Error(`plutil rejected listener plist: ${commandDetail(lint)}`)

  launchctl(['bootout', p.job])
  const enable = launchctl(['enable', p.job])
  if (enable.status !== 0) throw new Error(`launchctl enable failed: ${commandDetail(enable)}`)
  const boot = launchctl(['bootstrap', p.target, p.plistPath])
  if (boot.status !== 0) throw new Error(`launchctl bootstrap failed: ${commandDetail(boot)}`)

  console.log(`[apple-local] installed: http://${HOST}:${PORT}/sync`)
  status()
}

function status() {
  const p = paths()
  const loaded = launchctl(['print', p.job])
  console.log('label:', LABEL)
  console.log('endpoint:', `http://${HOST}:${PORT}/sync`)
  console.log('listener:', p.listenerScript)
  console.log('launcher:', p.appleLauncher, existsSync(p.appleLauncher) ? '(present)' : '(MISSING)')
  console.log('outbound wrapper:', p.deployedWrapper, existsSync(p.deployedWrapper) ? '(present)' : '(MISSING)')
  console.log('loaded/enabled:', loaded.status === 0 ? 'yes' : 'no')
}

function stop() {
  const p = paths()
  launchctl(['bootout', p.job])
  launchctl(['disable', p.job])
  console.log('[apple-local] stopped')
}

function uninstall() {
  const p = paths()
  launchctl(['bootout', p.job])
  launchctl(['disable', p.job])
  if (existsSync(p.plistPath)) rmSync(p.plistPath)
  if (existsSync(p.deployedWrapper)) rmSync(p.deployedWrapper)
  console.log('[apple-local] uninstalled; scheduled Calendar sync was not touched')
}

function originAllowed(origin) {
  if (!origin) return true // curl/local diagnostics; browsers send Origin cross-site.
  return ALLOWED_ORIGINS.has(origin)
}

function applyCors(req, res) {
  const origin = req.headers.origin
  if (!originAllowed(origin)) return false
  if (origin) {
    res.setHeader('Access-Control-Allow-Origin', origin)
    res.setHeader('Vary', 'Origin')
  }
  res.setHeader('Access-Control-Allow-Methods', 'POST, OPTIONS, GET')
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type')
  // Chromium's Private Network Access preflight; ignored by browsers that do not use it.
  res.setHeader('Access-Control-Allow-Private-Network', 'true')
  res.setHeader('Cache-Control', 'no-store')
  return true
}

function serve() {
  const p = paths()
  requireFiles(p)
  if (!existsSync(p.deployedWrapper)) {
    throw new Error(`listener not installed: missing ${p.deployedWrapper}; run install first`)
  }

  let child = null
  let rerun = false

  const kick = () => {
    if (child) {
      // A second Save while Swift is busy must not be lost. Drain once more when this pass exits.
      rerun = true
      return
    }
    const env = {
      ...process.env,
      CULEBRALUXE_REPO: p.repo,
      CULEBRALUXE_APPLE_SYNC_SCRIPT: p.deployedWrapper,
    }
    console.log(`[apple-local] wake ${new Date().toISOString()}`)
    child = spawn(p.appleLauncher, [], { cwd: p.supportDir, env, stdio: 'inherit' })
    child.once('error', (error) => console.error('[apple-local] launcher error:', error.message))
    child.once('close', (code) => {
      console.log(`[apple-local] outbound pass exit=${code ?? 'unknown'}`)
      child = null
      if (rerun) {
        rerun = false
        kick()
      }
    })
  }

  const server = createServer((req, res) => {
    if (!applyCors(req, res)) {
      res.writeHead(403, { 'content-type': 'application/json' })
      res.end(JSON.stringify({ ok: false, error: 'origin refused' }))
      return
    }

    if (req.method === 'OPTIONS') {
      res.writeHead(204)
      res.end()
      return
    }

    if (req.method === 'GET' && req.url === '/health') {
      res.writeHead(200, { 'content-type': 'application/json' })
      res.end(JSON.stringify({ ok: true, busy: Boolean(child), label: LABEL }))
      return
    }

    if (req.method === 'POST' && req.url === '/sync') {
      kick()
      res.writeHead(202, { 'content-type': 'application/json' })
      res.end(JSON.stringify({ ok: true, accepted: true, busy: Boolean(child) }))
      return
    }

    res.writeHead(404, { 'content-type': 'application/json' })
    res.end(JSON.stringify({ ok: false, error: 'not found' }))
  })

  server.on('error', (error) => {
    console.error('[apple-local] listener failed:', error.message)
    process.exitCode = 1
  })
  server.listen(PORT, HOST, () => {
    console.log(`[apple-local] listening on http://${HOST}:${PORT}`)
  })
}

function main() {
  const command = process.argv[2]
  try {
    if (command === 'install') return install()
    if (command === 'status') return status()
    if (command === 'stop') return stop()
    if (command === 'uninstall') return uninstall()
    if (command === 'serve') return serve()
    console.error('usage: node scripts/apple-local-listener.mjs <install|status|serve|stop|uninstall>')
    process.exitCode = 2
  } catch (error) {
    console.error(`[apple-local] ${error instanceof Error ? error.message : String(error)}`)
    process.exitCode = 1
  }
}

main()
