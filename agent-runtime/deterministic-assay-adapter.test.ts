import assert from 'node:assert/strict'
import test from 'node:test'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import {
  DeterministicAssayAdapter,
  type AssayCommandRunner,
} from './deterministic-assay-adapter'
import {
  withAssayCandidateDirective,
  withAssayPlanDirective,
} from './assay-evidence'
import type { AgentExecutionContext, AgentWorkCommand } from './types'

function git(cwd: string, args: string[]): string {
  return execFileSync('git', args, { cwd, encoding: 'utf8' }).trim()
}

function workspace(): { cwd: string; sha: string; cleanup: () => void } {
  const cwd = mkdtempSync(join(tmpdir(), 'forge-v6-assay-'))
  git(cwd, ['init', '-b', 'main', '-q'])
  git(cwd, ['config', 'user.email', 'forge-v6@example.com'])
  git(cwd, ['config', 'user.name', 'Forge V6 Test'])
  writeFileSync(join(cwd, 'candidate.txt'), 'candidate\n')
  git(cwd, ['add', '.'])
  git(cwd, ['commit', '-m', 'candidate', '-q'])
  return {
    cwd,
    sha: git(cwd, ['rev-parse', 'HEAD']),
    cleanup: () => rmSync(cwd, { recursive: true, force: true }),
  }
}

function command(instructions: string): AgentWorkCommand {
  return {
    workItemId: 'wi-assay-v6',
    storyId: 'ENG-FORGE-V6',
    role: 'verifier',
    modelProfile: 'verifier-mini',
    specialInstructions: instructions,
    priority: 1,
    state: 'Running',
    claimedBy: 'test',
    claimedAt: null,
    startedAt: null,
    finishedAt: null,
    storyRunId: 'run-assay-v6',
    errorText: null,
    runtimeAdapter: null,
    externalRunId: null,
    attempts: 1,
    maxAttempts: 3,
    executionEnvironment: 'LOCAL',
    createdAt: '2026-09-04T00:00:00.000Z',
    updatedAt: '2026-09-04T00:00:00.000Z',
  }
}

class ExposedAssay extends DeterministicAssayAdapter {
  startForTest(context: AgentExecutionContext) {
    return this.startExternal(context)
  }
  statusForTest(command: AgentWorkCommand, context: AgentExecutionContext) {
    return this.statusExternal(command, context)
  }
  resultForTest(command: AgentWorkCommand, context: AgentExecutionContext) {
    return this.resultExternal(command, context)
  }
}

function context(cwd: string, sha: string, cmd: AgentWorkCommand): AgentExecutionContext {
  return {
    command: cmd,
    story: { id: 'ENG-FORGE-V6', title: 'V6' } as never,
    policy: {
      allowCommit: false,
      allowDevDbWrite: false,
      allowControlPlaneWrite: true,
    },
    capabilities: [],
    executionEnvironment: 'LOCAL',
    executionWorkspace: {
      branchName: 'agent/eng-forge-v6/assay',
      worktreePath: cwd,
      baseRef: sha,
      baseCommit: sha,
      runId: 'run-assay-v6',
    },
    storyRunId: 'run-assay-v6',
  }
}

async function terminalStatus(
  adapter: ExposedAssay,
  cmd: AgentWorkCommand,
  ctx: AgentExecutionContext,
) {
  for (let i = 0; i < 20; i += 1) {
    const status = await adapter.statusForTest(cmd, ctx)
    if (status.lifecycle !== 'running') return status
  }
  throw new Error('Assay did not terminalize')
}

test('deterministic Assay executes immutable commands with no model and returns structured PASS', async () => {
  const w = workspace()
  try {
    const calls: string[] = []
    const runner: AssayCommandRunner = async ({ command }) => {
      calls.push(command)
      return {
        command,
        exitCode: 0,
        signal: null,
        timedOut: false,
        durationMs: 1,
        tests: { total: 26, passed: 26, failed: 0 },
        stdoutTail: 'words: failed commands; numeric facts still clean',
        stderrTail: '',
      }
    }
    const planned = withAssayPlanDirective('plan', {
      mode: 'SCOPED',
      commands: ['verify-one', 'verify-two'],
    })
    const instructions = withAssayCandidateDirective(planned, w.sha)
    const cmd = command(instructions)
    const ctx = context(w.cwd, w.sha, cmd)
    const adapter = new ExposedAssay(
      { work: {} as never, runs: {} as never },
      { runCommand: runner, commandTimeoutMs: 1_000 },
    )

    await adapter.startForTest(ctx)
    const status = await terminalStatus(adapter, cmd, ctx)
    assert.equal(status.lifecycle, 'success')
    const evidence = await adapter.resultForTest(cmd, ctx)

    assert.deepEqual(calls, ['verify-one', 'verify-two'])
    assert.equal(evidence?.runtimeAdapter, 'forge-assay')
    assert.equal(evidence?.resultStatus, 'Complete')
    assert.equal(evidence?.assayEvidence?.verdict, 'PASS')
    assert.equal(evidence?.assayEvidence?.candidateSha, w.sha)
    assert.equal(evidence?.assayEvidence?.verifiedSha, w.sha)
  } finally {
    w.cleanup()
  }
})

test('deterministic Assay stops at first failed command and returns Hold for human intervention', async () => {
  const w = workspace()
  try {
    const calls: string[] = []
    const runner: AssayCommandRunner = async ({ command }) => {
      calls.push(command)
      return {
        command,
        exitCode: 1,
        signal: null,
        timedOut: false,
        durationMs: 1,
        tests: { total: 26, passed: 25, failed: 1 },
        stdoutTail: '',
        stderrTail: 'test failed',
      }
    }
    const planned = withAssayPlanDirective('plan', {
      mode: 'SCOPED',
      commands: ['verify-one', 'must-not-run'],
    })
    const instructions = withAssayCandidateDirective(planned, w.sha)
    const cmd = command(instructions)
    const ctx = context(w.cwd, w.sha, cmd)
    const adapter = new ExposedAssay(
      { work: {} as never, runs: {} as never },
      { runCommand: runner },
    )

    await adapter.startForTest(ctx)
    await terminalStatus(adapter, cmd, ctx)
    const evidence = await adapter.resultForTest(cmd, ctx)

    assert.deepEqual(calls, ['verify-one'])
    assert.equal(evidence?.resultStatus, 'Hold')
    assert.equal(evidence?.assayEvidence?.verdict, 'FAIL')
    assert.equal(evidence?.assayEvidence?.failureCode, 'ASSAY_TEST_FAILED')
  } finally {
    w.cleanup()
  }
})
