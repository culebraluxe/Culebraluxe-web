import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'
import { promisify } from 'node:util'

import { scopeViolations, type SmithExecutionContract } from '../forge/smith-contract'
import { changedFilesForCandidate } from '../../lib/worker-workspace/candidate-diff'

// ENG-FORGE-SPLIT-01 — the last lock: a child's ACTUAL diff must be inside its
// assignment. Before this, scope was declared (contract + prose) but never checked,
// so a confused Smith could touch a sibling file, present a SHA, and the join would
// pass because it only asked "did every child produce a SHA?".

const contract = (over: Partial<SmithExecutionContract> = {}): SmithExecutionContract => ({
  identity: { storyId: 's', nodeId: 'smith_split_work', attempt: 1, owner: 'a' },
  objective: 'do the thing',
  requiredInputs: [],
  allowedScope: ['workflow_app/forge/a.ts', 'workflow_app/tests/a.test.ts'],
  prohibitedScope: ['workflow_app/forge/b.ts', 'workflow_app/tests/b.test.ts'],
  expectedOutputs: [],
  requiredEvidence: [],
  dependsOn: [],
  ...over,
})

test('a change inside the assignment is clean', () => {
  assert.deepEqual(
    scopeViolations(contract(), ['workflow_app/forge/a.ts', 'workflow_app/tests/a.test.ts']),
    [],
  )
})

test("a sibling's file is a violation even when it otherwise looks fine", () => {
  const violations = scopeViolations(contract(), ['workflow_app/forge/a.ts', 'workflow_app/forge/b.ts'])
  assert.deepEqual(violations, ['workflow_app/forge/b.ts'])
})

test('an unrelated file outside every scope is a violation (fail closed)', () => {
  assert.deepEqual(scopeViolations(contract(), ['app/api/something/route.ts']), [
    'app/api/something/route.ts',
  ])
})

test('symbol-level scope entries accept the whole FILE (git reports files, not symbols)', () => {
  // The live false positive: scope said `file.ts#symbol`, the diff said `file.ts`.
  const c = contract({
    allowedScope: ['workflow_app/forge/a.ts#doTheThing', 'workflow_app/tests/a.test.ts#case'],
    prohibitedScope: ['workflow_app/forge/b.ts#otherThing'],
  })
  assert.deepEqual(scopeViolations(c, ['workflow_app/forge/a.ts', 'workflow_app/tests/a.test.ts']), [])
})

test("a sibling's symbol-level surface still forbids the whole sibling file", () => {
  const c = contract({
    allowedScope: ['workflow_app/forge/a.ts#doTheThing'],
    prohibitedScope: ['workflow_app/forge/b.ts#otherThing'],
  })
  assert.deepEqual(scopeViolations(c, ['workflow_app/forge/b.ts']), ['workflow_app/forge/b.ts'])
})
test('an empty allowedScope refuses every change', () => {
  assert.deepEqual(scopeViolations(contract({ allowedScope: [] }), ['x.ts']), ['x.ts'])
})

test("allowedScope '*' still means anything goes", () => {
  assert.deepEqual(scopeViolations(contract({ allowedScope: ['*'], prohibitedScope: [] }), ['a/b/c.ts']), [])
})

// ---- the git side: what did the candidate actually change? --------------------

const run = promisify(execFile)
async function git(cwd: string, args: string[]): Promise<string> {
  const { stdout } = await run('git', args, { cwd })
  return stdout.trim()
}

test('changedFilesForCandidate reports the child diff from its merge base', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'forge-candidate-diff-'))
  const identity = ['-c', 'user.email=t@t', '-c', 'user.name=t']
  await git(dir, ['init', '-q', '-b', 'main'])
  await mkdir(join(dir, 'workflow_app', 'forge'), { recursive: true })
  await writeFile(join(dir, 'workflow_app', 'forge', 'base.ts'), 'export const x = 1\n')
  await git(dir, [...identity, 'add', '-A'])
  await git(dir, [...identity, 'commit', '-q', '-m', 'base'])
  const baseSha = await git(dir, ['rev-parse', 'HEAD'])

  // The child works on its own branch and changes exactly one file.
  await git(dir, ['checkout', '-q', '-b', 'child'])
  await writeFile(join(dir, 'workflow_app', 'forge', 'a.ts'), 'export const a = 1\n')
  await git(dir, [...identity, 'add', '-A'])
  await git(dir, [...identity, 'commit', '-q', '-m', 'child work'])
  const childSha = await git(dir, ['rev-parse', 'HEAD'])

  const changed = await changedFilesForCandidate({ cwd: dir, baseRef: baseSha, candidateSha: childSha })
  assert.deepEqual(changed, ['workflow_app/forge/a.ts'])
  // And that diff is clean against the child's own assignment...
  assert.deepEqual(scopeViolations(contract(), changed), [])
  // ...but would be a violation for a contract that only owned another file.
  assert.deepEqual(scopeViolations(contract({ allowedScope: ['workflow_app/forge/base.ts'], prohibitedScope: [] }), changed), [
    'workflow_app/forge/a.ts',
  ])
})
