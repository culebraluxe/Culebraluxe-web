import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'
import { promisify } from 'node:util'

import { adapterCommitOptions } from '../../agent-runtime/gateway/cli-agent-adapter'
import {
  commitWorkerWorkspaceChanges,
  renderAllowedScopeMarker,
} from '../../lib/worker-workspace/commit'

// ENG-FORGE-REVIEW-RESIDUALS-01 Unit A — the CLI gateway commits inside a scope.
//
// The factory path already parsed the lane's FORGE_ALLOWED_SCOPE marker and passed
// it to commitWorkerWorkspaceChanges; the CLI gateway called the same seam with no
// scope, so it fell into the legacy `git add -A` sweep. These lock the fix at the
// adapter's own seam: its options come from the SAME marker parser the factory uses,
// an in-scope change commits, and an out-of-scope path refuses the commit BY NAME.

const run = promisify(execFile)
const IDENTITY = ['-c', 'user.email=t@t', '-c', 'user.name=t']

async function git(cwd: string, args: string[]): Promise<string> {
  const { stdout } = await run('git', args, { cwd })
  return stdout.trim()
}

const DECLARED = 'agent-runtime/gateway/cli-agent-adapter.ts'

async function repo(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), 'cli-adapter-scope-'))
  await git(dir, ['init', '-q', '-b', 'main'])
  await writeFile(join(dir, 'README.md'), 'base\n')
  // Track the declared file in the base so `git status --porcelain` reports the
  // file itself, not a collapsed untracked directory (an untracked dir reads as
  // `?? agent-runtime/`, which is a different path from the declared file).
  await mkdir(join(dir, 'agent-runtime', 'gateway'), { recursive: true })
  await writeFile(join(dir, DECLARED), 'export const scoped = 0\n')
  await git(dir, [...IDENTITY, 'add', '-A'])
  await git(dir, [...IDENTITY, 'commit', '-q', '-m', 'base'])
  return dir
}

test('the adapter parses the shared FORGE_ALLOWED_SCOPE marker', () => {
  const marker = renderAllowedScopeMarker([DECLARED])
  assert.deepEqual(adapterCommitOptions(`prose\n${marker}`), { allowedScope: [DECLARED] })
  assert.deepEqual(adapterCommitOptions('no marker at all'), {})
  assert.deepEqual(adapterCommitOptions(null), {})
})

test('an in-scope commit is made', async () => {
  const dir = await repo()
  await writeFile(join(dir, DECLARED), 'export const scoped = 1\n')
  const result = await commitWorkerWorkspaceChanges(
    dir,
    'scope test',
    adapterCommitOptions(`prose\n${renderAllowedScopeMarker([DECLARED])}`),
  )
  assert.ok(result.commitHash, 'an in-scope change commits')
  assert.equal(result.refused, undefined)
})

test('a commit outside the declared scope is refused and the offending path is named', async () => {
  const dir = await repo()
  await writeFile(join(dir, 'rogue.ts'), 'export const rogue = 1\n')
  const result = await commitWorkerWorkspaceChanges(
    dir,
    'scope test',
    adapterCommitOptions(`prose\n${renderAllowedScopeMarker([DECLARED])}`),
  )
  assert.equal(result.commitHash, null, 'the commit is refused')
  assert.deepEqual(result.refused, ['rogue.ts'])
})
