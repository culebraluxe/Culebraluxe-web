// ---------------------------------------------------------------------------
// ENG-FORGE-V5-01 — OpenCodeHarnessAdapter focused unit tests.
//
// Proves the TRANSLATION LAYER behaviors deterministically (no Neon, no real
// OpenCode model run):
//   - startExternal runs `opencode run` in the EXACT Forge-provided worker
//     worktree with the model explicitly pinned to deepseek/deepseek-v4-flash
//   - startExternal preserves the canonical Forge task/prompt contract and the
//     DEV-sanitized child environment
//   - startExternal FAILS CLOSED outside the isolated worker worktree
//   - a missing/wrong explicit model fails closed (no default model selection)
//   - status mapping: running -> success; non-zero exit -> failed with
//     truthful error text
//   - successful result mapping records factual run metadata (harness=opencode,
//     model, worktree, exit, elapsed) and the worker commit when present
//   - failed result mapping returns null (the shared base owns terminalization)
//
// The fake startRun substitutes the OpenCode process mechanics; the adapter
// vendor hooks under test are the real production code.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import {
  OpenCodeHarnessAdapter,
  OPENCODE_PINNED_MODEL,
  openCodeModelBlocker,
  resolveOpenCodeModel,
} from './opencode-harness-adapter'
import type { OpenCodeHandle, OpenCodeRunResult } from './opencode-client'
import type {
  AgentExecutionContext,
  AgentExecutionWorkspace,
  AgentWorkCommand,
} from '../types'
import type { StoryboardStory } from '../../db/storyboard'

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

function git(cwd: string, args: string[]): string {
  return execFileSync('git', args, { cwd, encoding: 'utf8' }).trim()
}

function command(overrides: Partial<AgentWorkCommand> = {}): AgentWorkCommand {
  return {
    workItemId: 'wi-opencode-1',
    storyId: 'ENG-FORGE-V5-01',
    role: 'builder',
    modelProfile: 'builder-flash',
    specialInstructions: 'Lane=smith. Implement against the Architect plan.',
    priority: 50,
    state: 'Ready',
    claimedBy: null,
    claimedAt: null,
    startedAt: null,
    finishedAt: null,
    storyRunId: null,
    errorText: null,
    runtimeAdapter: null,
    externalRunId: null,
    attempts: 0,
    maxAttempts: 3,
    executionEnvironment: 'DEV',
    createdAt: '2026-09-03T00:00:00.000Z',
    updatedAt: '2026-09-03T00:00:00.000Z',
    ...overrides,
  }
}

function story(overrides: Partial<StoryboardStory> = {}): StoryboardStory {
  return {
    id: 'ENG-FORGE-V5-01',
    workstream: 'HARDEN',
    operatingSurface: null,
    title: 'OpenCode Harness Adapter',
    priority: 'High',
    status: 'In Progress',
    notes: 'test fixture',
    batch: null,
    goal: 'Prove Forge can use OpenCode as an inner Smith execution harness.',
    scope: 'Smallest possible OpenCode execution adapter.',
    dependencies: null,
    preconditions: null,
    architectBrief: 'OpenCode is an inner execution engine, not a second orchestrator.',
    contextRefs: null,
    acceptanceCriteria: 'Focused adapter + unchanged default routing.',
    postconditions: null,
    architectBriefUpdatedAt: null,
    completion: 0,
    rollup: true,
    plannedStartAt: null,
    actualStartAt: null,
    completedAt: null,
    createdAt: '2026-09-03T00:00:00.000Z',
    updatedAt: '2026-09-03T00:00:00.000Z',
    ...overrides,
  }
}

