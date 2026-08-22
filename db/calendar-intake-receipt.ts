import type { QueryExecutor } from './query-executor'
import type {
  CalendarIntakeReceipt,
  CalendarIntakeReceiptStatus,
} from '../lib/calendar/contracts'
import type { CalendarIntakeDurability } from '../lib/calendar/lowering'
import type { CreateInteractionInput } from '../lib/crm-types'

// ---------------------------------------------------------------------------
// Calendar intake receipt/cursor repository (migration 040, CRM-08).
//
// Mirrors website_intake_submission's insert-or-read idempotency: UNIQUE
// (source_system, source_external_id) is the replay dedupe key — a replayed
// provider event inserts nothing and reads back the SAME receipt, so it can
// never create a duplicate canonical interaction.
//
// The claim/transition lifecycle mirrors CRM-04's claim token pattern:
//   insertOrRead  -> status 'received'
//   claimReceipt  -> 'received' -> 'processing' (or re-claims a processing
//                    receipt whose claim is stale, > 15 minutes old)
//   transition    -> 'processing' -> 'completed' (interaction_id required) |
//                    'rejected' | 'resolution_required' | 'duplicate'
//
// provider_cursor is the cursor that was current when the event was first
// seen; readCalendarIntakeCursor returns the most recently synced non-null
// cursor per source system (webhook-delivered receipts carry no cursor and
// never displace the poller's).
//
// These functions never call the provider (rejected design) — the lowering
// service (lib/calendar/lowering.ts) composes provider calls with these
// writes. Tokens never appear here (provider-side store, migration 041).
// ---------------------------------------------------------------------------

type ReceiptRow = {
  id: string
  source_system: string
  source_external_id: string
  status: CalendarIntakeReceiptStatus
  interaction_id: string | null
  provider_cursor: string | null
  last_sync_at: string | null
  processing_started_at: string | null
  created_at: string
  updated_at: string
}

function mapReceipt(row: ReceiptRow): CalendarIntakeReceipt {
  return {
    id: row.id,
    sourceSystem: row.source_system,
    sourceExternalId: row.source_external_id,
    status: row.status,
    interactionId: row.interaction_id ?? undefined,
    providerCursor: row.provider_cursor ?? null,
    lastSyncedAt: row.last_sync_at ?? null,
    processingStartedAt: row.processing_started_at ?? null,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  }
}

export type InsertCalendarIntakeReceiptInput = {
  sourceSystem: string
  sourceExternalId: string
  /** The provider cursor current when the event was first seen (poller only). */
  providerCursor: string | null
  /** When the event was observed (sync time / webhook arrival time). */
  syncedAt: string
}

/** Insert-or-read on the unique (source_system, source_external_id). */
export async function insertOrReadCalendarIntakeReceipt(
  input: InsertCalendarIntakeReceiptInput,
  execute: QueryExecutor,
): Promise<{ receipt: CalendarIntakeReceipt; created: boolean }> {
  const inserted = await execute`
    insert into calendar_intake_receipt (
      source_system, source_external_id, provider_cursor, last_sync_at
    ) values (
      ${input.sourceSystem}, ${input.sourceExternalId},
      ${input.providerCursor ?? null}, ${input.syncedAt}
    )
    on conflict (source_system, source_external_id) do nothing
    returning id, source_system, source_external_id, status, interaction_id,
      provider_cursor, last_sync_at, processing_started_at, created_at,
      updated_at
  `
  const created = inserted[0] as ReceiptRow | undefined
  if (created) return { receipt: mapReceipt(created), created: true }

  const existing = await execute`
    select id, source_system, source_external_id, status, interaction_id,
      provider_cursor, last_sync_at, processing_started_at, created_at,
      updated_at
    from calendar_intake_receipt
    where source_system = ${input.sourceSystem}
      and source_external_id = ${input.sourceExternalId}
    limit 1
  `
  const row = existing[0] as ReceiptRow | undefined
  if (!row) {
    throw new Error('Calendar intake receipt could not be resolved.')
  }
  return { receipt: mapReceipt(row), created: false }
}

/**
 * Claim a receipt for processing: 'received' -> 'processing' with a fresh
 * claim token; a 'processing' receipt whose claim is older than 15 minutes
 * (crash between claim and transition) is re-claimed. A fresh in-flight claim
 * returns null — the caller treats it as in-flight and does no work.
 */
