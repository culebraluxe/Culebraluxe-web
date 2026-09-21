import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  isCompletedReleaseConflict,
  runWaveBatch,
  settleForgeLaneFailure,
  type WaveLane,
} from '@/legacy/workflow_app/forge/forge-executor'

// ---------------------------------------------------------------------------
// ENG-FORGE-RELEASE-CLEANUP-01 — a release that cannot apply because the task completed is a
// no-op, not a crash.
//
//   1. Releasing an already-completed task resolves; completing is strictly stronger than releasing.
//   2. A release failure for ANY other reason still throws, naming both the lane and release causes.
//   3. A lane that fails after its task completed reports the failure, keeps the advance, and the
//      wave still settles its siblings.
// ---------------------------------------------------------------------------

/** The engine's refusal for a completed task: TASK_NOT_RELEASABLE, message names `status: completed`. */
function completedReleaseConflict(): Error {
  const err = new Error('Task cannot be released in status: completed') as Error & { code?: string }
  err.name = 'WorkflowConflictError'
  err.code = 'TASK_NOT_RELEASABLE'
  return err
}

const delay = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms))

function lane(name: string): WaveLane<string> {
  return { lane: name, surface: null, task: name }
}

test('release after completion returns without error', async () => {
  const reports: Array<{ label: string; error: unknown }> = []
  const laneError = new Error('lane failed after its task completed')

  const settlement = await settleForgeLaneFailure({
    taskId: 'task-completed',
    nodeId: 'qa_verify',
    actor: 'actor-1',
    laneError,
    release: async () => {
      throw completedReleaseConflict()
    },
    report: (label, error) => {
      reports.push({ label, error })
    },
  })

  assert.equal(settlement, 'already-completed', 'a completed task is not an error to release')
  assert.equal(reports.length, 1, 'the lane failure is still reported, never swallowed')
  assert.equal(reports[0]!.error, laneError)
  assert.ok(isCompletedReleaseConflict(completedReleaseConflict()))
  assert.ok(
    !isCompletedReleaseConflict(new Error('Process p is not active (status=cancelled)')),
    'a non-completion release conflict is not the completed signal',
  )
})

test('a release that fails for any other reason still throws', async () => {
  await assert.rejects(
    settleForgeLaneFailure({
      taskId: 'task-open',
      nodeId: 'smith',
      actor: 'actor-2',
      laneError: new Error('lane exploded'),
      release: async () => {
        throw new Error('release exploded')
      },
      report: () => {},
    }),
    (err: unknown) => {
      assert.ok(err instanceof AggregateError, 'both causes are aggregated, not one swallowed')
      assert.match(err.message, /lane exploded/)
      assert.match(err.message, /release exploded/)
      return true
    },
  )
})

test('a lane failure after completion keeps the advance and settles its siblings', async () => {
  const settled: string[] = []
  const reported: string[] = []

  await runWaveBatch([lane('completed-lane'), lane('sibling')], async (item) => {
    if (item.lane === 'completed-lane') {
      const settlement = await settleForgeLaneFailure({
        taskId: 'task-advanced',
        nodeId: 'qa_verify',
        actor: 'actor-3',
        laneError: new Error('lane failed after its task completed'),
        release: async () => {
          throw completedReleaseConflict()
        },
        report: (label) => {
          reported.push(label)
        },
      })
      assert.equal(settlement, 'already-completed')
      return // the advance stands: this lane must not reject the wave
    }
    await delay(10)
    settled.push(item.lane)
  })

  assert.deepEqual(settled, ['sibling'], 'every sibling settled before the wave resolved')
  assert.equal(reported.length, 1, 'the lane failure was reported')
})