function makeWorkspace(): { workspace: AgentExecutionWorkspace; cleanup: () => void } {
  const cwd = mkdtempSync(join(tmpdir(), 'forge-opencode-ws-'))
  git(cwd, ['init', '-b', 'main', '-q'])
  git(cwd, ['config', 'user.email', 'opencode-test@example.com'])
  git(cwd, ['config', 'user.name', 'OpenCode Test'])
  writeFileSync(join(cwd, 'base.txt'), 'base\n')
  git(cwd, ['add', '.'])
  git(cwd, ['commit', '-m', 'base', '-q'])
  const baseCommit = git(cwd, ['rev-parse', 'HEAD'])
  return {
    workspace: {
      branchName: 'agent/eng-forge-v5-01/run-test',
      worktreePath: cwd,
      baseRef: 'main',
      baseCommit,
      runId: 'run-test',
    },
    cleanup: () => rmSync(cwd, { recursive: true, force: true }),
  }
}

function context(
  cmd: AgentWorkCommand,
  workspace?: AgentExecutionWorkspace,
): AgentExecutionContext {
  return {
    command: cmd,
    story: story(),
    policy: { allowCommit: true, allowDevDbWrite: true, allowControlPlaneWrite: true },
    capabilities: [],
    executionEnvironment: 'DEV',
    ...(workspace ? { executionWorkspace: workspace } : {}),
    storyRunId: '',
  }
}

const DEPS = { work: {} as never, runs: {} as never }

function handleFor(
  result: OpenCodeRunResult,
  opts: { done?: boolean; cancelled?: boolean } = {},
): OpenCodeHandle {
  const handle: OpenCodeHandle = {
    proc: { exitCode: result.exitCode, killed: false } as never,
    done: opts.done ?? true,
    promise: Promise.resolve(result),
    cancelled: opts.cancelled ?? false,
    pause: () => undefined,
    resume: () => undefined,
    cancel: () => undefined,
  }
  return handle
}

// ---------------------------------------------------------------------------
// Model pinning helpers
// ---------------------------------------------------------------------------

test('resolveOpenCodeModel returns the pinned model only', () => {
  assert.equal(resolveOpenCodeModel(OPENCODE_PINNED_MODEL), OPENCODE_PINNED_MODEL)
  assert.equal(resolveOpenCodeModel(undefined), OPENCODE_PINNED_MODEL)
})

test('resolveOpenCodeModel fails closed on a blank or different model', () => {
  assert.throws(() => resolveOpenCodeModel(''), /no explicit model configuration/)
  assert.throws(() => resolveOpenCodeModel('   '), /no explicit model configuration/)
  assert.throws(
    () => resolveOpenCodeModel('anthropic/claude-sonnet'),
    /not the ENG-FORGE-V5-01 pinned model/,
  )
})

test('openCodeModelBlocker names the missing-model blocker without throwing', () => {
  assert.match(openCodeModelBlocker('')!, /no explicit model configuration/)
  assert.match(openCodeModelBlocker('other/model')!, /pinned model/)
  assert.equal(openCodeModelBlocker(OPENCODE_PINNED_MODEL), null)
  assert.equal(openCodeModelBlocker(undefined), null)
})

// ---------------------------------------------------------------------------
// startExternal
// ---------------------------------------------------------------------------

test('startExternal runs opencode in the exact worker worktree with the pinned model', async () => {
  const { workspace, cleanup } = makeWorkspace()
  try {
    const captured: Array<Record<string, unknown>> = []
    const adapter = new OpenCodeHarnessAdapter(DEPS, {
      cliBin: 'opencode',
      workspace: process.cwd(),
      model: OPENCODE_PINNED_MODEL,
      startRun: (opts) => {
        captured.push({ ...opts })
        return handleFor({ status: 'success', exitCode: 0, stdout: '', stderr: '' })
      },
    })
    const start = await (adapter as any).startExternal(context(command(), workspace))
    assert.ok(start.externalRunId.startsWith('opencode-'))
    assert.equal(captured.length, 1)
    // AC2: the exact Forge-provided worker worktree is the cwd.
    assert.equal(captured[0].cwd, workspace.worktreePath)
    // AC3: the model is always passed explicitly — never a default.
    assert.equal(captured[0].model, OPENCODE_PINNED_MODEL)
    assert.equal(captured[0].cliBin, 'opencode')
    // AC4: the canonical Forge task/prompt contract is the run message.
    assert.match(String(captured[0].task), /^Execute SDLC story ENG-FORGE-V5-01/)
    assert.match(String(captured[0].task), /Architect brief/)
    // DEV-safe child env: forced DEV target, no PROD application URL.
    const env = captured[0].env as Record<string, string | undefined>
    assert.equal(env.EXECUTION_ENV, 'DEV')
    assert.equal(env.APP_ENV, 'development')
    assert.equal(env.DATABASE_URL_PROD, undefined)
  } finally {
    cleanup()
  }
})

