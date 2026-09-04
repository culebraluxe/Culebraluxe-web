import assert from 'node:assert/strict'
import test from 'node:test'
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { sha256File, wrapperIntegrity } from './agent-scheduler.mjs'

test('sha256File returns stable content fingerprint and null for missing files', () => {
  const dir = mkdtempSync(join(tmpdir(), 'forge-scheduler-'))
  try {
    const file = join(dir, 'worker.sh')
    writeFileSync(file, '#!/bin/bash\necho forge\n')
    const first = sha256File(file)
    const second = sha256File(file)
    assert.ok(first)
    assert.equal(first, second)
    assert.equal(first.length, 64)
    assert.equal(sha256File(join(dir, 'missing.sh')), null)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('wrapperIntegrity proves repo and deployed wrapper bytes are identical', () => {
  const dir = mkdtempSync(join(tmpdir(), 'forge-scheduler-'))
  try {
    const repo = join(dir, 'repo.sh')
    const deployed = join(dir, 'deployed.sh')
    writeFileSync(repo, '#!/bin/bash\necho one\n')
    writeFileSync(deployed, '#!/bin/bash\necho one\n')

    const synced = wrapperIntegrity(repo, deployed)
    assert.equal(synced.synced, true)
    assert.equal(synced.repoSha, synced.deployedSha)

    writeFileSync(deployed, '#!/bin/bash\necho two\n')
    const mismatched = wrapperIntegrity(repo, deployed)
    assert.equal(mismatched.synced, false)
    assert.notEqual(synced.repoSha, mismatched.deployedSha)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('wrapperIntegrity fails closed when either wrapper is absent', () => {
  const dir = mkdtempSync(join(tmpdir(), 'forge-scheduler-'))
  try {
    const repo = join(dir, 'repo.sh')
    const deployed = join(dir, 'deployed.sh')
    writeFileSync(repo, '#!/bin/bash\n')
    const result = wrapperIntegrity(repo, deployed)
    assert.equal(result.synced, false)
    assert.ok(result.repoSha)
    assert.equal(result.deployedSha, null)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('scheduled wake fast-forwards main but contains no recovery or destructive Git logic', () => {
  const wrapper = readFileSync(new URL('./agent-worker-once.sh', import.meta.url), 'utf8')

  assert.match(wrapper, /pnpm agent:work 2>&1/)
  assert.match(
    wrapper,
    /git pull --ff-only origin main/,
    'scheduler must refresh the control-plane checkout before unattended work',
  )
  assert.doesNotMatch(
    wrapper,
    /forge-runtime-recover|recoverStaleAgentWorkIndustrial/,
    'launchd wrapper must delegate recovery to Forge itself',
  )
  assert.doesNotMatch(
    wrapper,
    /\bgit\s+(?:checkout|switch|reset|rebase|stash|push)\b/,
    'scheduler Git sync must stay fast-forward-only and never rewrite/publish history',
  )
})

test('agent:work owns recovery before invoking the existing Forge command', () => {
  const packageJson = JSON.parse(
    readFileSync(new URL('../package.json', import.meta.url), 'utf8'),
  )
  assert.match(packageJson.scripts['agent:work'], /scripts\/agent-work-entry\.ts/)

  const entry = readFileSync(new URL('./agent-work-entry.ts', import.meta.url), 'utf8')
  const recovery = entry.indexOf('recoverStaleAgentWorkIndustrial')
  const child = entry.indexOf("resolve(process.cwd(), 'scripts/agent-work.ts')")
  assert.ok(recovery >= 0, 'agent:work entry must own industrial stale recovery')
  assert.ok(child > recovery, 'recovery must run before the normal Forge pass')
})