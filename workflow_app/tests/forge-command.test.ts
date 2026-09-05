import assert from 'node:assert/strict'
import test from 'node:test'

import type { ApplicationPort } from '../../workflow_engine/lib/workflow/types'
import {
  FORGE_ROUTED_COMMAND_TYPES,
  FORGE_RUN_APPEND_DETAIL,
  FORGE_RUN_SMITH_SPLIT,
  FORGE_STORY_MARK_COMPLETE,
  FORGE_STORY_MARK_HOLD,
  FORGE_STORY_MARK_IN_PROGRESS,
  forgeCommandIsRouted,
} from '../forge-command-types'
import { createForgeApplicationPort } from '../forge/application-port'
import { buildForgeCommandRegistry, dispatchForgeCommand } from '../forge/forge-command'
import type { ForgeStateWriter } from '../forge/forge-state-writer'
import type { ForgeReleaseExecutor } from '../forge/forge-state-writer'

// ---------------------------------------------------------------------------
// ENG-FORGE-V9 Stage 3 — Forge command domain (the "B" fork), DB-free.
//
// Proves the Forge-owned dispatcher + handlers route forge.* commands through
// an injected writer, and that the Forge ApplicationPort is a hard A/B
// boundary: it executes forge.* commands and NEVER an RE command. No database —
// a fake writer records calls.
// ---------------------------------------------------------------------------

function fakeWriter(): { writer: ForgeStateWriter; calls: string[] } {
  const calls: string[] = []
  const writer: ForgeStateWriter = {
    async markStoryInProgress(id) {
      calls.push(`in_progress:${id}`)
    },
    async markStoryHumanHold(id, reason) {
      calls.push(`hold:${id}:${reason}`)
    },
    async markStoryComplete(id) {
      calls.push(`complete:${id}`)
    },
    async appendRunDetail(runId, detail) {
      calls.push(`detail:${runId}:${detail}`)
    },
  }
  return { writer, calls }
}

function envelope(commandType: string, input: Record<string, unknown>) {
  return { commandId: 'cmd-1', commandType, input }
}

const fakeReleaseExecutor: ForgeReleaseExecutor = {
  async execute(input) {
    return { commandType: input.commandType, outcome: 'success' }
  },
}

test('ENG-FORGE-V9: the Forge registry covers every routed forge.* command', () => {
  const { writer } = fakeWriter()
  const registry = buildForgeCommandRegistry(writer, fakeReleaseExecutor)
  const registered = new Set(registry.list())
  for (const type of FORGE_ROUTED_COMMAND_TYPES) {
    assert.ok(registered.has(type), `registry must route ${type}`)
  }
  assert.equal(registered.size, FORGE_ROUTED_COMMAND_TYPES.size)
})

test('ENG-FORGE-V10: a legacy role command cannot simulate successful execution', async () => {
  const { writer } = fakeWriter()
  const registry = buildForgeCommandRegistry(writer)
  const result = await dispatchForgeCommand(
    envelope(FORGE_RUN_SMITH_SPLIT, { storyId: 'S-1', runId: 'R-9' }),
    registry,
  )
  assert.equal(result.outcome, 'precondition_failure')
  assert.match(result.message ?? '', /claimed Forge engine task/)
})

test('ENG-FORGE-V10: release-critical command fails closed without a real executor', async () => {
  const { writer } = fakeWriter()
  const registry = buildForgeCommandRegistry(writer)
  const result = await dispatchForgeCommand(
    envelope('forge.publish_candidate', { storyId: 'S-1' }),
    registry,
  )
  assert.equal(result.outcome, 'precondition_failure')
})

test('ENG-FORGE-V9: forge.story.hold routes to the writer with storyId + reason', async () => {
  const { writer, calls } = fakeWriter()
  const registry = buildForgeCommandRegistry(writer)
  const result = await dispatchForgeCommand(
    envelope(FORGE_STORY_MARK_HOLD, { storyId: 'S-1', reason: 'veto' }),
    registry,
  )
  assert.equal(result.outcome, 'success')
  assert.deepEqual(calls, ['hold:S-1:veto'])
})

test('ENG-FORGE-V9: forge.story.complete / in_progress / run.detail route to the writer', async () => {
  const { writer, calls } = fakeWriter()
  const registry = buildForgeCommandRegistry(writer)
  assert.equal(
    (await dispatchForgeCommand(envelope(FORGE_STORY_MARK_COMPLETE, { storyId: 'S-2' }), registry))
      .outcome,
    'success',
  )
  assert.equal(
    (
      await dispatchForgeCommand(envelope(FORGE_STORY_MARK_IN_PROGRESS, { storyId: 'S-2' }), registry)
    ).outcome,
    'success',
  )
  assert.equal(
    (
      await dispatchForgeCommand(
        envelope(FORGE_RUN_APPEND_DETAIL, { runId: 'R-9', detail: 'assayed' }),
        registry,
      )
    ).outcome,
    'success',
  )
  assert.deepEqual(calls, ['complete:S-2', 'in_progress:S-2', 'detail:R-9:assayed'])
})

test('ENG-FORGE-V9: missing required input is a validation_failure, never a silent write', async () => {
  const { writer, calls } = fakeWriter()
  const registry = buildForgeCommandRegistry(writer)
  const result = await dispatchForgeCommand(envelope(FORGE_STORY_MARK_HOLD, {}), registry)
  assert.equal(result.outcome, 'validation_failure')
  assert.deepEqual(calls, [], 'no writer call on malformed input')
})

test('ENG-FORGE-V9: an RE command is not routed by the Forge dispatcher (A/B separation)', async () => {
  const { writer, calls } = fakeWriter()
  const registry = buildForgeCommandRegistry(writer)
  const result = await dispatchForgeCommand(envelope('deal.set_stage_closed', {}), registry)
  assert.equal(result.outcome, 'not_found')
  assert.deepEqual(calls, [])
})

test('ENG-FORGE-V9: Forge inventory routes forge.* and never RE commands', () => {
  assert.equal(forgeCommandIsRouted(FORGE_STORY_MARK_HOLD), true)
  assert.equal(forgeCommandIsRouted('deal.set_stage_closed'), false)
})

test('ENG-FORGE-V9: the Forge ApplicationPort executes forge.* and refuses RE commands', async () => {
  const { writer, calls } = fakeWriter()
  const port: ApplicationPort = await createForgeApplicationPort({ writer })

  const hold = await port.executeCommand({
    commandId: 'cmd-2',
    commandType: FORGE_STORY_MARK_HOLD,
    subjectType: 'story',
    subjectId: 'S-1',
    correlationId: 'c-1',
    causationId: null,
    input: { storyId: 'S-1', reason: 'veto' },
  })
  assert.equal(hold.outcome, 'success')

  const re = await port.executeCommand({
    commandId: 'cmd-3',
    commandType: 'deal.set_stage_closed',
    subjectType: 'deal',
    subjectId: 'D-1',
    correlationId: 'c-2',
    causationId: null,
    input: {},
  })
  assert.equal(re.outcome, 'not_found')
  assert.deepEqual(calls, ['hold:S-1:veto'], 'only the Forge command reached the writer')
})

test('ENG-FORGE-V9: Forge ApplicationPort readFacts returns {} without a reader', async () => {
  const { writer } = fakeWriter()
  const port: ApplicationPort = await createForgeApplicationPort({ writer })
  const facts = await port.readFacts({ subjectType: 'story', subjectId: 'S-1' })
  assert.deepEqual(facts, {})
})
