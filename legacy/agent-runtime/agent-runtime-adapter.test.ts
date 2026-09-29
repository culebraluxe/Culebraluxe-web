// ---------------------------------------------------------------------------
// The base adapter's terminal paths must RECORD a failure, never crash on it.
//
// Measured live on 2026-09-16: the architect lane died, and the only cause that
// reached `forge_engine_task_execution.last_error` was
// `Cannot read properties of null (reading 'notes')` — a TypeError thrown by
// `execute()` itself, on `result!.notes`, because the vendor hook answered
// `null` after reporting success. The recorder's OWN crash became the recorded
// failure, so the real one was unreadable from the rows.
//
// Two obligations are pinned here:
//   1. a successful status with no evidence is a CONTRADICTION: the run is
//      terminalized as Failed against a NAMED reason, and that reason is what
//      the ledger can read back;
//   2. no terminal path may dereference a null hook answer or a missing run
//      link, so a failure is never replaced by a property-access TypeError.
//
// No Neon, no vendor process: the hooks are the fakes, the shared lifecycle under
// test is the real production code.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  AgentRuntimeAdapter,
  type ExternalStartResult,
  type ExternalStatusResult,
} from './agent-runtime-adapter'
import type { AgentCapability } from './capabilities'
import type {
  AgentExecutionContext,
  AgentRunEvidence,
  AgentWorkCommand,
} from './types'

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

type Item = {
  id: string
  storyId: string
  state: string
  storyRunId: string | null
  errorText: string | null
  role: string | null
  modelProfile: string | null
  priority: number
  claimedBy: string | null
  attempts: number
  maxAttempts: number
}

const RUN_ID = 'run-00000000-0000-4000-8000-000000000001'
const WORK_ITEM_ID = 'wi-base-adapter-1'

function command(overrides: Partial<AgentWorkCommand> = {}): AgentWorkCommand {
  return {
    workItemId: WORK_ITEM_ID,
    storyId: 'ENG-QA-SINGLE-VERDICT-01',
    role: 'architect',
    modelProfile: 'architect-pro',
    specialInstructions: null,
    priority: 50,
    state: 'Claimed',
    claimedBy: 'stub-adapter',
    claimedAt: '2026-09-16T07:28:06.000Z',
    startedAt: null,
    finishedAt: null,
    storyRunId: null,
    errorText: null,
    runtimeAdapter: null,
    externalRunId: null,
    attempts: 0,
    maxAttempts: 3,
    executionEnvironment: 'PROD',
    createdAt: '2026-09-16T07:28:06.000Z',
    updatedAt: '2026-09-16T07:28:06.000Z',
    ...overrides,
  }
}

function story() {
  return {
    id: 'ENG-QA-SINGLE-VERDICT-01',
    workstream: 'ENG',
    title: 'one adjudicator owns the QA verdict',
    priority: 'High',
    status: 'In Progress',
    notes: 'test fixture',
    batch: null,
    batchDeploy: false,
    goal: 'test goal',
    scope: 'test scope',
    dependencies: null,
    preconditions: null,
    architectBrief: null,
    contextRefs: null,
    acceptanceCriteria: null,
    postconditions: null,
    architectBriefUpdatedAt: null,
    completion: 0,
    rollup: true,
    plannedStartAt: null,
    actualStartAt: null,
    completedAt: null,
    createdAt: '2026-09-16T07:28:06.000Z',
    updatedAt: '2026-09-16T07:28:06.000Z',
    testMode: null,
  }
}

/**
 * The smallest durable work repo the shared lifecycle needs, plus the calls it
 * recorded. `storyRunId` seeds whether a run link exists — the second of the two
 * nulls this suite exists for.
 */
