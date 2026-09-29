import { test } from 'node:test'
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { provisionOrRecoverWorkerWorkspace } from './recovering-provisioner'

function git(cwd: string, args: string[]): string {
  return execFileSync('git', args, { cwd, encoding: 'utf8' }).trim()
}

test('recovery reuses the exact Forge worktree and preserves dirty Smith work', async () => {
  const root = mkdtempSync(join(tmpdir(), 'forge-recovery-'))
  const repoRoot = join(root, 'repo')
  const worktreesRoot = join(root, 'worktrees')
  mkdirSync(repoRoot, { recursive: true })

  try {
    git(repoRoot, ['init', '-b', 'main', '-q'])
    git(repoRoot, ['config', 'user.email', 'forge-recovery@example.com'])
    git(repoRoot, ['config', 'user.name', 'Forge Recovery Test'])
    writeFileSync(join(repoRoot, 'base.txt'), 'base\n')
    git(repoRoot, ['add', 'base.txt'])
    git(repoRoot, ['commit', '-m', 'base', '-q'])

    const spec = {
      storyId: 'ENG-FORGE-RECOVERY-TEST',
      workerId: 'forge-test-worker',
      baseRef: 'main',
      runId: 'work-item-123',
      repoRoot,
      worktreesRoot,
      sharedLinks: [],
    }

    const first = await provisionOrRecoverWorkerWorkspace(spec)
    const dirtyPath = join(first.worktreePath, 'smith-partial.ts')
    writeFileSync(dirtyPath, 'export const survived = true\n')

    const before = git(first.worktreePath, ['status', '--porcelain'])
    assert.match(before, /smith-partial\.ts/)

    // Simulate Forge restarting and provisioning the SAME durable work item.
    // The second call must attach, not create/reset/clean/rebase.
    const recovered = await provisionOrRecoverWorkerWorkspace(spec)

    assert.equal(recovered.branchName, first.branchName)
    assert.equal(recovered.worktreePath, first.worktreePath)
    assert.equal(recovered.baseCommit, first.baseCommit)
    assert.equal(readFileSync(dirtyPath, 'utf8'), 'export const survived = true\n')
    assert.match(git(recovered.worktreePath, ['status', '--porcelain']), /smith-partial\.ts/)
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})
