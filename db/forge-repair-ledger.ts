import type { QueryExecutor } from './query-executor'
import type { QaDisposition } from '../workflow_app/forge/qa-repair-policy'

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
  lastQaDisposition: QaDisposition | null
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
    lastQaDisposition: row.forge_last_qa_disposition as QaDisposition | null,
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

