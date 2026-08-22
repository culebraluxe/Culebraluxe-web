// ---------------------------------------------------------------------------
// ENG-21 — Isolated Worker Worktree Execution: operator CLI dispatch tests.
//
// runWorkspaceCliCore is exercised with INJECTED fake provisioner deps so the
// dispatch contract is proven deterministically — zero git, zero Neon:
//   - create maps 1:1 to provision and formats branch/worktree/base evidence
//   - status filters by story id and formats rows
//   - remove passes story/run through and reports the preserved branch
//   - missing arguments and unknown subcommands fail with usage (code 2)
// The real provisioner mechanics are covered by worker-workspace.test.ts.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  runWorkspaceCliCore,
  usage,
  type WorkspaceCliDeps,
} from '../../scripts/workspace-cli'
import { buildAgentInvokerWorkspaces } from '../../agent-runtime/invoker'
import type {
  WorkerWorkspace,
  WorkerWorkspaceListItem,
} from '../../lib/worker-workspace'

function fakeDeps(overrides: Partial<WorkspaceCliDeps> = {}): WorkspaceCliDeps {
  return {
    provision: async (spec) =>
      ({
        storyId: spec.storyId,
        workerId: spec.workerId,
        runId: spec.runId ?? 'derived',
        branchName: `agent/${spec.storyId}/${spec.runId ?? 'derived'}`,
        worktreePath: `/tmp/worktrees/${spec.storyId}-${spec.runId ?? 'derived'}`,
        baseRef: spec.baseRef,
        baseCommit: 'a'.repeat(40),
        sharedLinks: ['node_modules', '.env.local'],
      }) as WorkerWorkspace,
    list: async () =>
      [
        {
          branchName: 'agent/cmd-02/run-a',
          worktreePath: '/tmp/worktrees/cmd-02-run-a',
          head: 'b'.repeat(40),
          storyId: 'cmd-02',
          runId: 'run-a',
        },
        {
          branchName: 'agent/cmd-03/run-b',
          worktreePath: '/tmp/worktrees/cmd-03-run-b',
          head: 'c'.repeat(40),
          storyId: 'cmd-03',
          runId: 'run-b',
        },
      ] as WorkerWorkspaceListItem[],
    remove: async (opts) => ({
      removedPath: `/tmp/worktrees/${opts.storyId}-${opts.runId ?? 'x'}`,
      preservedBranch: `agent/${opts.storyId}/${opts.runId ?? 'x'}`,
    }),
    ...overrides,
  }
}

test('ENG-21 CLI: create passes the full spec and reports branch/worktree/base', async () => {
  const calls: unknown[] = []
  const deps = fakeDeps({
    provision: async (spec) => {
      calls.push(spec)
      return (await fakeDeps().provision(spec)) as WorkerWorkspace
    },
  })
  const out = await runWorkspaceCliCore(deps, [
    'create',
    'cmd-02',
    '--base',
    'main',
    '--worker',
    'lane-1',
    '--run',
    'run-a',
  ])
  assert.equal(out.code, 0)
  assert.match(out.text, /created isolated workspace for story cmd-02/)
  assert.match(out.text, /branch:\s+agent\/cmd-02\/run-a/)
  assert.match(out.text, /worktree:\s+\/tmp\/worktrees\/cmd-02-run-a/)
  assert.match(out.text, /base ref:\s+main -> [a-f0-9]{40}/)
  assert.deepEqual(calls[0], {
    storyId: 'cmd-02',
    workerId: 'lane-1',
    baseRef: 'main',
    runId: 'run-a',
  })
})

test('ENG-21 CLI: create defaults worker and requires a story id', async () => {
  const out = await runWorkspaceCliCore(fakeDeps(), ['create'])
  assert.equal(out.code, 2)
  assert.match(out.text, /usage: pnpm agent:workspace create/)

  const withDefaults = await runWorkspaceCliCore(fakeDeps(), ['create', 'st-1'])
  assert.equal(withDefaults.code, 0)
  assert.match(withDefaults.text, /agent\/st-1\/derived/)
})

