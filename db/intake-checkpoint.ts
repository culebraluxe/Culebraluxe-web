// ---------------------------------------------------------------------------
// INTAKE CHECKPOINT repository (migration 162).
//
// The durable resume token for the bounded mailbox intake. One row per
// (source, source_account, shard_key). This is ACQUISITION state only: it holds
// no Person decision and promotion never reads it.
//
// The progression rule lives in the intake loop (lib/intake/mailbox-paging.ts):
// the cursor advances ONLY once every record in a page has landed or replayed.
// This file just persists/reads the row.
//
// Repository boundary: driver-native values are normalized HERE (timestamptz to
// ISO string or null, bigint to a safe JS number) so the intake loop never needs
// to know what Neon/Postgres returned.
// ---------------------------------------------------------------------------

import type { QueryExecutor } from './query-executor'

export type IntakeCheckpointStatus = 'in_progress' | 'complete'

export type IntakeCheckpoint = {
  source: string
  sourceAccount: string
  shardKey: string
  shardStart: string | null
  shardEnd: string | null
  cursor: string | null
  pagesCompleted: number
  recordsSeen: number
  recordsLanded: number
  recordsReplayed: number
  errors: number
  status: IntakeCheckpointStatus
  updatedAt: string
}

type CheckpointRow = {
  source: string
  source_account: string
  shard_key: string
  shard_start: unknown
  shard_end: unknown
  cursor: string | null
  pages_completed: unknown
  records_seen: unknown
  records_landed: unknown
  records_replayed: unknown
  errors: unknown
  status: string
  updated_at: unknown
}

function isoOrNull(value: unknown): string | null {
  if (value == null) return null
  if (value instanceof Date) return value.toISOString()
  const parsed = new Date(String(value))
  return Number.isNaN(parsed.getTime()) ? null : parsed.toISOString()
}

function safeNumber(value: unknown): number {
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : 0
}

function normalize(row: CheckpointRow): IntakeCheckpoint {
  return {
    source: row.source,
    sourceAccount: row.source_account,
    shardKey: row.shard_key,
    shardStart: isoOrNull(row.shard_start),
    shardEnd: isoOrNull(row.shard_end),
    cursor: row.cursor ?? null,
    pagesCompleted: safeNumber(row.pages_completed),
    recordsSeen: safeNumber(row.records_seen),
    recordsLanded: safeNumber(row.records_landed),
    recordsReplayed: safeNumber(row.records_replayed),
    errors: safeNumber(row.errors),
    status: row.status === 'complete' ? 'complete' : 'in_progress',
    updatedAt: isoOrNull(row.updated_at) ?? new Date(0).toISOString(),
  }
}

/** Read the checkpoint for one shard, or null if this shard has never been run. */
export async function getIntakeCheckpoint(
  source: string,
  sourceAccount: string,
  shardKey: string,
  execute: QueryExecutor,
): Promise<IntakeCheckpoint | null> {
  const rows = (await execute`
    select source, source_account, shard_key, shard_start, shard_end, cursor,
           pages_completed, records_seen, records_landed, records_replayed,
           errors, status, updated_at
    from integration_intake_checkpoint
    where source = ${source} and source_account = ${sourceAccount} and shard_key = ${shardKey}
  `) as unknown as CheckpointRow[]
  const row = rows[0]
  return row ? normalize(row) : null
}

export type SaveIntakeCheckpointInput = {
  source: string
  sourceAccount: string
  shardKey: string
  shardStart?: string | null
  shardEnd?: string | null
  cursor: string | null
  pagesCompleted: number
  recordsSeen: number
  recordsLanded: number
  recordsReplayed: number
  errors: number
  status: IntakeCheckpointStatus
}

/** Upsert the shard checkpoint. Called ONLY after a page has fully landed/replayed. */
export async function saveIntakeCheckpoint(
  input: SaveIntakeCheckpointInput,
  execute: QueryExecutor,
): Promise<IntakeCheckpoint> {
  const rows = (await execute`
    insert into integration_intake_checkpoint (
      source, source_account, shard_key, shard_start, shard_end, cursor,
      pages_completed, records_seen, records_landed, records_replayed, errors,
      status, updated_at
    ) values (
      ${input.source}, ${input.sourceAccount}, ${input.shardKey},
      ${input.shardStart ?? null}::timestamptz, ${input.shardEnd ?? null}::timestamptz,
      ${input.cursor}, ${input.pagesCompleted}, ${input.recordsSeen},
      ${input.recordsLanded}, ${input.recordsReplayed}, ${input.errors},
      ${input.status}, now()
    )
    on conflict (source, source_account, shard_key) do update set
      shard_start      = excluded.shard_start,
      shard_end        = excluded.shard_end,
      cursor           = excluded.cursor,
      pages_completed  = excluded.pages_completed,
      records_seen     = excluded.records_seen,
      records_landed   = excluded.records_landed,
      records_replayed = excluded.records_replayed,
      errors           = excluded.errors,
      status           = excluded.status,
      updated_at       = now()
    returning source, source_account, shard_key, shard_start, shard_end, cursor,
              pages_completed, records_seen, records_landed, records_replayed,
              errors, status, updated_at
  `) as unknown as CheckpointRow[]
  return normalize(rows[0])
}

/** Every checkpoint for a source account, for reporting a run's overall state. */
export async function listIntakeCheckpoints(
  source: string,
  sourceAccount: string,
  execute: QueryExecutor,
): Promise<IntakeCheckpoint[]> {
  const rows = (await execute`
    select source, source_account, shard_key, shard_start, shard_end, cursor,
           pages_completed, records_seen, records_landed, records_replayed,
           errors, status, updated_at
    from integration_intake_checkpoint
    where source = ${source} and source_account = ${sourceAccount}
    order by shard_key desc
  `) as unknown as CheckpointRow[]
  return rows.map(normalize)
}
