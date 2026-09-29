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
// ENG-FORGE-V5-02 adds the exported execution-identity surface
// (openCodeExecutionIdentity / OPENCODE_HARNESS_ADAPTER_ID): the identity
// record reports the pinned adapter `opencode-harness` + model
// `deepseek/deepseek-v4-flash`, the adapter class field reads the same
// constant, and persisted evidence runtime_adapter agrees with the identity.
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
  forgeSessionContinuityEnabled,
  forgeSessionMarkerPath,
  OPENCODE_HARNESS_ADAPTER_ID,
  OpenCodeHarnessAdapter,
  OPENCODE_PINNED_MODEL,
  openCodeExecutionIdentity,
  openCodeModelBlocker,
  resolveOpenCodeModel,
  SESSION_CONTINUITY_ENV,
} from './opencode-harness-adapter'
import type { OpenCodeHandle, OpenCodeRunResult } from './opencode-client'
import { verifiedShaFromWorkspaceEvidence } from '../candidate-assay-handoff'
import type {
  AgentExecutionContext,
  AgentExecutionWorkspace,
  AgentWorkCommand,
} from '../types'
import type { StoryboardStory } from '@/legacy/db/storyboard'

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
    batchDeploy: false,
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
// OpenCode execution identity (ENG-FORGE-V5-02)
// ---------------------------------------------------------------------------

test('openCodeExecutionIdentity reports the pinned harness and model (ENG-FORGE-V5-02)', () => {
  const identity = openCodeExecutionIdentity()
  // Exact literals: adapter `opencode-harness`, model `deepseek/deepseek-v4-flash`.
  assert.deepEqual(identity, {
    runtimeAdapter: 'opencode-harness',
    model: 'deepseek/deepseek-v4-flash',
  })
  assert.equal(identity.runtimeAdapter, OPENCODE_HARNESS_ADAPTER_ID)
  assert.equal(identity.model, OPENCODE_PINNED_MODEL)
})

