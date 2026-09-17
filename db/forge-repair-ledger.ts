import type { QueryExecutor } from './query-executor'
import {
  QA_PASS_DISPOSITION,
  classifyStoredQaDisposition,
  type QaDisposition,
  type QaDispositionReading,
} from '../workflow_app/forge/qa-repair-policy'

// ---------------------------------------------------------------------------
// ENG-FORGE-V11-S1 — durable repair/replan lifecycle ledger.
//
// Story-scoped attempt counters + last QA disposition, persisted on
// storyboard_story (migration 114). Every write is a single atomic statement so
// two workers cannot independently authorize an extra retry: the returned
// authoritative counts are whatever the row held after the one-row UPDATE.
// Reading is a plain SELECT, so a reconstructed process sees identical state
// (restart / stale-worker recovery / task reclaim neutral).
//
// NOTE: the Neon driver parameterizes interpolated string values, so a shared
// column-list constant can never be interpolated into a tagged template (it
// would become `$1`). Column lists are written literally in every query.
// ---------------------------------------------------------------------------

export type ForgeRepairLedger = {
  storyId: string
  repairAttempts: number
  replanAttempts: number
  /**
   * The stored QA outcome, classified against the ONE vocabulary. A stored value the
   * vocabulary does not recognise reads as UNKNOWN — never cast into a failure
   * disposition, which is what would misroute the next lane.
   */
  lastQaDisposition: QaDispositionReading | null
  lastFailureReason: string | null
}

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

type LedgerRow = {
  id: string
  forge_repair_attempts: number
  forge_replan_attempts: number
  forge_last_qa_disposition: string | null
  forge_last_failure_reason: string | null
}

function mapRow(row: LedgerRow): ForgeRepairLedger {
  return {
    storyId: row.id,
    repairAttempts: row.forge_repair_attempts,
    replanAttempts: row.forge_replan_attempts,
    lastQaDisposition: classifyStoredQaDisposition(row.forge_last_qa_disposition),
    lastFailureReason: row.forge_last_failure_reason,
  }
}

export async function readForgeRepairLedger(
  storyId: string,
  execute?: QueryExecutor,
): Promise<ForgeRepairLedger | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, forge_repair_attempts, forge_replan_attempts,
      forge_last_qa_disposition, forge_last_failure_reason
    from storyboard_story
    where id = ${storyId}
  `
  const row = rows[0] as LedgerRow | undefined
  return row ? mapRow(row) : null
}

/**
 * Atomically increment the repair counter (single-statement). Returns the
 * authoritative post-increment ledger row. Callers enforce the budget via the
 * pure policy before dispatching; this is the durable commit of that decision.
 */
export async function incrementForgeRepair(
  storyId: string,
  execute?: QueryExecutor,
): Promise<ForgeRepairLedger> {
  const q = execute ?? (await executor())
  const rows = await q`
    update storyboard_story
    set forge_repair_attempts = forge_repair_attempts + 1,
        updated_at = now()
    where id = ${storyId}
    returning id, forge_repair_attempts, forge_replan_attempts,
      forge_last_qa_disposition, forge_last_failure_reason
  `
  const row = rows[0] as LedgerRow | undefined
  if (!row) throw new Error(`Forge repair ledger: story ${storyId} not found`)
  return mapRow(row)
}

/**
 * Atomically increment the replan counter (single-statement).
 */
export async function incrementForgeReplan(
  storyId: string,
  execute?: QueryExecutor,
): Promise<ForgeRepairLedger> {
  const q = execute ?? (await executor())
  const rows = await q`
    update storyboard_story
    set forge_replan_attempts = forge_replan_attempts + 1,
        updated_at = now()
    where id = ${storyId}
    returning id, forge_repair_attempts, forge_replan_attempts,
      forge_last_qa_disposition, forge_last_failure_reason
  `
  const row = rows[0] as LedgerRow | undefined
  if (!row) throw new Error(`Forge repair ledger: story ${storyId} not found`)
  return mapRow(row)
}

/**
 * Record the machine-readable QA disposition + failure reason for a FAIL. The
 * disposition CHECK constraint rejects anything outside REPAIR/REPLAN/ESCALATE.
 */
/**
 * A CLEAN QA PASS IS A DISPOSITION TOO.
 *
 * `forge_last_qa_disposition` was written ONLY by `recordForgeQaFailure`, so a story that passed left the field
 * null — and the chain, which reads that field to decide whether QA is done, saw no verdict and re-ran QA on
 * every pass. That is the infinite QA loop: not a failing test, a missing writer for the passing case.
 *
 * The value is the shared vocabulary's PASS constant, not a literal, so the writer and the CHECK constraint
 * cannot drift: the same constant is what the migration's `in (...)` list declares.
 */
export async function recordForgeQaPass(storyId: string, execute?: QueryExecutor): Promise<void> {
  const q = execute ?? (await executor())
  await q`
    update storyboard_story
    set forge_last_qa_disposition = ${QA_PASS_DISPOSITION},
        forge_last_failure_reason = null,
        updated_at = now()
    where id = ${storyId}
  `
}

export async function recordForgeQaFailure(
  storyId: string,
  input: { disposition: QaDisposition; reason: string },
  execute?: QueryExecutor,
): Promise<ForgeRepairLedger> {
  const q = execute ?? (await executor())
  const rows = await q`
    update storyboard_story
    set forge_last_qa_disposition = ${input.disposition},
        forge_last_failure_reason = ${input.reason},
        updated_at = now()
    where id = ${storyId}
    returning id, forge_repair_attempts, forge_replan_attempts,
      forge_last_qa_disposition, forge_last_failure_reason
  `
  const row = rows[0] as LedgerRow | undefined
  if (!row) throw new Error(`Forge repair ledger: story ${storyId} not found`)
  return mapRow(row)
}

