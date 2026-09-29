import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import { commitWorkerWorkspaceChanges } from '../lib/worker-workspace'

function git(cwd: string, args: string[]): string {
  return execFileSync('git', args, { cwd, encoding: 'utf8' }).trim()
}

function repo(): string {
  const cwd = mkdtempSync(join(tmpdir(), 'forge-commit-'))
  git(cwd, ['init'])
  git(cwd, ['config', 'user.email', 'forge-test@example.com'])
  git(cwd, ['config', 'user.name', 'Forge Test'])
  writeFileSync(join(cwd, 'base.txt'), 'base\n')
  git(cwd, ['add', 'base.txt'])
  git(cwd, ['commit', '-m', 'base'])
  return cwd
}

test('outer Forge commits dirty Smith workspace', async () => {
  const cwd = repo()
  const base = git(cwd, ['rev-parse', 'HEAD'])
  writeFileSync(join(cwd, 'smith.txt'), 'candidate\n')

  const result = await commitWorkerWorkspaceChanges(cwd, 'FORGE-TEST: candidate')

  assert.equal(result.changed, true)
  assert.ok(result.commitHash)
  assert.notEqual(result.commitHash, base)
  assert.equal(git(cwd, ['status', '--porcelain']), '')
  assert.equal(git(cwd, ['log', '-1', '--format=%s']), 'FORGE-TEST: candidate')
})

test('outer Forge does not create empty commit', async () => {
  const cwd = repo()
  const head = git(cwd, ['rev-parse', 'HEAD'])

  const result = await commitWorkerWorkspaceChanges(cwd, 'FORGE-TEST: empty')

  assert.deepEqual(result, { commitHash: null, changed: false })
  assert.equal(git(cwd, ['rev-parse', 'HEAD']), head)
})
