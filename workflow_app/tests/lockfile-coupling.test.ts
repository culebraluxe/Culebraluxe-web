import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'
import { promisify } from 'node:util'

import {
  DEPENDENCY_LOCKFILE,
  DEPENDENCY_MANIFEST,
  commitWorkerWorkspaceChanges,
  dependencyCompanions,
} from '../../lib/worker-workspace/commit'

const execFileAsync = promisify(execFile)

async function git(cwd: string, args: string[]): Promise<string> {
  const { stdout } = await execFileAsync('git', args, { cwd, encoding: 'utf8' })
  return stdout.trim()
}

async function makeRepo(): Promise<string> {
  const repo = await mkdtemp(join(tmpdir(), 'forge-lockfile-'))
  await git(repo, ['init', '-b', 'main', '-q'])
  await git(repo, ['config', 'user.name', 'fence'])
  await git(repo, ['config', 'user.email', 'fence@test'])
  await git(repo, ['config', 'commit.gpgsign', 'false'])
  await writeFile(join(repo, DEPENDENCY_MANIFEST), '{"name":"x","dependencies":{}}\n')
  await writeFile(join(repo, DEPENDENCY_LOCKFILE), 'lockfileVersion: 9.0\n')
  await mkdir(join(repo, 'lib'), { recursive: true })
  await writeFile(join(repo, 'lib', 'thing.ts'), 'export const thing = 1\n')
  await git(repo, ['add', '-A'])
  await git(repo, ['commit', '-q', '-m', 'base'])
  return repo
}

const committedPaths = (repo: string): string[] =>
  git(repo, ['show', '--name-only', '--format=', 'HEAD']).then((out) => out.split('\n').filter(Boolean))

// ---------------------------------------------------------------------------
// ENG-FORGE-LOCKFILE-COUPLING-01 — the publish side of the dependency coupling.
//
// Twice on 2026-09-18 a lane added a dependency, ran the install, and published a candidate that left
// pnpm-lock.yaml behind: main went red at `pnpm install --frozen-lockfile` in 15 seconds, and CI
// caught it while the engine did not. The scope written before the run could not name the artifact the
// run's own choice implied, so the SEAM carries it now.
// ---------------------------------------------------------------------------

test('lockfile-coupling: a dependency change carries its lockfile even though the lane never declared it', async () => {
  const repo = await makeRepo()
  try {
    await writeFile(join(repo, DEPENDENCY_MANIFEST), '{"name":"x","dependencies":{"fast-check":"4"}}\n')
    await writeFile(join(repo, DEPENDENCY_LOCKFILE), 'lockfileVersion: 9.0\nfast-check: 4.0.0\n')
    // The lane declared ONLY the manifest, exactly as a surface written in advance would.
    const result = await commitWorkerWorkspaceChanges(repo, 'add a dependency', {
      allowedScope: [DEPENDENCY_MANIFEST],
    })
    assert.equal(result.refused, undefined, 'the lockfile must not be refused as out of scope')
    assert.ok(result.commitHash, 'the candidate committed')
    const paths = await committedPaths(repo)
    assert.ok(paths.includes(DEPENDENCY_MANIFEST), 'manifest is in the commit')
    assert.ok(paths.includes(DEPENDENCY_LOCKFILE), 'the lockfile travelled with it')
  } finally {
    await rm(repo, { recursive: true, force: true })
  }
})

test('lockfile-coupling: a lockfile-only change is allowed', async () => {
  const repo = await makeRepo()
  try {
    await writeFile(join(repo, DEPENDENCY_LOCKFILE), 'lockfileVersion: 9.0\nresolved: something\n')
    const result = await commitWorkerWorkspaceChanges(repo, 'resolve', { allowedScope: ['lib/'] })
    assert.equal(result.refused, undefined)
    assert.ok(result.commitHash)
    assert.ok((await committedPaths(repo)).includes(DEPENDENCY_LOCKFILE))
  } finally {
    await rm(repo, { recursive: true, force: true })
  }
})

test('lockfile-coupling: a change with no dependency involvement is untouched by the guard', async () => {
  const repo = await makeRepo()
  try {
    await writeFile(join(repo, 'lib', 'thing.ts'), 'export const thing = 2\n')
    const result = await commitWorkerWorkspaceChanges(repo, 'plain change', { allowedScope: ['lib/'] })
    assert.ok(result.commitHash)
    const paths = await committedPaths(repo)
    assert.deepEqual(paths, ['lib/thing.ts'], 'only the declared change is committed')
  } finally {
    await rm(repo, { recursive: true, force: true })
  }
})

test('lockfile-coupling: the guard does NOT weaken scope enforcement for anything else', async () => {
  const repo = await makeRepo()
  try {
    await writeFile(join(repo, 'lib', 'thing.ts'), 'export const thing = 2\n')
    await writeFile(join(repo, DEPENDENCY_LOCKFILE), 'lockfileVersion: 9.0\nresolved: x\n')
    await writeFile(join(repo, DEPENDENCY_MANIFEST), '{"name":"x","dependencies":{"a":"1"}}\n')
    // `lib/` is declared; an unrelated top-level file is dirty. The lockfile may accompany the change,
    // but an UNDECLARED non-dependency file must still refuse the commit — that behaviour is what makes
    // the declared surface worth anything, and a coupling guard must not erode it.
    //
    // The refusal names every out-of-scope dirty path, and package.json IS one of them here: the
    // manifest is not a companion that can be swept in (only the lockfile is), so a lane that means to
    // change dependencies still has to declare package.json. That is the "refused" half of this
    // story's acceptance, and the lockfile is absent from the refusal because the guard admitted it.
    await writeFile(join(repo, 'README.md'), 'unrelated\n')
    const result = await commitWorkerWorkspaceChanges(repo, 'sneaky', { allowedScope: ['lib/'] })
    assert.equal(result.commitHash, null, 'no commit was made')
    assert.ok(result.refused?.includes('README.md'), 'the unrelated file is named')
    assert.ok(result.refused?.includes(DEPENDENCY_MANIFEST), 'an undeclared manifest is still refused')
    assert.equal(result.refused?.includes(DEPENDENCY_LOCKFILE), false, 'the lockfile is admitted, not refused')
  } finally {
    await rm(repo, { recursive: true, force: true })
  }
})

test('lockfile-coupling: companions are exactly the lockfile, and only when it is dirty', () => {
  assert.deepEqual(dependencyCompanions(['package.json', 'pnpm-lock.yaml']), ['pnpm-lock.yaml'])
  assert.deepEqual(dependencyCompanions(['pnpm-lock.yaml']), ['pnpm-lock.yaml'])
  assert.deepEqual(dependencyCompanions(['package.json', 'lib/a.ts']), [], 'a manifest-only edit needs no companion')
  assert.deepEqual(dependencyCompanions(['lib/a.ts']), [])
})
