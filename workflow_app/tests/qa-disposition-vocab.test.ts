import assert from 'node:assert/strict'
import test from 'node:test'
import { readFileSync } from 'node:fs'

import {
  QA_PASS_DISPOSITION,
  QA_STORED_DISPOSITIONS,
  QA_UNKNOWN_DISPOSITION,
  classifyStoredQaDisposition,
  routeQaResult,
  storedReadingToQaResultInput,
  type QaDisposition,
} from '../forge/qa-repair-policy'
import {
  readForgeRepairLedger,
  recordForgeQaFailure,
  recordForgeQaPass,
} from '../../db/forge-repair-ledger'
import type { QueryExecutor, QueryRow } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// ENG-FORGE-QA-VERDICT-VOCAB-01 — ONE vocabulary for the writer, the constraint
// and the reader.
//
// `forge_last_qa_disposition` was guarded by a constraint that allowed only
// REPAIR/REPLAN/ESCALATE while the pass writer wrote PASS, so every clean-pass
// write was rejected and the column stayed null. These assertions DRIVE the real
// writers and the real reader against the constraint's own vocabulary — they do
// not inspect it — and they prove an out-of-vocabulary stored value reads back as
// UNKNOWN rather than being cast into a repair action.
// ---------------------------------------------------------------------------

const MIGRATION = 'db/migrations/189_forge_qa_disposition_vocab.sql'

const FAILURE_DISPOSITIONS: readonly QaDisposition[] = ['REPAIR', 'REPLAN', 'ESCALATE']

/** The values the CHECK constraint itself declares, read from the migration. */
function constraintVocabulary(): string[] {
  const sql = readFileSync(MIGRATION, 'utf8')
  const at = sql.indexOf('storyboard_story_forge_qa_disposition_check')
  const body = at >= 0 ? sql.slice(at) : ''
  const match = body.match(/in \(([^)]*)\)/i)
  if (!match) return []
  return [...match[1].matchAll(/'([^']+)'/g)].map((entry) => entry[1])
}

type LedgerRow = {
  id: string
  forge_repair_attempts: number
  forge_replan_attempts: number
  forge_last_qa_disposition: string | null
  forge_last_failure_reason: string | null
}

function blankRow(disposition: string | null = null): LedgerRow {
  return {
    id: 'STORY-VOCAB-1',
    forge_repair_attempts: 0,
    forge_replan_attempts: 0,
    forge_last_qa_disposition: disposition,
    forge_last_failure_reason: null,
  }
}

/**
 * A stand-in for the engine DB: the REAL writers and the REAL reader, no network.
 * An update mutates the one row; a select reads it back, so "recorded and read
 * back" is exercised end to end.
 */
function ledgerDb(row: LedgerRow): QueryExecutor {
  const execute = async (strings: TemplateStringsArray, ...values: unknown[]): Promise<QueryRow[]> => {
    const sql = strings.join('?').replace(/\s+/g, ' ').trim().toLowerCase()
    if (sql.startsWith('update storyboard_story')) {
      if (sql.includes('forge_last_failure_reason = null')) {
        row.forge_last_qa_disposition = String(values[0])
        row.forge_last_failure_reason = null
      } else {
        row.forge_last_qa_disposition = String(values[0])
        row.forge_last_failure_reason = String(values[1])
      }
      // The failure writer's statement RETURNS the authoritative row; the pass writer's does not.
      return sql.includes('returning') ? [row as unknown as QueryRow] : []
    }
    return [row as unknown as QueryRow]
  }
  return execute as QueryExecutor
}

function routeStored(reading: ReturnType<typeof classifyStoredQaDisposition>) {
  return routeQaResult({
    ...storedReadingToQaResultInput(reading),
    state: { repairAttempts: 0, replanAttempts: 0 },
  })
}

test('each writer disposition value is accepted by the constraint vocabulary', async () => {
  const constraint = constraintVocabulary()
  assert.ok(constraint.length > 0, 'the migration declares the constraint vocabulary')
  assert.deepEqual(
    [...constraint].sort(),
    [...QA_STORED_DISPOSITIONS].sort(),
    'the constraint vocabulary IS the one shared vocabulary',
  )

  // DRIVEN, not inspected: run the real writers and check what each one stores.
  const passRow = blankRow()
  await recordForgeQaPass('STORY-VOCAB-1', ledgerDb(passRow))
  assert.ok(
    constraint.includes(String(passRow.forge_last_qa_disposition)),
    'the PASS writer stores a value the constraint accepts',
  )

  for (const disposition of FAILURE_DISPOSITIONS) {
    const row = blankRow()
    await recordForgeQaFailure(
      'STORY-VOCAB-1',
      { disposition, reason: `failed ${disposition}` },
      ledgerDb(row),
    )
    assert.ok(
      constraint.includes(String(row.forge_last_qa_disposition)),
      `the ${disposition} writer stores a value the constraint accepts`,
    )
  }
})

test('a clean QA PASS is recorded and read back as a pass', async () => {
  const row = blankRow()
  await recordForgeQaPass('STORY-VOCAB-1', ledgerDb(row))

  const ledger = await readForgeRepairLedger('STORY-VOCAB-1', ledgerDb(row))
  assert.equal(ledger?.lastQaDisposition, QA_PASS_DISPOSITION, 'the recorded PASS is read back as a pass')
  assert.equal(ledger?.lastFailureReason, null, 'a pass clears the stale failure reason')
  assert.deepEqual(routeStored(ledger?.lastQaDisposition ?? null), { action: 'pass' })
})

test('a failure disposition is recorded and read back as that action', async () => {
  const expected: Record<QaDisposition, 'smith' | 'architect' | 'hold'> = {
    REPAIR: 'smith',
    REPLAN: 'architect',
    ESCALATE: 'hold',
  }

  for (const disposition of FAILURE_DISPOSITIONS) {
    const row = blankRow()
    await recordForgeQaFailure(
      'STORY-VOCAB-1',
      { disposition, reason: `failed ${disposition}` },
      ledgerDb(row),
    )

    const ledger = await readForgeRepairLedger('STORY-VOCAB-1', ledgerDb(row))
    assert.equal(ledger?.lastQaDisposition, disposition, `${disposition} is read back as itself`)
    assert.equal(
      routeStored(ledger?.lastQaDisposition ?? null).action,
      expected[disposition],
      `${disposition} routes to ${expected[disposition]}`,
    )
  }
})

test('an unknown stored value is reported as unknown and never as a repair action', async () => {
  assert.equal(
    classifyStoredQaDisposition('NOT_A_DISPOSITION'),
    QA_UNKNOWN_DISPOSITION,
    'a value outside the vocabulary classifies as unknown',
  )

  const row = blankRow('NOT_A_DISPOSITION')
  const ledger = await readForgeRepairLedger('STORY-VOCAB-1', ledgerDb(row))
  assert.equal(ledger?.lastQaDisposition, QA_UNKNOWN_DISPOSITION, 'the reader reports unknown, never a cast')

  const routed = routeStored(ledger?.lastQaDisposition ?? null)
  assert.equal(routed.action, 'hold', 'an unknown stored value fails closed into a hold')
  assert.notEqual(routed.action, 'smith', 'an unknown value is never a repair action')
  assert.notEqual(routed.action, 'architect', 'an unknown value is never a replan action')
})
