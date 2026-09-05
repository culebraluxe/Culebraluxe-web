import { test } from 'node:test'
import assert from 'node:assert/strict'
import { randomUUID } from 'node:crypto'
import { interactiveSql } from '../../../lib/neon-interactive'
import {
  incrementForgeReplan,
  incrementForgeRepair,
  readForgeRepairLedger,
  recordForgeQaFailure,
} from '../../../db/forge-repair-ledger'

// ENG-FORGE-V11-S1 durable ledger persistence proof (real DEV DB). Confirms the
// repair/replan attempt counts and last QA disposition live on the canonical
// story row, survive a fresh read (restart neutral), increment atomically, and
// that an illegal disposition is rejected by the CHECK constraint.
const exe = interactiveSql as any

async function createStory(): Promise<string> {
  const storyId = `TMP-V11LEDGER-${Date.now()}-${randomUUID().slice(0, 8)}`
  await interactiveSql`
    insert into storyboard_story (
      id, workstream, title, priority, status, notes, completion, rollup
    ) values (
      ${storyId}, 'Platform / Engineering / Data', 'v11 ledger fixture',
      'High', 'Planned', 'temporary', 0, true
    )
  `
  return storyId
}

test('repair/replan ledger: initial state, atomic increments, durable re-read', async () => {
  const storyId = await createStory()
  try {
    const initial = await readForgeRepairLedger(storyId, exe)
    assert.deepEqual(
      {
        repairAttempts: initial?.repairAttempts,
        replanAttempts: initial?.replanAttempts,
        lastQaDisposition: initial?.lastQaDisposition,
      },
      { repairAttempts: 0, replanAttempts: 0, lastQaDisposition: null },
    )

    const r1 = await incrementForgeRepair(storyId, exe)
    const r2 = await incrementForgeRepair(storyId, exe)
    const r3 = await incrementForgeRepair(storyId, exe)
    assert.equal(r1.repairAttempts, 1)
    assert.equal(r3.repairAttempts, 3)
    assert.equal(r3.replanAttempts, 0)

    const p1 = await incrementForgeReplan(storyId, exe)
    const p2 = await incrementForgeReplan(storyId, exe)
    assert.equal(p2.replanAttempts, 2)

    await recordForgeQaFailure(storyId, { disposition: 'REPAIR', reason: 'tests 2/5 failed' }, exe)
    const recorded = await readForgeRepairLedger(storyId, exe)
    assert.equal(recorded?.lastQaDisposition, 'REPAIR')
    assert.match(recorded?.lastFailureReason ?? '', /tests 2\/5 failed/)

    // Fresh read == durable truth (a reconstructed executor/process sees the same).
    const fresh = await readForgeRepairLedger(storyId, exe)
    assert.deepEqual(
      {
        repairAttempts: fresh?.repairAttempts,
        replanAttempts: fresh?.replanAttempts,
        lastQaDisposition: fresh?.lastQaDisposition,
      },
      { repairAttempts: 3, replanAttempts: 2, lastQaDisposition: 'REPAIR' },
    )
  } finally {
    await interactiveSql`delete from storyboard_story where id = ${storyId}`
  }
})

test('repair/replan ledger: illegal QA disposition is rejected by the CHECK constraint', async () => {
  const storyId = await createStory()
  try {
    await assert.rejects(
      () => recordForgeQaFailure(storyId, { disposition: 'NOT_A_DISPOSITION' as never, reason: 'x' }, exe),
      (err: unknown) => {
        const text = String((err as Error)?.message ?? err)
        return /check|constraint/i.test(text) || /forge_qa_disposition/i.test(text)
      },
    )
    const after = await readForgeRepairLedger(storyId, exe)
    assert.equal(after?.lastQaDisposition, null, 'an illegal disposition must never be recorded')
  } finally {
    await interactiveSql`delete from storyboard_story where id = ${storyId}`
  }
})
