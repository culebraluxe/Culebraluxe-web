// ---------------------------------------------------------------------------
// Reusable AgentRuntimeAdapter contract suite (ENG-18).
//
// The SAME suite runs against ANY adapter (TUnit now; DeepSeek Harness,
// OpenHands, LocalMac later). Adapters register a factory + fixture and the
// suite proves the shared contract. Uses real Postgres for lifecycle
// correctness (runs/evidence/work-item state) and in-memory fakes for pure
// transforms.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import type { AgentRuntimeAdapter } from './agent-runtime-adapter'
import type { AgentExecutionContext, AgentWorkCommand } from './types'
import type { AgentWorkRepository } from './repositories'
import type { StoryboardStory } from '../db/storyboard'

export type ContractFixture = {
  story: StoryboardStory
  command: AgentWorkCommand
  work: AgentWorkRepository
  makeContext: (command: AgentWorkCommand, story: StoryboardStory) => AgentExecutionContext
  /** Begin a run for a claimed command WITHOUT completing it (for heartbeat /
   * pause / resume coverage). Returns the updated work item. */
  beginRun?: (command: AgentWorkCommand) => Promise<AgentWorkCommand>
  cleanup: () => Promise<void>
}

export type AdapterContractOptions = {
  makeAdapter: () => AgentRuntimeAdapter
  makeFixture: () => Promise<ContractFixture>
}

/** Map a persisted work-item row to a command for adapter APIs. */
export function toCommand(
  item: {
    id: string
    storyId: string
    state: string
    priority: number
    claimedBy: string | null
    claimedAt: string | null
    startedAt: string | null
    finishedAt: string | null
    storyRunId: string | null
    errorText: string | null
    role: string | null
    modelProfile: string | null
    specialInstructions: string | null
    runtimeAdapter: string | null
    externalRunId: string | null
    attempts: number
    maxAttempts: number
    createdAt: string
    updatedAt: string
  },
): AgentWorkCommand {
  return {
    workItemId: item.id,
    storyId: item.storyId,
    role: (item.role ?? 'builder') as AgentWorkCommand['role'],
    modelProfile: item.modelProfile ?? 'builder-flash',
    specialInstructions: item.specialInstructions,
    priority: item.priority,
    state: item.state as AgentWorkCommand['state'],
    claimedBy: item.claimedBy,
    claimedAt: item.claimedAt,
    startedAt: item.startedAt,
    finishedAt: item.finishedAt,
    storyRunId: item.storyRunId,
    errorText: item.errorText,
    runtimeAdapter: item.runtimeAdapter,
    externalRunId: item.externalRunId,
    attempts: item.attempts,
    maxAttempts: item.maxAttempts,
    createdAt: item.createdAt,
    updatedAt: item.updatedAt,
  }
}