test('startExternal refuses to run outside the Forge-provided isolated worktree', async () => {
  const adapter = new OpenCodeHarnessAdapter(DEPS, {
    cliBin: 'opencode',
    workspace: process.cwd(),
    model: OPENCODE_PINNED_MODEL,
    startRun: (() => {
      throw new Error('startRun must not be reached without isolation')
    }) as never,
  })
  await assert.rejects(
    () => (adapter as any).startExternal(context(command())),
    /isolated worker worktree/,
  )
})

test('startExternal fails closed when the explicit model is missing or different', async () => {
  const { workspace, cleanup } = makeWorkspace()
  try {
    for (const model of ['', 'deepseek/deepseek-v3']) {
      const adapter = new OpenCodeHarnessAdapter(DEPS, {
        cliBin: 'opencode',
        workspace: process.cwd(),
        model,
        startRun: (() => {
          throw new Error('startRun must not be reached without an explicit pinned model')
        }) as never,
      })
      await assert.rejects(
        () => (adapter as any).startExternal(context(command(), workspace)),
        /model/,
      )
    }
  } finally {
    cleanup()
  }
})

// ---------------------------------------------------------------------------
// statusExternal
// ---------------------------------------------------------------------------

test('status maps a live run to running and then to success', async () => {
  const { workspace, cleanup } = makeWorkspace()
  try {
    const handle = handleFor(
      { status: 'success', exitCode: 0, stdout: 'done', stderr: '' },
      { done: false },
    )
    const adapter = new OpenCodeHarnessAdapter(DEPS, {
      cliBin: 'opencode',
      workspace: process.cwd(),
      model: OPENCODE_PINNED_MODEL,
      startRun: () => handle,
    })
    await (adapter as any).startExternal(context(command(), workspace))
    const running = await (adapter as any).statusExternal(command(), context(command(), workspace))
    assert.equal(running.lifecycle, 'running')
    handle.done = true
    const success = await (adapter as any).statusExternal(command(), context(command(), workspace))
    assert.equal(success.lifecycle, 'success')
  } finally {
    cleanup()
  }
})

test('a non-zero OpenCode run maps to failed with truthful error text', async () => {
  const { workspace, cleanup } = makeWorkspace()
  try {
    const adapter = new OpenCodeHarnessAdapter(DEPS, {
      cliBin: 'opencode',
      workspace: process.cwd(),
      model: OPENCODE_PINNED_MODEL,
      startRun: () =>
        handleFor({ status: 'failed', exitCode: 7, stdout: '', stderr: 'opencode boom' }),
    })
    await (adapter as any).startExternal(context(command(), workspace))
    const status = await (adapter as any).statusExternal(command(), context(command(), workspace))
    assert.equal(status.lifecycle, 'failed')
    assert.match((adapter as any).externalErrorText, /opencode boom/)
  } finally {
    cleanup()
  }
})

test('a cancelled OpenCode run maps to cancelled', async () => {
  const { workspace, cleanup } = makeWorkspace()
  try {
    const adapter = new OpenCodeHarnessAdapter(DEPS, {
      cliBin: 'opencode',
      workspace: process.cwd(),
      model: OPENCODE_PINNED_MODEL,
      startRun: () =>
        handleFor(
          { status: 'failed', exitCode: null, stdout: '', stderr: '' },
          { cancelled: true },
        ),
    })
    await (adapter as any).startExternal(context(command(), workspace))
    const status = await (adapter as any).statusExternal(command(), context(command(), workspace))
    assert.equal(status.lifecycle, 'cancelled')
  } finally {
    cleanup()
  }
})

