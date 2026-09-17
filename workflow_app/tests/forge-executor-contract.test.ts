import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import { driveForgeStory, planWave, type WaveLane } from '../forge/forge-executor'
import { forgeLaneSurface } from '../forge/agent-runtime-role-runner'
import { shapeArchitectFindings, shapeSizeFloor } from '../forge/forge-shaping'
import { reviewLeadProposal } from '../forge/forge-lead-routing'
import {
  commitWorkerWorkspaceChanges,
  parseAllowedScopeMarker,
  renderAllowedScopeMarker,
} from '../../lib/worker-workspace/commit'

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

test('ENG-FORGE-SURFACE-SUPPLIER-01: the supplier feeds planWave so two non-fanout lanes with disjoint surfaces share one wave', () => {
  const a = forgeLaneSurface({ formData: { surface: ['workflow_app/a.ts'] } })
  const b = forgeLaneSurface({ formData: { surface: ['workflow_app/b.ts'] } })
  assert.deepEqual(a, ['workflow_app/a.ts'])
  assert.deepEqual(b, ['workflow_app/b.ts'])
  const plan = planWave([lane('smith', a), lane('qa_verify', b)], 2)
  assert.equal(plan.ok, true)
  if (!plan.ok) return
  assert.equal(plan.batches.length, 1)
  assert.deepEqual(
    plan.batches[0].map((entry) => entry.lane),
    ['smith', 'qa_verify'],
  )
})

test('ENG-FORGE-SURFACE-SUPPLIER-01: a lane with no declared surface still runs alone', () => {
  const undeclared = forgeLaneSurface({ formData: {} })
  assert.equal(undeclared, null)
  const plan = planWave([lane('smith', undeclared), lane('qa_verify', ['workflow_app/b.ts'])], 2)
  assert.equal(plan.ok, true)
  if (!plan.ok) return
  assert.deepEqual(
    plan.batches.map((batch) => batch.length),
    [1, 1],
  )
})

test('ENG-FORGE-SURFACE-SUPPLIER-01: the declared surface round-trips through the commit marker', () => {
  const surface = ['lib/worker-workspace/commit.ts', 'workflow_app/forge/forge-shaping.ts']
  const rendered = renderAllowedScopeMarker(surface)
  assert.ok(rendered.startsWith('FORGE_ALLOWED_SCOPE:'))
  assert.deepEqual(parseAllowedScopeMarker(rendered), surface)
  assert.equal(parseAllowedScopeMarker('no marker here'), null)
  assert.equal(renderAllowedScopeMarker([]), '')
})

test('ENG-FORGE-SURFACE-SUPPLIER-01: a commit that exists is never reported as nothing', async () => {
  const cwd = repo()
  writeFileSync(join(cwd, 'declared.txt'), 'mine\n')
  const result = await commitWorkerWorkspaceChanges(cwd, 'FORGE: never nothing', {
    allowedScope: ['declared.txt'],
  })
  assert.equal(result.changed, true)
  assert.ok(result.commitHash, 'a commit was made, so its sha must be returned')
  assert.equal(git(cwd, ['rev-parse', 'HEAD']), result.commitHash)
  assert.equal(result.refused, undefined)
})

const FROZEN_PROOF = 'node --import tsx --test workflow_app/tests/forge-executor-contract.test.ts'

test('ENG-FORGE-SURFACE-SUPPLIER-01: the MEDIUM floor comes from the shaper seam groups, not the model rating', () => {
  const oneGroup = shapeArchitectFindings({
    findings: [
      { id: 'a', summary: 'a', required: true, seams: ['workflow_app/a.ts'] },
      { id: 'b', summary: 'b', required: true, seams: ['workflow_app/a.ts'] },
    ],
  })
  assert.equal(oneGroup.units.length, 1)
  assert.equal(shapeSizeFloor(oneGroup), 'SMALL')

  const twoGroups = shapeArchitectFindings({
    findings: [
      { id: 'a', summary: 'a', required: true, seams: ['workflow_app/a.ts'] },
      { id: 'b', summary: 'b', required: true, seams: ['workflow_app/b.ts'] },
      { id: 'c', summary: 'c', required: false, seams: ['workflow_app/c.ts'] },
    ],
  })
  assert.equal(twoGroups.units.length, 2)
  assert.equal(shapeSizeFloor(twoGroups), 'MEDIUM')
})

test('ENG-FORGE-SURFACE-SUPPLIER-01: a SMALL plan over two shaper seam groups is refused', () => {
  const review = reviewLeadProposal(
    {
      version: 1,
      decision: 'SMITH',
      size: 'SMALL',
      sizeReason: 'the model rated every feature 1',
      reason: 'one smith',
      assignments: [
        {
          id: 'a1',
          findingIds: ['a', 'b'],
          dependsOn: [],
          evidenceRefs: ['story_goal'],
          reasoning: 'do the bounded work',
          features: {
            semanticSurface: 1,
            dependencyDepth: 1,
            uncertainty: 1,
            contextBurden: 1,
            proofBurden: 1,
            coupling: 1,
            changeNovelty: 1,
            workerFit: 1,
          },
          plan: {
            size: 'SMALL',
            chunks: [
              {
                id: 1,
                outcome: 'o',
                surface: ['workflow_app/a.ts'],
                invariant: 'i',
                proof: FROZEN_PROOF,
                dependsOn: [],
              },
            ],
          },
        },
      ],
      mergeChecks: [FROZEN_PROOF],
    },
    {
      findings: [
        { id: 'a', required: true, seams: ['workflow_app/a.ts'] },
        { id: 'b', required: true, seams: ['workflow_app/b.ts'] },
      ],
      evidenceRefs: ['story_goal'],
      splitEnabled: true,
      maxSmiths: 2,
      allowedProofs: [FROZEN_PROOF],
    },
  )
  assert.equal(review.ok, false)
  if (review.ok) return
  assert.ok(
    review.errors.some((error) => error.includes('size floor is MEDIUM')),
    `expected a MEDIUM floor error, got: ${review.errors.join(' | ')}`,
  )
})
