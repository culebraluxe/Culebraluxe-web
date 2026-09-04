// ---------------------------------------------------------------------------
// OpenCode Smith routing + Forge-owned candidate commit tests.
//
// V6 keeps Smith on OpenCode by default but moves verifier-mini permanently to
// the model-free forge-assay adapter. Builder provider overrides never reroute
// Assay.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { createAgentRuntimeRegistry, forgeTeamWithBuilderFlashOverride, parseBuilderFlashOverride, sharedAgentRuntimeRegistry, resetSharedAgentRuntimeRegistry, resolveDeepSeekWorkspace } from '../factory'
import { buildDeepSeekModelPatch, resolveDeepSeekModel } from '../deepseek/deepseek-harness-adapter'
import { DEFAULT_FORGE_TEAM } from '../team'
import { OPENCODE_PINNED_MODEL } from './opencode-harness-adapter'
import type { AgentRuntimeRegistry } from '../registry'
import type {
  AgentExecutionContext,
  AgentExecutionWorkspace,
  AgentWorkCommand,
} from '../types'
import type { StoryboardStory } from '../../db/storyboard'
import type { OpenCodeHandle } from './opencode-client'

const NOOP_START_RUN = (() => {
  throw new Error('startRun must not be reached in this test')
}) as never

function openCodeReadyConfig(): { cliBin: string; workspace: string; model: string; startRun: typeof NOOP_START_RUN } {
  return {
    cliBin: '/fake/opencode',
    workspace: process.cwd(),
    model: OPENCODE_PINNED_MODEL,
    startRun: NOOP_START_RUN,
  }
}

function deepSeekReadyConfig() {
  return {
    cliBin: '/fake/dsh',
    workspace: process.cwd(),
    startRun: NOOP_START_RUN,
  }
}

function git(cwd: string, args: string[]): string {
  return execFileSync('git', args, { cwd, encoding: 'utf8' }).trim()
}

function withProfileEnv(value: string | undefined, fn: () => void): void {
  const before = process.env.FORGE_PROVIDER_BUILDER_FLASH
  try {
    if (value === undefined) delete process.env.FORGE_PROVIDER_BUILDER_FLASH
    else process.env.FORGE_PROVIDER_BUILDER_FLASH = value
    fn()
  } finally {
    if (before === undefined) delete process.env.FORGE_PROVIDER_BUILDER_FLASH
    else process.env.FORGE_PROVIDER_BUILDER_FLASH = before
  }
}

test('default Smith is OpenCode while V6 Assay is deterministic Forge runtime', () => {
  withProfileEnv(undefined, () => {
    const registry: AgentRuntimeRegistry = createAgentRuntimeRegistry(
      deepSeekReadyConfig(),
      openCodeReadyConfig(),
    )
    assert.ok(registry.listAdapters().includes('opencode-harness'))
    assert.ok(registry.listAdapters().includes('forge-assay'))
    const expected: Record<string, string> = {
      'scout-volume': 'deepseek-harness:deepseek/deepseek-v4-flash',
      'architect-pro': 'deepseek-harness:deepseek/deepseek-chat',
      'builder-flash': 'opencode-harness',
      'verifier-mini': 'forge-assay',
    }
    for (const [profile, adapterId] of Object.entries(expected)) {
      assert.equal(registry.resolveProfile(profile).adapterId, adapterId)
    }
    const smith = registry.resolveAdapter('builder-flash', {
      work: {} as never,
      runs: {} as never,
    })
    assert.equal(smith.runtimeAdapterId, 'opencode-harness')
    const assay = registry.resolveAdapter('verifier-mini', {
      work: {} as never,
      runs: {} as never,
    })
    assert.equal(assay.runtimeAdapterId, 'forge-assay')
  })
})

test('explicit Smith OpenCode routing does not reroute Assay', () => {
  withProfileEnv('opencode', () => {
    const registry: AgentRuntimeRegistry = createAgentRuntimeRegistry(
      deepSeekReadyConfig(),
      openCodeReadyConfig(),
    )
    assert.equal(registry.resolveProfile('builder-flash').adapterId, 'opencode-harness')
    assert.equal(
      registry.inspectProfileReadiness('builder-flash').ready,
      true,
      'ready when the CLI/config are qualified',
    )
    assert.equal(registry.resolveProfile('scout-volume').adapterId, 'deepseek-harness:deepseek/deepseek-v4-flash')
    assert.equal(registry.resolveProfile('architect-pro').adapterId, 'deepseek-harness:deepseek/deepseek-chat')
    assert.equal(registry.resolveProfile('verifier-mini').adapterId, 'forge-assay')

    const smith = registry.resolveAdapter('builder-flash', {
      work: {} as never,
      runs: {} as never,
    })
    assert.equal(smith.runtimeAdapterId, 'opencode-harness')
  })
})