// ---------------------------------------------------------------------------
// resultExternal
// ---------------------------------------------------------------------------

test('successful run maps to Complete evidence with factual metadata', async () => {
  const { workspace, cleanup } = makeWorkspace()
  try {
    const stdout = [
      'Implemented the tiny adapter.',
      'Verification (SCOPED): ran the focused opencode adapter tests.',
      'Tests: agent-runtime/opencode/opencode-harness-adapter.test.ts 5/5 pass',
    ].join('\n')
    const adapter = new OpenCodeHarnessAdapter(DEPS, {
      cliBin: 'opencode',
      workspace: process.cwd(),
      model: OPENCODE_PINNED_MODEL,
      startRun: () => handleFor({ status: 'success', exitCode: 0, stdout, stderr: '' }),
    })
    const cmd = command()
    await (adapter as any).startExternal(context(cmd, workspace))
    const evidence = await (adapter as any).resultExternal(cmd, context(cmd, workspace))

    assert.equal(evidence.resultStatus, 'Complete')
    assert.equal(evidence.completion, 100)
    assert.equal(evidence.runtimeAdapter, 'opencode-harness')
    assert.equal(evidence.modelProfile, 'builder-flash')
    assert.equal(evidence.executionEnvironment, 'DEV')
    assert.equal(evidence.commitHash, null, 'still at base -> no fabricated commit')
    // Run metadata: harness/model/worktree/exit (+ elapsed when practical).
    assert.match(evidence.notes, /harness=opencode/)
    assert.match(evidence.notes, /model=deepseek\/deepseek-v4-flash/)
    assert.match(evidence.notes, new RegExp(`worktree=${escapeRegExp(workspace.worktreePath)}`))
    assert.match(evidence.notes, /exit=0/)
    assert.match(evidence.notes, /Execution workspace: branch=agent\/eng-forge-v5-01\/run-test/)
    // ENG-08 summary extraction from the model's Tests: line.
    assert.equal(
      evidence.testsSummary,
      'agent-runtime/opencode/opencode-harness-adapter.test.ts 5/5 pass',
    )
  } finally {
    cleanup()
  }
})

test('a worker commit in the worktree is read as the factual candidate (AC5)', async () => {
  const { workspace, cleanup } = makeWorkspace()
  try {
    writeFileSync(join(workspace.worktreePath, 'candidate.txt'), 'candidate\n')
    git(workspace.worktreePath, ['add', 'candidate.txt'])
    git(workspace.worktreePath, ['commit', '-m', 'opencode worker commit', '-q'])
    const head = git(workspace.worktreePath, ['rev-parse', 'HEAD'])

    const adapter = new OpenCodeHarnessAdapter(DEPS, {
      cliBin: 'opencode',
      workspace: process.cwd(),
      model: OPENCODE_PINNED_MODEL,
      startRun: () =>
        handleFor({ status: 'success', exitCode: 0, stdout: 'done', stderr: '' }),
    })
    const cmd = command()
    await (adapter as any).startExternal(context(cmd, workspace))
    const evidence = await (adapter as any).resultExternal(cmd, context(cmd, workspace))
    assert.equal(evidence.commitHash, head)
  } finally {
    cleanup()
  }
})

test('failed result maps to null (shared base terminalizes as failure)', async () => {
  const { workspace, cleanup } = makeWorkspace()
  try {
    const adapter = new OpenCodeHarnessAdapter(DEPS, {
      cliBin: 'opencode',
      workspace: process.cwd(),
      model: OPENCODE_PINNED_MODEL,
      startRun: () =>
        handleFor({ status: 'failed', exitCode: 1, stdout: '', stderr: 'nope' }),
    })
    const cmd = command()
    await (adapter as any).startExternal(context(cmd, workspace))
    const evidence = await (adapter as any).resultExternal(cmd, context(cmd, workspace))
    assert.equal(evidence, null)
    assert.match((adapter as any).externalErrorText, /nope/)
  } finally {
    cleanup()
  }
})

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}