function fakeWork(storyRunId: string | null) {
  const calls = {
    fail: [] as string[],
    cancel: [] as string[],
    finish: [] as string[],
    progress: 0,
  }
  let item: Item = {
    id: WORK_ITEM_ID,
    storyId: 'ENG-QA-SINGLE-VERDICT-01',
    state: 'Claimed',
    storyRunId,
    errorText: null,
    role: 'architect',
    modelProfile: 'architect-pro',
    priority: 50,
    claimedBy: 'stub-adapter',
    attempts: 0,
    maxAttempts: 3,
  }
  const repo = {
    get: async () => ({ ...item }),
    claimSpecific: async () => ({ ...item }),
    beginRun: async () => {
      // The run link is whatever the seed said it was: `null` keeps the "no run
      // row" case reachable, which is the second null this suite exists for.
      item = { ...item, state: 'Running' }
      return { workItem: { ...item }, story: story() }
    },
    progress: async () => {
      calls.progress += 1
      return { ...item }
    },
    setRuntime: async () => ({ ...item }),
    finish: async (
      _id: string,
      input: {
        resultStatus: string
        completion: number
        notes: string
        testsSummary: string | null
        commitHash: string | null
      },
    ) => {
      calls.finish.push(input.notes)
      item = { ...item, state: 'Done' }
      // The run row the real repository returns, built from what was finished —
      // `normalizeEvidence` reads THIS object, not a fixture.
      return {
        workItem: { ...item },
        run: {
          id: item.storyRunId,
          storyId: item.storyId,
          role: item.role,
          resultStatus: input.resultStatus,
          completion: input.completion,
          notes: input.notes,
          testsSummary: input.testsSummary,
          commitHash: input.commitHash,
          startedAt: '2026-09-16T07:28:07.000Z',
          endedAt: '2026-09-16T07:29:30.000Z',
          executionEnvironment: 'PROD',
        },
        story: story(),
      }
    },
    fail: async (_id: string, errorText: string) => {
      calls.fail.push(errorText)
      item = { ...item, state: 'Error', errorText }
      return { ...item }
    },
    cancel: async (_id: string, note?: string) => {
      calls.cancel.push(note ?? '')
      item = { ...item, state: 'Cancelled' }
      return { ...item }
    },
  }
  return { repo: repo as never, calls }
}

function fakeRuns(run: Record<string, unknown> | null) {
  return { listForStory: async () => (run ? [run] : []) } as never
}

function evidence(overrides: Partial<AgentRunEvidence> = {}): AgentRunEvidence {
  return {
    resultStatus: 'Complete',
    completion: 100,
    notes: 'architect brief recorded',
    testsSummary: 'brief parsed',
    commitHash: null,
    runtimeAdapter: 'stub-adapter',
    modelProfile: 'architect-pro',
    externalRunId: 'stub-1',
    startedAt: '2026-09-16T07:28:07.000Z',
    endedAt: '2026-09-16T07:29:30.000Z',
    ...overrides,
  }
}

/**
 * A vendor adapter whose ONLY interesting behaviour is what its hooks answer.
 * `resultReply = null` is the contradiction this suite is about; `startErrorText`
 * stands in for a vendor failure the hook already measured.
 */
class StubAdapter extends AgentRuntimeAdapter {
  readonly runtimeAdapterId = 'stub-adapter'
  readonly capabilities: AgentCapability[] = []
  statusReply: ExternalStatusResult = { lifecycle: 'success' }
  resultReply: AgentRunEvidence | null = null
  startErrorText: string | null = null

  protected async startExternal(): Promise<ExternalStartResult> {
    this.externalErrorText = this.startErrorText
    return { externalRunId: 'stub-1' }
  }
  protected async statusExternal(): Promise<ExternalStatusResult> {
    return this.statusReply
  }
  protected async pauseExternal(): Promise<void> {}
  protected async resumeExternal(): Promise<void> {}
  protected async cancelExternal(): Promise<void> {}
  protected async resultExternal(): Promise<AgentRunEvidence | null> {
    return this.resultReply
  }
}

function context(cmd: AgentWorkCommand): AgentExecutionContext {
  return {
    command: cmd,
    story: story() as never,
    policy: { allowCommit: false, allowDevDbWrite: false, allowControlPlaneWrite: true },
    capabilities: [],
    executionEnvironment: 'PROD',
    storyRunId: '',
  }
}

// ---------------------------------------------------------------------------
// The contradiction: success with no evidence
// ---------------------------------------------------------------------------