test('explicit deepseek override returns Smith to forge-native DeepSeek but leaves Assay deterministic', () => {
  withProfileEnv('deepseek', () => {
    const registry: AgentRuntimeRegistry = createAgentRuntimeRegistry(
      deepSeekReadyConfig(),
      openCodeReadyConfig(),
    )
    // Routing truth is the pinned registry id; the class-level
    // runtimeAdapterId stays 'deepseek-harness' for evidence compat.
    assert.equal(registry.resolveProfile('builder-flash').adapterId, 'deepseek-harness:deepseek/deepseek-v4-flash')
    assert.equal(registry.resolveProfile('verifier-mini').adapterId, 'forge-assay')
    assert.equal(registry.inspectProfileReadiness('builder-flash').ready, true)
    const smith = registry.resolveAdapter('builder-flash', {
      work: {} as never,
      runs: {} as never,
    })
    assert.equal(smith.runtimeAdapterId, 'deepseek-harness')
  })
})

test('default OpenCode routing but the CLI is missing fails closed (never forge-native fallback)', () => {
  withProfileEnv(undefined, () => {
    const registry = createAgentRuntimeRegistry(deepSeekReadyConfig(), {
      cliBin: '/definitely/not/installed/opencode',
      workspace: process.cwd(),
      model: OPENCODE_PINNED_MODEL,
    })
    const readiness = registry.inspectProfileReadiness('builder-flash')
    assert.equal(registry.resolveProfile('builder-flash').adapterId, 'opencode-harness')
    assert.equal(readiness.registered, true)
    assert.equal(readiness.installed, false)
    assert.equal(readiness.ready, false)
    assert.match(readiness.reason, /OpenCode CLI entrypoint not found/)
    assert.match(readiness.reason, /no silent fallback/)
  })
})

test('OpenCode selected but the explicit model config is blank fails closed on the pin', () => {
  withProfileEnv('opencode', () => {
    const registry = createAgentRuntimeRegistry(deepSeekReadyConfig(), {
      cliBin: '/fake/opencode',
      workspace: process.cwd(),
      model: '   ',
      startRun: NOOP_START_RUN,
    })
    const readiness = registry.inspectProfileReadiness('builder-flash')
    assert.equal(readiness.ready, false)
    assert.match(readiness.reason, /no explicit model configuration/)
    assert.match(readiness.reason, /deepseek\/deepseek-v4-flash/)
    assert.equal(registry.resolveProfile('builder-flash').adapterId, 'opencode-harness')
  })
})

test('OpenCode selected with a non-pinned model fails closed', () => {
  withProfileEnv('opencode', () => {
    const registry = createAgentRuntimeRegistry(deepSeekReadyConfig(), {
      cliBin: '/fake/opencode',
      workspace: process.cwd(),
      model: 'anthropic/claude-opus',
      startRun: NOOP_START_RUN,
    })
    const readiness = registry.inspectProfileReadiness('builder-flash')
    assert.equal(readiness.ready, false)
    assert.match(readiness.reason, /not the ENG-FORGE-V5-01 pinned model/)
  })
})

test('factory finding #1: explicit override matrix routes builder-flash without env reads mid-loop', () => {
  const matrix: Array<{ override: 'deepseek' | 'forge-native' | 'opencode' | 'openclaw' | 'warp' | 'warp-agent'; adapterId: string }> = [
    { override: 'deepseek', adapterId: 'deepseek-harness:deepseek/deepseek-v4-flash' },
    { override: 'forge-native', adapterId: 'deepseek-harness:deepseek/deepseek-v4-flash' },
    { override: 'opencode', adapterId: 'opencode-harness' },
    { override: 'openclaw', adapterId: 'gateway-openclaw' },
    { override: 'warp', adapterId: 'gateway-warp' },
    { override: 'warp-agent', adapterId: 'gateway-warp' },
  ]
  for (const { override, adapterId } of matrix) {
    const registry = createAgentRuntimeRegistry({
      deepseek: deepSeekReadyConfig(),
      opencode: openCodeReadyConfig(),
      builderFlashOverride: override,
    })
    assert.equal(registry.resolveProfile('builder-flash').adapterId, adapterId, override)
  }
  assert.equal(parseBuilderFlashOverride(null), null)
  assert.equal(parseBuilderFlashOverride('  '), null)
  assert.equal(parseBuilderFlashOverride('OpenCode'), 'opencode')
  assert.throws(() => parseBuilderFlashOverride('mystery'), /unknown FORGE_PROVIDER_BUILDER_FLASH/)
})