test('ENG-21 CLI: status lists all workspaces and filters by story id', async () => {
  const all = await runWorkspaceCliCore(fakeDeps(), ['status'])
  assert.equal(all.code, 0)
  assert.match(all.text, /agent\/cmd-02\/run-a/)
  assert.match(all.text, /agent\/cmd-03\/run-b/)

  const filtered = await runWorkspaceCliCore(fakeDeps(), ['status', 'cmd-03'])
  assert.equal(filtered.code, 0)
  assert.match(filtered.text, /agent\/cmd-03\/run-b/)
  assert.doesNotMatch(filtered.text, /cmd-02/)

  const none = await runWorkspaceCliCore(fakeDeps(), ['status', 'nope'])
  assert.equal(none.code, 0)
  assert.match(none.text, /no agent workspaces for story nope/)
})

test('ENG-21 CLI: remove passes story/run through and reports the preserved branch', async () => {
  const calls: unknown[] = []
  const deps = fakeDeps({
    remove: async (opts) => {
      calls.push(opts)
      return {
        removedPath: `/tmp/worktrees/${opts.storyId}-${opts.runId ?? 'x'}`,
        preservedBranch: `agent/${opts.storyId}/${opts.runId ?? 'x'}`,
      }
    },
  })
  const out = await runWorkspaceCliCore(deps, ['remove', 'cmd-02', '--run', 'run-a'])
  assert.equal(out.code, 0)
  assert.match(out.text, /removed worktree \/tmp\/worktrees\/cmd-02-run-a/)
  assert.match(out.text, /branch agent\/cmd-02\/run-a preserved/)
  assert.deepEqual(calls[0], { storyId: 'cmd-02', runId: 'run-a' })

  const missing = await runWorkspaceCliCore(fakeDeps(), ['remove'])
  assert.equal(missing.code, 2)
  assert.match(missing.text, /usage: pnpm agent:workspace remove/)
})

test('ENG-21 CLI: unknown subcommand and help behave', async () => {
  const unknown = await runWorkspaceCliCore(fakeDeps(), ['frobnicate'])
  assert.equal(unknown.code, 2)
  assert.match(unknown.text, /unknown subcommand: frobnicate/)

  const help = await runWorkspaceCliCore(fakeDeps(), ['help'])
  assert.equal(help.code, 0)
  assert.ok(help.text.includes('pnpm agent:workspace'))

  const bare = await runWorkspaceCliCore(fakeDeps(), [])
  assert.equal(bare.code, 2)
  assert.equal(bare.text, usage())
})

test('ENG-21: buildAgentInvokerWorkspaces resolves the approved base fail-closed', () => {
  // Default: workspace execution enabled at the canonical main branch.
  const def = buildAgentInvokerWorkspaces('w1', {})
  assert.ok(def)
  assert.equal(def.workerId, 'w1')
  assert.equal(def.baseRef, 'main')
  assert.equal(typeof def.provision, 'function')

  // Explicit approved base override wins.
  const over = buildAgentInvokerWorkspaces('w1', {
    AGENT_WORKSPACE_BASE_REF: 'release/v1',
  })
  assert.equal(over?.baseRef, 'release/v1')

  // AGENT_WORKSPACE_WORKTREES_ROOT relocates the worktree directory.
  const rooted = buildAgentInvokerWorkspaces('w1', {
    AGENT_WORKSPACE_WORKTREES_ROOT: '/tmp/agent-worktrees',
  })
  assert.equal(rooted?.worktreesRoot, '/tmp/agent-worktrees')

  // AGENT_WORKSPACE_DISABLED=1 restores the legacy shared-checkout path
  // explicitly (documented escape hatch).
  assert.equal(
    buildAgentInvokerWorkspaces('w1', { AGENT_WORKSPACE_DISABLED: '1' }),
    undefined,
  )
  // Only the literal '1' disables; anything else keeps isolation on.
  assert.ok(buildAgentInvokerWorkspaces('w1', { AGENT_WORKSPACE_DISABLED: 'yes' }))
})
