import assert from 'node:assert/strict'
import test from 'node:test'

import {
  completeRoleTaskThenRecordQaDisposition,
  type ForgeRoleOutcome,
} from '@/legacy/workflow_app/forge/forge-executor'
import { adjudicateAssay } from '@/legacy/workflow_app/forge/agents/qa/run'
import type { CommandResult } from '@/legacy/workflow_app/forge/agents/qa/types'
import type { QaDisposition } from '@/legacy/workflow_app/forge/qa-repair-policy'
import type { ForgeRepairLedger } from '@/legacy/db/forge-repair-ledger'

// ---------------------------------------------------------------------------
// ENG-FORGE-QA-RACE-01 — two QA adjudication risks closed.
//
//   1. RACE: a QA worker that loses the engine's completion CAS writes no disposition,
//      so it cannot overwrite the winner's verdict on storyboard_story.
//   2. IDENTITY: the adjudicator compares the SET of command identities, not their count,
//      so a same-count substitution is refused by name.
// ---------------------------------------------------------------------------

/** A stand-in for the engine DB. The injected writers mean no executor is ever used. */
const noDb = {} as never

type Ledger = { disposition: string | null; reason: string | null }

/** In-memory stand-ins for recordForgeQaFailure / recordForgeQaPass. */
function writers(ledger: Ledger) {
  return {
    recordFailure: async (
      storyId: string,
      input: { disposition: QaDisposition; reason: string },
    ): Promise<ForgeRepairLedger> => {
      ledger.disposition = input.disposition
      ledger.reason = input.reason
      return {
        storyId,
        repairAttempts: 0,
        replanAttempts: 0,
        lastQaDisposition: input.disposition,
        lastFailureReason: input.reason,
      }
    },
    recordPass: async (): Promise<void> => {
      ledger.disposition = 'PASS'
      ledger.reason = null
    },
  }
}

test('a losing QA worker writes no durable disposition', async () => {
  const ledger: Ledger = { disposition: null, reason: null }
  let writes = 0
  await assert.rejects(
    completeRoleTaskThenRecordQaDisposition({
      nodeId: 'qa_verify',
      storyId: 'STORY-RACE-1',
      outcome: {
        evidence: {
          qaPassed: false,
          disposition: 'REPAIR',
          failedCommands: ['node --test loser'],
        },
      },
      complete: async () => {
        throw new Error('TASK_ALREADY_COMPLETED')
      },
      execute: noDb,
      recordFailure: async () => {
        writes++
        return {} as ForgeRepairLedger
      },
      recordPass: async () => {
        writes++
      },
    }),
    /TASK_ALREADY_COMPLETED/,
  )
  assert.equal(writes, 0, 'a worker that lost the completion race must write nothing')
  assert.equal(ledger.disposition, null)
})

test('only the winning disposition survives two competing completions', async () => {
  const ledger: Ledger = { disposition: null, reason: null }
  const { recordFailure, recordPass } = writers(ledger)

  const winningOutcome: ForgeRoleOutcome = { evidence: { qaPassed: true } }
  const losingOutcome: ForgeRoleOutcome = {
    evidence: {
      qaPassed: false,
      disposition: 'REPAIR',
      failedCommands: ['node --test loser'],
    },
  }

  const winningWorker = completeRoleTaskThenRecordQaDisposition({
    nodeId: 'qa_verify',
    storyId: 'STORY-RACE-2',
    outcome: winningOutcome,
    complete: async () => {},
    execute: noDb,
    recordFailure,
    recordPass,
  })
  const losingWorker = completeRoleTaskThenRecordQaDisposition({
    nodeId: 'qa_verify',
    storyId: 'STORY-RACE-2',
    outcome: losingOutcome,
    complete: async () => {
      throw new Error('TASK_ALREADY_COMPLETED')
    },
    execute: noDb,
    recordFailure,
    recordPass,
  }).catch(() => undefined)

  await Promise.all([winningWorker, losingWorker])
  assert.equal(ledger.disposition, 'PASS', 'only the winner disposition survives')
  assert.equal(ledger.reason, null, 'the loser failure reason never lands')
})

test('a same-count command substitution is refused by name', () => {
  const planned = 'node --import tsx --test workflow_app/tests/a.test.ts'
  const substituted = 'node --import tsx --test workflow_app/tests/b.test.ts'
  const plan = { commands: [planned] }
  const substitutedResult: CommandResult = {
    command: substituted,
    exitCode: 0,
    passed: true,
    excerpt: '',
  }

  const refused = adjudicateAssay({ plan, commands: [substitutedResult] })
  assert.equal(refused.verdict, 'FAIL', 'a same-count substitution is never the same evidence')
  assert.ok(
    refused.blockers.includes(`ASSAY_COMMAND_SUBSTITUTED ${substituted}`),
    'the substituted command is named',
  )
  assert.ok(!refused.blockers.includes('ASSAY_COMMAND_DRIFT'), 'the count did not drift')

  const identical = adjudicateAssay({
    plan,
    commands: [{ command: planned, exitCode: 0, passed: true, excerpt: '' }],
  })
  assert.equal(identical.verdict, 'PASS')
  assert.deepEqual(identical.blockers, [])
})
