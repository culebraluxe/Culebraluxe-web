import assert from 'node:assert/strict'
import test from 'node:test'
import {
  forgeQaVerdictBlock,
  FORGE_QA_ASSAY_ARTIFACT_KIND,
  type ForgeQaArtifactRow,
} from '@/legacy/workflow_app/forge/forge-visibility'

const artifact = (over: Partial<ForgeQaArtifactRow> = {}): ForgeQaArtifactRow => ({
  storyRunId: 'run-1',
  kind: FORGE_QA_ASSAY_ARTIFACT_KIND,
  verdict: 'PASS',
  detail: {
    requiredCommands: ['node --test a.test.ts', 'node --test b.test.ts'],
    commandResults: [
      { command: 'node --test a.test.ts', exitCode: 0 },
      { command: 'node --test b.test.ts', exitCode: 0 },
    ],
  },
  ...over,
})

test('a passing run reports qaPassed with each frozen command and its exit code', () => {
  const block = forgeQaVerdictBlock('run-1', [artifact()])
  assert.ok(block)
  assert.equal(block.runId, 'run-1')
  assert.equal(block.verdict, 'PASS')
  assert.equal(block.qaPassed, true)
  assert.deepEqual(block.commands, [
    { command: 'node --test a.test.ts', exitCode: 0 },
    { command: 'node --test b.test.ts', exitCode: 0 },
  ])
})

test('a run with no rows of its own reports NO verdict, never another run’s', () => {
  assert.equal(forgeQaVerdictBlock('run-1', []), null)
  assert.equal(forgeQaVerdictBlock('run-1', [artifact({ storyRunId: 'run-2' })]), null)
  assert.equal(forgeQaVerdictBlock('run-1', [artifact({ kind: 'static-gate' })]), null)
})

test('a failed command is named with its exit code', () => {
  const block = forgeQaVerdictBlock('run-1', [
    artifact({
      verdict: 'FAIL',
      detail: {
        requiredCommands: ['node --test a.test.ts', 'node --test b.test.ts'],
        commandResults: [
          { command: 'node --test a.test.ts', exitCode: 0 },
          { command: 'node --test b.test.ts', exitCode: 1 },
        ],
      },
    }),
  ])
  assert.ok(block)
  assert.equal(block.verdict, 'FAIL')
  assert.equal(block.qaPassed, false)
  const failed = block.commands.find((c) => c.exitCode !== 0)
  assert.ok(failed)
  assert.equal(failed.command, 'node --test b.test.ts')
  assert.equal(failed.exitCode, 1)
})

test('a frozen command with no measured result reads null, not 0', () => {
  const block = forgeQaVerdictBlock('run-1', [
    artifact({
      detail: {
        requiredCommands: ['node --test a.test.ts', 'node --test missing.test.ts'],
        commandResults: [{ command: 'node --test a.test.ts', exitCode: 0 }],
      },
    }),
  ])
  assert.ok(block)
  const unmeasured = block.commands.find((c) => c.command === 'node --test missing.test.ts')
  assert.ok(unmeasured)
  assert.equal(unmeasured.exitCode, null)
  assert.notEqual(unmeasured.exitCode, 0)
})

test('the newest artifact for the run decides, not the best the run ever had', () => {
  const older = artifact({ verdict: 'PASS' })
  const newer = artifact({
    verdict: 'FAIL',
    detail: { requiredCommands: ['node --test a.test.ts'], commandResults: [] },
  })
  const block = forgeQaVerdictBlock('run-1', [newer, older])
  assert.ok(block)
  assert.equal(block.verdict, 'FAIL')
  assert.equal(block.qaPassed, false)
})

test('a blank verdict reads as no verdict, never FAIL', () => {
  assert.equal(forgeQaVerdictBlock('run-1', [artifact({ verdict: '   ' })]), null)
  assert.equal(forgeQaVerdictBlock('run-1', [artifact({ verdict: null })]), null)
})

test('the verdict carries no sha', () => {
  const block = forgeQaVerdictBlock('run-1', [artifact()])
  assert.ok(block)
  assert.equal('sha' in block, false)
})