export async function claimCalendarIntakeReceipt(
  receiptId: string,
  execute: QueryExecutor,
): Promise<CalendarIntakeReceipt | null> {
  const rows = await execute`
    update calendar_intake_receipt
    set status = 'processing',
        processing_started_at = now(),
        updated_at = now()
    where id = ${receiptId}
      and (
        status = 'received'
        or (status = 'processing'
          and processing_started_at <= now() - interval '15 minutes')
      )
    returning id, source_system, source_external_id, status, interaction_id,
      provider_cursor, last_sync_at, processing_started_at, created_at,
      updated_at
  `
  const row = rows[0] as ReceiptRow | undefined
  return row ? mapReceipt(row) : null
}

export type TransitionCalendarIntakeReceiptInput = {
  receiptId: string
  claimToken: string
  from: CalendarIntakeReceiptStatus
  to: Exclude<CalendarIntakeReceiptStatus, 'received' | 'processing'>
  interactionId?: string
}

const allowedTransitions = new Set([
  'processing:completed',
  'processing:rejected',
  'processing:resolution_required',
  'processing:duplicate',
])

/** Terminal transition guarded by the claim token (mirrors CRM-04). */
export async function transitionCalendarIntakeReceipt(
  input: TransitionCalendarIntakeReceiptInput,
  execute: QueryExecutor,
): Promise<boolean> {
  if (!allowedTransitions.has(`${input.from}:${input.to}`)) {
    throw new Error('Calendar intake receipt transition is not allowed.')
  }
  if ((input.to === 'completed') !== Boolean(input.interactionId)) {
    throw new Error('Only a completed receipt may have an interaction ID.')
  }

  const rows = await execute`
    update calendar_intake_receipt
    set status = ${input.to},
        processing_started_at = null,
        interaction_id = ${input.interactionId ?? null},
        updated_at = now()
    where id = ${input.receiptId}
      and status = ${input.from}
      and processing_started_at = ${input.claimToken}
    returning id
  `
  return rows.length === 1
}

export async function getCalendarIntakeReceiptBySourceIdentity(
  sourceSystem: string,
  sourceExternalId: string,
  execute: QueryExecutor,
): Promise<CalendarIntakeReceipt | null> {
  const rows = await execute`
    select id, source_system, source_external_id, status, interaction_id,
      provider_cursor, last_sync_at, processing_started_at, created_at,
      updated_at
    from calendar_intake_receipt
    where source_system = ${sourceSystem}
      and source_external_id = ${sourceExternalId}
    limit 1
  `
  const row = rows[0] as ReceiptRow | undefined
  return row ? mapReceipt(row) : null
}

/**
 * The current provider cursor for a source system: the most recently synced
 * receipt that carries a cursor (webhook-delivered receipts have none and are
 * skipped). A null result means "no cursor yet — run the initial lookback".
 */
export async function readCalendarIntakeCursor(
  sourceSystem: string,
  execute: QueryExecutor,
): Promise<string | null> {
  const rows = await execute`
    select provider_cursor
    from calendar_intake_receipt
    where source_system = ${sourceSystem}
      and provider_cursor is not null
    order by last_sync_at desc, created_at desc
    limit 1
  `
  const row = rows[0] as { provider_cursor: string } | undefined
  return row?.provider_cursor ?? null
}

// ---------------------------------------------------------------------------
// Durability factory — wires this repository (plus the canonical interaction
// insert) into the lowering service's CalendarIntakeDurability shape.
// ---------------------------------------------------------------------------

export function createCalendarIntakeDurability(
  execute: QueryExecutor,
  persistInteraction: (
    input: CreateInteractionInput,
  ) => Promise<{ interactionId: string; created: boolean }>,
): CalendarIntakeDurability {
  return {
    insertOrReadReceipt: (input) =>
      insertOrReadCalendarIntakeReceipt(input, execute),
    claimReceipt: (receiptId) => claimCalendarIntakeReceipt(receiptId, execute),
    transitionReceipt: (input) =>
      transitionCalendarIntakeReceipt(
        {
          receiptId: input.receiptId,
          claimToken: input.claimToken,
          from: input.from,
          to: input.to,
          interactionId: input.interactionId,
        },
        execute,
      ),
    readCurrentCursor: (sourceSystem) =>
      readCalendarIntakeCursor(sourceSystem, execute),
    persistInteraction,
  }
}