test('factory finding #1: team override applies to every builder-flash variant including grades', () => {
  const overridden = forgeTeamWithBuilderFlashOverride(DEFAULT_FORGE_TEAM, 'deepseek')
  assert.equal(overridden.assignments.smith.harnessId, 'forge-native')
  assert.equal(overridden.assignments.smith.playerId, 'deepseek-flash')
  assert.equal(DEFAULT_FORGE_TEAM.assignments.smith.harnessId, 'opencode', 'base team is not mutated')
})

test('factory finding #3: Pi mapping registers a blocked placeholder instead of crashing hydration', () => {
  const team = {
    ...DEFAULT_FORGE_TEAM,
    assignments: {
      ...DEFAULT_FORGE_TEAM.assignments,
      archive: {
        ...DEFAULT_FORGE_TEAM.assignments.archive,
        profile: 'archivist-pi',
        harnessId: 'pi' as const,
      },
    },
  }
  const registry = createAgentRuntimeRegistry({
    deepseek: deepSeekReadyConfig(),
    opencode: openCodeReadyConfig(),
    team,
  })
  assert.ok(registry.hasProfile('builder-flash'))
  assert.ok(registry.hasProfile('archivist-pi'))
  const readiness = registry.inspectProfileReadiness('archivist-pi')
  assert.equal(readiness.ready, false)
  assert.match(readiness.reason, /no Pi runtime adapter is configured/)
})

test('factory finding #7: explicit DeepSeek workspace wins over CWD-baked default', () => {
  assert.equal(resolveDeepSeekWorkspace('/tmp/worktree'), '/tmp/worktree')
  assert.equal(resolveDeepSeekWorkspace('  '), resolveDeepSeekWorkspace(null))
})

test('factory finding #8: shared registry memoizes per process', () => {
  resetSharedAgentRuntimeRegistry()
  try {
    const first = sharedAgentRuntimeRegistry()
    const second = sharedAgentRuntimeRegistry()
    assert.equal(first, second)
    assert.ok(first.hasProfile('builder-flash'))
  } finally {
    resetSharedAgentRuntimeRegistry()
  }
})

test('factory finding #6: sharing a profile across lanes with different capabilities fails closed', () => {
  // architect requires [storyboard.read, config.read]; archive shares the
  // architect-pro profile today. Point archive at the assay lane's narrower
  // ASSAY set via the same profile name: same player/harness route would
  // silently widen/narrow privilege, so the factory refuses.
  const team = {
    ...DEFAULT_FORGE_TEAM,
    assignments: {
      ...DEFAULT_FORGE_TEAM.assignments,
      assay: {
        ...DEFAULT_FORGE_TEAM.assignments.assay,
        profile: 'architect-pro',
        playerId: 'deepseek-pro' as const,
        harnessId: 'forge-native' as const,
      },
    },
  }
  assert.throws(
    () =>
      createAgentRuntimeRegistry({
        deepseek: deepSeekReadyConfig(),
        opencode: openCodeReadyConfig(),
        team,
      }),
    /different required capabilities/,
  )
})

test('spend vision: forge-native profiles route to model-pinned adapters', () => {
  const registry = createAgentRuntimeRegistry({
    deepseek: deepSeekReadyConfig(),
    opencode: openCodeReadyConfig(),
  })
  // scout-volume (flash) and architect-pro (pro) share the forge-native
  // harness but must NOT share an adapter: each gets its exact model.
  const scout = registry.resolveProfile('scout-volume')
  const architect = registry.resolveProfile('architect-pro')
  assert.notEqual(scout.adapterId, architect.adapterId)
  assert.equal(scout.adapterId, 'deepseek-harness:deepseek/deepseek-v4-flash')
  assert.equal(architect.adapterId, 'deepseek-harness:deepseek/deepseek-chat')
  assert.ok(registry.listAdapters().includes('deepseek-harness:deepseek/deepseek-v4-flash'))
  assert.ok(registry.listAdapters().includes('deepseek-harness:deepseek/deepseek-chat'))
})