test('a successful run with no evidence terminalizes as Failed with a named reason, never a TypeError', async () => {
  const { repo, calls } = fakeWork(null)
  const adapter = new StubAdapter({ work: repo, runs: fakeRuns(null) })
  adapter.resultReply = null

  const cmd = command()
  // Before the fix this rejected with
  // `TypeError: Cannot read properties of null (reading 'notes')`.
  const result = await adapter.execute(cmd, context(cmd))

  assert.equal(result.resultStatus, 'Failed')
  assert.equal(result.completion, 0)
  assert.equal(calls.fail.length, 1, 'the work item is failed exactly once')
  // The reason NAMES the contract breach and the item, so the engine ledger's
  // `last_error` says what went wrong instead of naming a property access.
  assert.match(calls.fail[0]!, /stub-adapter reported a successful run and returned no evidence/)
  assert.match(calls.fail[0]!, /wi-base-adapter-1/)
  assert.doesNotMatch(calls.fail[0]!, /reading 'notes'/)
  assert.equal(calls.finish.length, 0, 'nothing is finished as a success')
  // With no run row to read, the reason IS the evidence.
  assert.equal(result.notes, calls.fail[0])
})

test('the same contradiction reads its evidence from the run row when one exists', async () => {
  const { repo, calls } = fakeWork(RUN_ID)
  const failedRun = {
    id: RUN_ID,
    storyId: 'ENG-QA-SINGLE-VERDICT-01',
    role: 'architect',
    resultStatus: 'Failed',
    completion: 0,
    notes: 'Failed: stub-adapter returned no evidence',
    testsSummary: null,
    commitHash: null,
    startedAt: '2026-09-16T07:28:07.000Z',
    endedAt: '2026-09-16T07:29:30.000Z',
  }
  const adapter = new StubAdapter({ work: repo, runs: fakeRuns(failedRun) })
  adapter.resultReply = null

  const cmd = command()
  const result = await adapter.execute(cmd, context(cmd))

  assert.equal(result.resultStatus, 'Failed')
  // The run row is the ONE author of the evidence — the adapter does not invent a
  // second copy of what happened.
  assert.equal(result.notes, failedRun.notes)
  assert.equal(calls.fail.length, 1)
})




// ---------------------------------------------------------------------------
// The ordinary paths still work
// ---------------------------------------------------------------------------

test('a successful run with evidence finishes normally', async () => {
  const { repo, calls } = fakeWork(RUN_ID)
  const completedRun = {
    id: RUN_ID,
    storyId: 'ENG-QA-SINGLE-VERDICT-01',
    role: 'architect',
    resultStatus: 'Complete',
    completion: 100,
    notes: 'architect brief recorded',
    testsSummary: 'brief parsed',
    commitHash: null,
    startedAt: '2026-09-16T07:28:07.000Z',
    endedAt: '2026-09-16T07:29:30.000Z',
  }
  const adapter = new StubAdapter({ work: repo, runs: fakeRuns(completedRun) })
  adapter.resultReply = evidence()

  const cmd = command()
  const result = await adapter.execute(cmd, context(cmd))

  assert.equal(result.resultStatus, 'Complete')
  assert.equal(calls.finish.length, 1)
  assert.equal(calls.fail.length, 0)
  // `normalizeEvidence` reads the finished run row, which is the one author of
  // the notes the lane reports.
  assert.equal(result.notes, 'architect brief recorded')
})

test('a failed vendor status terminalizes as Failed even with no run link', async () => {
  const { repo, calls } = fakeWork(null)
  const adapter = new StubAdapter({ work: repo, runs: fakeRuns(null) })
  adapter.statusReply = { lifecycle: 'failed' }
  adapter.startErrorText = 'opencode exited 7'

  const cmd = command()
  const result = await adapter.execute(cmd, context(cmd))

  assert.equal(result.resultStatus, 'Failed')
  assert.deepEqual(calls.fail, ['opencode exited 7'])
  assert.equal(calls.cancel.length, 0)
})

test('a cancelled vendor status stays Cancelled even with no run link', async () => {
  const { repo, calls } = fakeWork(null)
  const adapter = new StubAdapter({ work: repo, runs: fakeRuns(null) })
  adapter.statusReply = { lifecycle: 'cancelled' }

  const cmd = command()
  const result = await adapter.execute(cmd, context(cmd))

  assert.equal(result.resultStatus, 'Cancelled')
  assert.equal(calls.cancel.length, 1)
  assert.equal(calls.fail.length, 0)
})