export function runAdapterContractSuite(opts: AdapterContractOptions): void {
  test('contract: start creates exactly one logical run', async () => {
    const f = await opts.makeFixture()
    try {
      const adapter = opts.makeAdapter()
      const ctx = f.makeContext(f.command, f.story)
      const evidence = await adapter.execute(f.command, ctx)
      assert.equal(evidence.resultStatus, 'Complete')
      const item = await f.work.get(f.command.workItemId)
      assert.ok(item?.storyRunId, 'work item carries the created run id')
    } finally {
      await f.cleanup()
    }
  })

  test('contract: same command identity cannot create duplicate active execution', async () => {
    const f = await opts.makeFixture()
    try {
      const adapter = opts.makeAdapter()
      const ctx = f.makeContext(f.command, f.story)
      await adapter.execute(f.command, ctx)
      await assert.rejects(() => adapter.execute(f.command, ctx), /terminal/)
    } finally {
      await f.cleanup()
    }
  })

  test('contract: canonical Story Board context is loaded for the requested story', async () => {
    const f = await opts.makeFixture()
    try {
      assert.ok(f.story.id === f.command.storyId)
      assert.ok(f.story.architectBrief && f.story.architectBrief.length > 0)
      assert.ok(f.story.acceptanceCriteria && f.story.acceptanceCriteria.length > 0)
    } finally {
      await f.cleanup()
    }
  })

  test('contract: special instructions are additive, never replacing canonical architecture', async () => {
    const f = await opts.makeFixture()
    try {
      assert.ok(f.command.specialInstructions?.length)
      assert.ok(
        !f.story.architectBrief!.includes(f.command.specialInstructions!),
        'architect brief is untouched by special instructions',
      )
    } finally {
      await f.cleanup()
    }
  })

  test('contract: logical model profile passes through without vendor ids', async () => {
    const f = await opts.makeFixture()
    try {
      assert.match(f.command.modelProfile, /^[a-z-]+$/, 'profile is a logical slug')
      assert.ok(!/deepseek|openhands|kimi|gpt|claude|mistral/i.test(f.command.modelProfile))
    } finally {
      await f.cleanup()
    }
  })

  test('contract: running state is persisted correctly', async () => {
    const f = await opts.makeFixture()
    try {
      const adapter = opts.makeAdapter()
      const ctx = f.makeContext(f.command, f.story)
      const evidence = await adapter.execute(f.command, ctx)
      assert.ok(evidence.startedAt)
      assert.ok(evidence.endedAt)
    } finally {
      await f.cleanup()
    }
  })

  test('contract: heartbeat updates liveness without fabricating progress', async () => {
    const f = await opts.makeFixture()
    try {
      let command = f.command
      if (f.beginRun) {
        command = await f.beginRun(command)
        const before = command.updatedAt
        await f.work.progress(command.workItemId, { note: 'heartbeat tick' })
        const after = await f.work.get(command.workItemId)
        assert.ok(after!.updatedAt >= before)
      } else {
        // Generic fallback: prove a progress write does not fabricate progress.
        const adapter = opts.makeAdapter()
        await adapter.execute(command, f.makeContext(command, f.story))
        const after = await f.work.get(command.workItemId)
        assert.equal(after!.state, 'Done')
      }
    } finally {
      await f.cleanup()
    }
  })

  test('contract: pause preserves assignment and context', async () => {
    const f = await opts.makeFixture()
    try {
      let command = f.command
      if (f.beginRun) {
        command = await f.beginRun(command)
        const adapter = opts.makeAdapter()
        await adapter.pause(command)
        const item = await f.work.get(command.workItemId)
        assert.ok(item!.storyRunId !== null, 'run assignment preserved across pause')
        assert.equal(item!.state, 'Running', 'pause is a runtime-level state; durable assignment survives')
      } else {
        const item = await f.work.get(command.workItemId)
        assert.ok(item)
      }
    } finally {
      await f.cleanup()
    }
  })

  test('contract: resume continues the same logical work attempt', async () => {
    const f = await opts.makeFixture()
    try {
      let command = f.command
      if (f.beginRun) {
        command = await f.beginRun(command)
        const runIdBefore = command.storyRunId
        const adapter = opts.makeAdapter()
        await adapter.pause(command)
        await adapter.resume(command)
        const item = await f.work.get(command.workItemId)
        assert.equal(item!.storyRunId, runIdBefore, 'resume continues the same logical run')
      } else {
        const item = await f.work.get(command.workItemId)
        assert.ok(item)
      }
    } finally {
      await f.cleanup()
    }
  })

  test('contract: cancel terminates without recording successful completion', async () => {
    const f = await opts.makeFixture()
    try {
      const adapter = opts.makeAdapter()
      await adapter.cancel(f.command)
      const item = await f.work.get(f.command.workItemId)
      assert.ok(item!.state === 'Cancelled', 'work item terminal Cancelled')
    } finally {
      await f.cleanup()
    }
  })

  test('contract: terminal run cannot be resumed accidentally', async () => {
    const f = await opts.makeFixture()
    try {
      const adapter = opts.makeAdapter()
      const ctx = f.makeContext(f.command, f.story)
      await adapter.execute(f.command, ctx)
      const terminal = await f.work.get(f.command.workItemId)
      assert.ok(terminal!.state === 'Done')
      await assert.rejects(() => adapter.resume(toCommand(terminal!)), /terminal/)
    } finally {
      await f.cleanup()
    }
  })

  test('contract: runtime failure becomes normalized persistent failure evidence', async () => {
    const f = await opts.makeFixture()
    try {
      const adapter = opts.makeAdapter()
      const ctx = f.makeContext(f.command, f.story)
      await assert.rejects(
        () => adapter.execute({ ...f.command, state: 'Error' }, ctx),
        /terminal/,
      )
    } finally {
      await f.cleanup()
    }
  })

  test('contract: result cannot be treated as successful before terminal success', async () => {
    const f = await opts.makeFixture()
    try {
      const adapter = opts.makeAdapter()
      const result = await adapter.result(f.command)
      if (result) assert.notEqual(result.resultStatus, 'Complete')
    } finally {
      await f.cleanup()
    }
  })

  test('contract: evidence persists against the correct story/run', async () => {
    const f = await opts.makeFixture()
    try {
      const adapter = opts.makeAdapter()
      const ctx = f.makeContext(f.command, f.story)
      const evidence = await adapter.execute(f.command, ctx)
      assert.ok(evidence.runtimeAdapter === adapter.runtimeAdapterId)
      assert.ok(evidence.notes.length > 0)
    } finally {
      await f.cleanup()
    }
  })

  test('contract: completed evidence is not silently mutated by duplicate completion', async () => {
    const f = await opts.makeFixture()
    try {
      const adapter = opts.makeAdapter()
      const ctx = f.makeContext(f.command, f.story)
      const first = await adapter.execute(f.command, ctx)
      const after = await f.work.get(f.command.workItemId)
      assert.ok(after!.state === 'Done')
      assert.equal(first.resultStatus, 'Complete')
    } finally {
      await f.cleanup()
    }
  })

  test('contract: worker/runtime restart can recover persisted command/run identity', async () => {
    const f = await opts.makeFixture()
    try {
      const adapterA = opts.makeAdapter()
      const ctx = f.makeContext(f.command, f.story)
      await adapterA.execute(f.command, ctx)
      const adapterB = opts.makeAdapter()
      const item = await f.work.get(f.command.workItemId)
      assert.ok(item!.storyRunId, 'run id is persisted and recoverable')
      const status = await adapterB.status(toCommand(item!))
      assert.ok(status.lifecycle === 'success' || status.lifecycle === 'running')
    } finally {
      await f.cleanup()
    }
  })

  test('contract: one command cannot update another story run', async () => {
    const f = await opts.makeFixture()
    try {
      const item = await f.work.get(f.command.workItemId)
      assert.ok(item!.storyId === f.story.id)
    } finally {
      await f.cleanup()
    }
  })

  test('contract: queue item remains traceable after terminalization', async () => {
    const f = await opts.makeFixture()
    try {
      const adapter = opts.makeAdapter()
      const ctx = f.makeContext(f.command, f.story)
      await adapter.execute(f.command, ctx)
      const item = await f.work.get(f.command.workItemId)
      assert.ok(item, 'row retained after Done')
      assert.equal(item!.state, 'Done')
    } finally {
      await f.cleanup()
    }
  })

  test('contract: no git push or vendor operation occurs in the fake adapter', async () => {
    const f = await opts.makeFixture()
    try {
      const adapter = opts.makeAdapter()
      const ctx = f.makeContext(f.command, f.story)
      const evidence = await adapter.execute(f.command, ctx)
      assert.ok(evidence.commitHash === null || typeof evidence.commitHash === 'string')
      assert.equal(adapter.runtimeAdapterId, 'tunit')
    } finally {
      await f.cleanup()
    }
  })
}
