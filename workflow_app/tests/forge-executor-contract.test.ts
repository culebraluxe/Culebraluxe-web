import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import { driveForgeStory, planWave, type WaveLane } from '../forge/forge-executor'
import { commitWorkerWorkspaceChanges } from '../../lib/worker-workspace/commit'

test('ENG-FORGE-V10: production driver refuses to invent a role runner', async () => {
  await assert.rejects(
    () => driveForgeStory('ENG-FORGE-V10'),
    /requires an explicit real role runner/i,
  )
})

function lane(name: string, surface: string[] | null): WaveLane<string> {
  return { lane: name, surface, task: name }
}

test('ENG-FORGE-PARALLEL-WAVE-01: two non-split ready lanes share a wave under the cap', () => {
  const plan = planWave(
    [lane('smith', ['workflow_app/a.ts']), lane('qa_verify', ['workflow_app/b.ts'])],
    2,
  )
  assert.equal(plan.ok, true)
  if (!plan.ok) return
  assert.equal(plan.batches.length, 1)
  assert.deepEqual(
    plan.batches[0].map((entry) => entry.lane),
    ['smith', 'qa_verify'],
  )
})

test('ENG-FORGE-PARALLEL-WAVE-01: a one-task wave runs exactly once', () => {
  const plan = planWave([lane('smith', ['workflow_app/a.ts'])], 2)
  assert.equal(plan.ok, true)
  if (!plan.ok) return
  assert.equal(plan.batches.length, 1)
  assert.equal(plan.batches[0].length, 1)
  assert.equal(plan.batches[0][0].lane, 'smith')
})

test('ENG-FORGE-CONTRACT-ONE-WRITER-01: an overlapping pair is deferred and a disjoint lane still runs', () => {
  const plan = planWave(
    [
      lane('smith', ['workflow_app/a.ts']),
      lane('qa_verify', ['workflow_app/a.ts']),
      lane('inspector', ['workflow_app/c.ts']),
    ],
    2,
  )
  assert.equal(plan.ok, true)
  const placement = new Map<string, number>()
  plan.batches.forEach((batch, index) => {
    batch.forEach((entry) => placement.set(entry.lane, index))
  })
  assert.ok(placement.has('inspector'), 'the disjoint lane must still be scheduled')
  assert.ok(placement.has('smith') && placement.has('qa_verify'), 'no lane may be dropped')
  assert.notEqual(
    placement.get('smith'),
    placement.get('qa_verify'),
    'the overlapping pair must not share a batch',
  )
  assert.equal(plan.refusals.length, 1)
  assert.deepEqual(plan.refusals[0].lanes, ['smith', 'qa_verify'])
  assert.equal(plan.refusals[0].path, 'workflow_app/a.ts')
})

test('ENG-FORGE-CONTRACT-ONE-WRITER-01: a lone overlapping pair still progresses in separate batches', () => {
  const plan = planWave(
    [lane('smith', ['workflow_app/a.ts']), lane('qa_verify', ['workflow_app/a.ts'])],
    2,
  )
  assert.equal(plan.ok, true)
  assert.deepEqual(
    plan.batches.map((batch) => batch.map((entry) => entry.lane)),
    [['smith'], ['qa_verify']],
  )
  assert.deepEqual(plan.refusals, [{ lanes: ['smith', 'qa_verify'], path: 'workflow_app/a.ts' }])
})

test('ENG-FORGE-PARALLEL-WAVE-01: an undeclared surface never runs concurrently', () => {
  const plan = planWave([lane('smith', null), lane('qa_verify', ['workflow_app/b.ts'])], 2)
  assert.equal(plan.ok, true)
  if (!plan.ok) return
  assert.deepEqual(
    plan.batches.map((batch) => batch.length),
    [1, 1],
  )
})

function git(cwd: string, args: string[]): string {
  return execFileSync('git', args, { cwd, encoding: 'utf8' }).trim()
}

function repo(): string {
  const cwd = mkdtempSync(join(tmpdir(), 'forge-wave-commit-'))
  git(cwd, ['init'])
  git(cwd, ['config', 'user.email', 'forge-test@example.com'])
  git(cwd, ['config', 'user.name', 'Forge Test'])
  writeFileSync(join(cwd, 'base.txt'), 'base\n')
  git(cwd, ['add', 'base.txt'])
  git(cwd, ['commit', '-m', 'base'])
  return cwd
}

test('ENG-FORGE-PARALLEL-WAVE-01: a commit carrying an undeclared path is refused by name', async () => {
  const cwd = repo()
  const head = git(cwd, ['rev-parse', 'HEAD'])
  writeFileSync(join(cwd, 'declared.txt'), 'mine\n')
  writeFileSync(join(cwd, 'other-writer.txt'), 'not mine\n')

  const result = await commitWorkerWorkspaceChanges(cwd, 'FORGE: declared only', {
    allowedScope: ['declared.txt'],
  })

  assert.equal(result.commitHash, null)
  assert.equal(result.changed, false)
  assert.deepEqual(result.refused, ['other-writer.txt'])
  assert.equal(git(cwd, ['rev-parse', 'HEAD']), head)
})

test('ENG-FORGE-PARALLEL-WAVE-01: a declared-only commit carries only declared paths', async () => {
  const cwd = repo()
  writeFileSync(join(cwd, 'declared.txt'), 'mine\n')

  const result = await commitWorkerWorkspaceChanges(cwd, 'FORGE: declared only', {
    allowedScope: ['declared.txt'],
  })

  assert.equal(result.changed, true)
  assert.ok(result.commitHash)
  assert.deepEqual(
    git(cwd, ['show', '--name-only', '--format=', 'HEAD'])
      .split('\n')
      .map((line) => line.trim())
      .filter(Boolean),
    ['declared.txt'],
  )
})
