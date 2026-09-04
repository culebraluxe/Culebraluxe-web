import assert from 'node:assert/strict'
import test from 'node:test'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
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
    assert.notEqual(mismatched.repoSha, mismatched.deployedSha)
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