test('the adapter runtimeAdapterId equals the identity record (single source of truth)', () => {
  const adapter = new OpenCodeHarnessAdapter(DEPS, {
    cliBin: 'opencode',
    workspace: process.cwd(),
    model: OPENCODE_PINNED_MODEL,
    startRun: () =>
      handleFor({ status: 'success', exitCode: 0, stdout: '', stderr: '' }),
  })
  assert.equal(adapter.runtimeAdapterId, openCodeExecutionIdentity().runtimeAdapter)
  assert.equal(adapter.runtimeAdapterId, 'opencode-harness')
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

test('startExternal runs in the working directory it was given (NO TREES, 2026-09-16)', async () => {
  // This test replaced `startExternal refuses to run outside the Forge-provided isolated worktree`: that
  // refusal was the bug. The estate was deleted, the invoker stopped handing out a workspace (51750b83),
  // and the refusal then killed the architect lane on every start (engine task 65f40df2, 07:26) with
  // "requires the Forge-provisioned isolated worker worktree". No lane owns a tree; each one runs in the
  // directory it was given.
  const captured: Array<Record<string, unknown>> = []
  const adapter = new OpenCodeHarnessAdapter(DEPS, {
    cliBin: 'opencode',
    workspace: process.cwd(),
    model: OPENCODE_PINNED_MODEL,
    startRun: (opts) => {
      captured.push({ ...opts })
      return handleFor({ status: 'success', exitCode: 0, stdout: 'done', stderr: '' })
    },
  })
  // context(command()) carries NO executionWorkspace.
  const start = await (adapter as any).startExternal(context(command()))
  assert.ok(start.externalRunId.startsWith('opencode-'))
  assert.equal(captured.length, 1, 'the harness is started, not refused')
  assert.equal(captured[0].cwd, process.cwd())
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
    // AC1 (ENG-FORGE-V5-02): the persisted runtime_adapter equals the exported
    // execution identity — one source of truth for operator/test visibility.
    assert.equal(evidence.runtimeAdapter, openCodeExecutionIdentity().runtimeAdapter)
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

test('NO TREES: a run with no executionWorkspace still yields evidence (not null)', async () => {
  // The architect lane's death, pinned: `startExternal` shed its demand for a Forge-provisioned
  // worktree in 53556ce7, but `resultExternal` kept the SAME refusal and answered `null`, so the
  // base class died on `result!.notes` and the engine ledger recorded
  // `Cannot read properties of null (reading 'notes')` instead of the run's real result
  // (engine tasks e599df3a, d4266df9, 2026-09-16).
  const adapter = new OpenCodeHarnessAdapter(DEPS, {
    cliBin: 'opencode',
    workspace: process.cwd(),
    model: OPENCODE_PINNED_MODEL,
    startRun: () =>
      handleFor({ status: 'success', exitCode: 0, stdout: 'Tests: 3/3 pass', stderr: '' }),
  })
  const cmd = command()
  // context(cmd) carries NO executionWorkspace — the world after NO TREES.
  await (adapter as any).startExternal(context(cmd))
  const evidence = await (adapter as any).resultExternal(cmd, context(cmd))

  assert.ok(evidence, 'a successful run must produce evidence, not null')
  assert.equal(evidence.resultStatus, 'Complete')
  assert.equal(evidence.completion, 100)
  // It measured the directory it was given, and it says so factually.
  assert.match(evidence.notes, new RegExp(`worktree=${escapeRegExp(process.cwd())}`))
  // No isolated workspace means no `base=<ref>@<sha>` line, so nothing can read this run as proof
  // that a candidate was verified — `verifiedShaFromWorkspaceEvidence` must honestly find nothing.
  assert.equal(verifiedShaFromWorkspaceEvidence(evidence.notes), null)
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

test('NO TREES: a lane that committed nothing reports NO commit, not the inherited HEAD', async () => {
  // Measured 2026-09-16 (ENG-QA-SINGLE-VERDICT-01): the smith created 08b569f8, and the lead_post run and
  // BOTH dev_ops runs each recorded 08b569f8 as their OWN commit — a commit-capable lane that committed
  // nothing claimed whatever HEAD it happened to find. The baseline is now where HEAD stood when the lane
  // STARTED. This covers the no-workspace path (the one that leaked); the isolated path's "HEAD advanced past
  // the approved base" rule is the AC5 test above, unchanged.
  const headBefore = git(process.cwd(), ['rev-parse', 'HEAD'])
  const adapter = new OpenCodeHarnessAdapter(DEPS, {
    cliBin: 'opencode',
    workspace: process.cwd(),
    model: OPENCODE_PINNED_MODEL,
    // The model does nothing, so HEAD cannot move.
    startRun: () => handleFor({ status: 'success', exitCode: 0, stdout: 'nothing to do', stderr: '' }),
  })
  const cmd = command()
  // context(cmd) carries NO executionWorkspace, so the lane runs in the working directory it was given.
  await (adapter as any).startExternal(context(cmd))
  const evidence = await (adapter as any).resultExternal(cmd, context(cmd))

  assert.ok(evidence, 'a successful run still produces evidence')
  assert.equal(evidence.commitHash, null, 'this lane committed nothing — it claims nothing')
  // The fact behind the null: the checkout really did sit still.
  assert.equal(git(process.cwd(), ['rev-parse', 'HEAD']), headBefore)
})

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

test('V5-21 session continuity is ON by default with an explicit opt-out (ENG-FORGE-WARM-SESSION-01)', () => {
  const env = (x: Record<string, string>): NodeJS.ProcessEnv => x as unknown as NodeJS.ProcessEnv
  // The rule inverted after this test was written: one live session per execution generation is the
  // DESIGN, not an experiment (a cold-starting role pays to re-read what the previous role already read),
  // so continuity is on unless the operator opts out. Recorded here because the stale expectation — not
  // the code — was the defect: the suite was red for it while the behaviour was documented and intended.
  assert.equal(forgeSessionContinuityEnabled(env({})), true, 'must be default ON')
  assert.equal(forgeSessionContinuityEnabled(env({ FORGE_SESSION_CONTINUITY: '0' })), false)
  assert.equal(forgeSessionContinuityEnabled(env({ FORGE_SESSION_CONTINUITY: 'off' })), false)
  assert.equal(forgeSessionContinuityEnabled(env({ FORGE_SESSION_CONTINUITY: '1' })), true)
  assert.equal(SESSION_CONTINUITY_ENV, 'FORGE_SESSION_CONTINUITY')
})

test('V5-21 marker path is worktree-local so sessions cannot leak across worktrees', () => {
  assert.equal(forgeSessionMarkerPath('/w/a'), '/w/a/.forge-session.continue')
  assert.equal(forgeSessionMarkerPath('/w/b'), '/w/b/.forge-session.continue')
})
