import assert from 'node:assert/strict'
import { execFile, execFileSync } from 'node:child_process'
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'
import { promisify } from 'node:util'

import { candidateOwnChangedFiles } from '@/legacy/workflow_app/forge/story-scope-base'
import { scopeViolations, type SmithExecutionContract } from '@/legacy/workflow_app/forge/smith-contract'
import { serialScopeMissReasons } from '@/legacy/workflow_app/forge/forge-serial-doors'
import { gitBinary } from '@/lib/worker-workspace/provisioner'

// ENG-FORGE-SCOPE-OWN-CHANGES-01 — a lane is judged by ITS OWN changes, not by every
// commit that landed while it ran. Measured 2026-09-18: a SPLIT child was held for nine
// files, every one of them the operator's, committed to main while the child sat in
// flight. The candidate's range contained them; the candidate did not.

const run = promisify(execFile)
async function git(cwd: string, args: string[]): Promise<string> {
  const { stdout } = await run(gitBinary(), args, { cwd })
  return stdout.trim()
}

const identity = ['-c', 'user.email=t@t', '-c', 'user.name=t']

async function commit(cwd: string, file: string, body: string, message: string): Promise<string> {
  const full = join(cwd, file)
  await mkdir(join(full, '..'), { recursive: true })
  await writeFile(full, body)
  await git(cwd, [...identity, 'add', '-A'])
  await git(cwd, [...identity, 'commit', '-q', '-m', message])
  return git(cwd, ['rev-parse', 'HEAD'])
}

/** The production reads are synchronous (`readGit`/`isAncestor`); mirror them here. */
const syncGit = (cwd: string, args: string[]): string => {
  try {
    return execFileSync(gitBinary(), args, { cwd, encoding: 'utf8' }).trim()
  } catch {
    return ''
  }
}
const readsFor = (cwd: string) => ({
  readChangedFiles: (commitSha: string) => {
    const out = syncGit(cwd, ['diff', '--name-only', `${commitSha}^`, commitSha])
    return out ? out.split('\n') : []
  },
  isAncestor: (ancestor: string, descendant: string) => {
    try {
      execFileSync(gitBinary(), ['merge-base', '--is-ancestor', ancestor, descendant], {
        cwd,
        stdio: 'ignore',
      })
      return true
    } catch {
      return false
    }
  },
})

const contract = (over: Partial<SmithExecutionContract> = {}): SmithExecutionContract => ({
  identity: { storyId: 's', nodeId: 'smith', attempt: 1, owner: 'a' },
  objective: 'do the thing',
  requiredInputs: [],
  allowedScope: ['legacy/workflow_app/forge/a.ts'],
  prohibitedScope: [],
  expectedOutputs: [],
  requiredEvidence: [],
  dependsOn: [],
  ...over,
})

/** base → foreign (operator) → lane clean. The foreign commit is in the interval, not the lane. */
async function makeRange() {
  const dir = await mkdtemp(join(tmpdir(), 'forge-scope-own-'))
  await git(dir, ['init', '-q', '-b', 'main'])
  const baseSha = await commit(dir, 'legacy/workflow_app/forge/base.ts', 'export const x = 1\n', 'base')
  const foreignSha = await commit(dir, 'docs/foreign.md', '# foreign\n', 'foreign: operator commit')
  const cleanSha = await commit(dir, 'legacy/workflow_app/forge/a.ts', 'export const a = 1\n', 'lane: clean')
  return { dir, baseSha, foreignSha, cleanSha }
}

test('candidateOwnChanges excludes a foreign commit that landed in the range', async () => {
  const { dir, baseSha, cleanSha } = await makeRange()
  const own = candidateOwnChangedFiles({
    candidateSha: cleanSha,
    recordedBase: baseSha,
    laneCommits: [cleanSha],
    ...readsFor(dir),
  })
  assert.equal(own.ok, true)
  if (!own.ok) return
  assert.deepEqual(own.changedFiles, ['legacy/workflow_app/forge/a.ts'])
  assert.ok(!own.changedFiles.includes('docs/foreign.md'))
  // Sanity: the interval really does contain the foreign commit, so excluding it is the rule.
  const interval = syncGit(dir, ['diff', '--name-only', baseSha, cleanSha]).split('\n')
  assert.ok(interval.includes('docs/foreign.md'), 'the interval contains the foreign commit')
})

test('candidateOwnChanges includes every commit the lane authored in the range', async () => {
  const { dir, baseSha, cleanSha } = await makeRange()
  const laneOutOfScope = await commit(dir, 'app/api/evil.ts', 'export const evil = 1\n', 'lane: out of scope')
  const own = candidateOwnChangedFiles({
    candidateSha: laneOutOfScope,
    recordedBase: baseSha,
    laneCommits: [cleanSha, laneOutOfScope],
    ...readsFor(dir),
  })
  assert.equal(own.ok, true)
  if (!own.ok) return
  assert.deepEqual(own.changedFiles, ['app/api/evil.ts', 'legacy/workflow_app/forge/a.ts'])
  assert.ok(!own.changedFiles.includes('docs/foreign.md'))
})

test('a genuine out-of-scope edit by the lane is still refused by name', async () => {
  const { dir, baseSha, cleanSha } = await makeRange()
  const laneOutOfScope = await commit(dir, 'app/api/evil.ts', 'export const evil = 1\n', 'lane: out of scope')
  const own = candidateOwnChangedFiles({
    candidateSha: laneOutOfScope,
    recordedBase: baseSha,
    laneCommits: [cleanSha, laneOutOfScope],
    ...readsFor(dir),
  })
  assert.equal(own.ok, true)
  if (!own.ok) return
  const violations = scopeViolations(contract(), own.changedFiles)
  assert.deepEqual(violations, ['app/api/evil.ts'])
  assert.deepEqual(serialScopeMissReasons(violations, 'a'), ['smith-scope:app/api/evil.ts is outside a'])
})

test('a candidate that is not a descendant of its recorded base is refused', async () => {
  const { dir, baseSha } = await makeRange()
  await git(dir, ['checkout', '-q', '--orphan', 'orphan'])
  const orphanSha = await commit(dir, 'elsewhere.ts', 'export const z = 1\n', 'orphan')
  const own = candidateOwnChangedFiles({
    candidateSha: orphanSha,
    recordedBase: baseSha,
    laneCommits: [orphanSha],
    ...readsFor(dir),
  })
  assert.equal(own.ok, false)
  if (own.ok) return
  assert.match(own.reason, /not a descendant of its recorded base/)
  assert.ok(own.reason.includes(orphanSha))
  assert.ok(own.reason.includes(baseSha))
})

test('the clean case yields no scope violation', async () => {
  const { dir, baseSha, cleanSha } = await makeRange()
  const own = candidateOwnChangedFiles({
    candidateSha: cleanSha,
    recordedBase: baseSha,
    laneCommits: [cleanSha],
    ...readsFor(dir),
  })
  assert.equal(own.ok, true)
  if (!own.ok) return
  assert.deepEqual(scopeViolations(contract(), own.changedFiles), [])
})