test('spend vision: dsh model patch pins the exact provider/model', () => {
  const patch = buildDeepSeekModelPatch('deepseek/deepseek-chat')
  assert.match(patch, /id: agent-default-model/)
  assert.match(patch, /provider: deepseek/)
  assert.match(patch, /model: deepseek-chat/)
  assert.equal(resolveDeepSeekModel('deepseek/deepseek-chat'), 'deepseek/deepseek-chat')
  assert.equal(resolveDeepSeekModel(null), null)
  assert.throws(() => resolveDeepSeekModel('  '), /no explicit model configuration/)
  assert.throws(() => resolveDeepSeekModel('vague'), /exact 'provider\/model' id/)
})

function makeWorkspace(): { workspace: AgentExecutionWorkspace; cleanup: () => void } {
  const cwd = mkdtempSync(join(tmpdir(), 'forge-opencode-policy-'))
  git(cwd, ['init', '-b', 'main', '-q'])
  git(cwd, ['config', 'user.email', 'opencode-policy@example.com'])
  git(cwd, ['config', 'user.name', 'OpenCode Policy Test'])
  writeFileSync(join(cwd, 'base.txt'), 'base\n')
  git(cwd, ['add', '.'])
  git(cwd, ['commit', '-m', 'base', '-q'])
  const baseCommit = git(cwd, ['rev-parse', 'HEAD'])
  return {
    workspace: {
      branchName: 'agent/eng-forge-v5-01/policy-test',
      worktreePath: cwd,
      baseRef: 'main',
      baseCommit,
      runId: 'policy-test',
    },
    cleanup: () => rmSync(cwd, { recursive: true, force: true }),
  }
}

function command(): AgentWorkCommand {
  return {
    workItemId: 'wi-opencode-policy',
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
  }
}

function story(): StoryboardStory {
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
    scope: null,
    dependencies: null,
    preconditions: null,
    architectBrief: 'OpenCode is an inner execution engine.',
    contextRefs: null,
    acceptanceCriteria: 'Focused adapter.',
    postconditions: null,
    architectBriefUpdatedAt: null,
    completion: 0,
    rollup: true,
    plannedStartAt: null,
    actualStartAt: null,
    completedAt: null,
    createdAt: '2026-09-03T00:00:00.000Z',
    updatedAt: '2026-09-03T00:00:00.000Z',
  }
}

function context(workspace: AgentExecutionWorkspace): AgentExecutionContext {
  const cmd = command()
  return {
    command: cmd,
    story: story(),
    policy: { allowCommit: true, allowDevDbWrite: true, allowControlPlaneWrite: true },
    capabilities: [],
    executionEnvironment: 'DEV',
    executionWorkspace: workspace,
    storyRunId: '',
  }
}

test('Forge creates the candidate commit when OpenCode leaves the worktree dirty (default routing)', async () => {
  const { workspace, cleanup } = makeWorkspace()
  try {
    const handle: OpenCodeHandle = {
      proc: { exitCode: 0, killed: false } as never,
      done: true,
      promise: Promise.resolve({
        status: 'success',
        exitCode: 0,
        stdout: 'OpenCode edited files but did not commit.',
        stderr: '',
      }),
      cancelled: false,
      pause: () => undefined,
      resume: () => undefined,
      cancel: () => undefined,
    }
    let adapter: any
    withProfileEnv(undefined, () => {
      const registry = createAgentRuntimeRegistry(deepSeekReadyConfig(), {
        cliBin: 'opencode',
        workspace: process.cwd(),
        model: OPENCODE_PINNED_MODEL,
        startRun: () => handle,
      })
      adapter = registry.resolveAdapter('builder-flash', {
        work: {} as never,
        runs: {} as never,
      })
    })
    assert.equal(adapter.runtimeAdapterId, 'opencode-harness')

    writeFileSync(join(workspace.worktreePath, 'smith.txt'), 'candidate\n')

    const ctx = context(workspace)
    await adapter.startExternal(ctx)
    const evidence = await adapter.resultExternal(ctx.command, ctx)

    assert.equal(evidence.resultStatus, 'Complete')
    assert.ok(evidence.commitHash, 'Forge created the candidate commit')
    assert.notEqual(evidence.commitHash, workspace.baseCommit)
    assert.match(
      evidence.notes,
      /Forge harness created candidate commit [0-9a-f]{40} from OpenCode's worker changes\./,
    )
    assert.equal(
      git(workspace.worktreePath, ['log', '-1', '--format=%H']),
      evidence.commitHash,
    )
    assert.equal(git(workspace.worktreePath, ['status', '--porcelain']), '')
  } finally {
    cleanup()
  }
})
