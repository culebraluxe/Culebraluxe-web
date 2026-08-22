// ---------------------------------------------------------------------------
// CRM-08 — Calendar intake lowering service (the LIVE durability layer).
//
// This is the poller/webhook intake path that makes calendar ingestion
// durable on top of the pure readiness coordinator
// (lib/crm-calendar-intake.ts) and the neutral adapter
// (lib/crm-calendar-normalization.ts):
//
//   syncCalendarEvents (poller)          handleCalendarWebhook (push)
//        │                                        │
//        └──────────────▶ processCalendarEvent ◀──┘
//                            │  1. insert-or-read receipt (UNIQUE source
//                            │     identity — the replay dedupe key)
//                            │  2. claim (received → processing)
//                            │  3. adaptCalendarEvent → prepareCalendarIntake
//                            │  4. persist the canonical interaction ONLY when
//                            │     status === 'ready' (no person, no task)
//                            │  5. transition the receipt to completed /
//                            │     rejected / resolution_required / duplicate
//
// Rules preserved (architect brief, REJECTED list):
//   - allowCreation stays false: a calendar event NEVER auto-creates a person
//     (attendee identity is organizer-supplied and never ownership-verified).
//   - No follow-up task noise is derived from an appointment — the only
//     canonical write is the interaction row (db/interactions.createInteraction
//     writes the interaction only).
//   - Deterministic idempotency via (source_system, source_external_id): a
//     replayed provider event reads back the SAME receipt and cannot create a
//     second interaction.
//   - Provider payloads, SDK objects, credentials, and tokens never cross
//     into CRM rows — only the neutral CalendarProviderEvent does.
//
// Cursor: each receipt records the provider cursor that was current when the
// event was first seen; readCurrentCursor (part of the durability seam)
// returns the most recently synced non-null cursor per source system, so the
// next poll starts exactly where the previous one ended.
// ---------------------------------------------------------------------------

import { prepareCalendarIntake } from '../crm-calendar-intake'
import type { CalendarIntakeRepositories } from '../crm-calendar-intake'
import type {
  CalendarAdapterConfiguration,
  CalendarProviderEvent,
} from '../crm-calendar-types'
import type { CreateInteractionInput } from '../crm-types'
import type {
  CalendarIntakeReceipt,
  CalendarIntakeReceiptStatus,
  CalendarProvider,
  CalendarWebhookVerification,
} from './contracts'

export type CalendarTerminalReceiptStatus = Exclude<
  CalendarIntakeReceiptStatus,
  'received' | 'processing'
>

/** The durable boundary the lowering service needs (migration 040 + the
 *  canonical interaction insert). Tests inject an in-memory implementation;
 *  production uses db/calendar-intake-receipt.createCalendarIntakeDurability. */
export interface CalendarIntakeDurability {
  insertOrReadReceipt(input: {
    sourceSystem: string
    sourceExternalId: string
    providerCursor: string | null
    syncedAt: string
  }): Promise<{ receipt: CalendarIntakeReceipt; created: boolean }>
  claimReceipt(receiptId: string): Promise<CalendarIntakeReceipt | null>
  transitionReceipt(input: {
    receiptId: string
    claimToken: string
    from: CalendarIntakeReceiptStatus
    to: CalendarTerminalReceiptStatus
    interactionId?: string
  }): Promise<boolean>
  readCurrentCursor(sourceSystem: string): Promise<string | null>
  /** The ONLY canonical write: the interaction row, never a person/task. */
  persistInteraction(
    input: CreateInteractionInput,
  ): Promise<{ interactionId: string; created: boolean }>
}

export type CalendarEventProcessingOutcome =
  | {
      outcome: 'completed'
      receipt: CalendarIntakeReceipt
      interactionId: string
      created: boolean
    }
  | {
      outcome: 'duplicate'
      receipt: CalendarIntakeReceipt
      existingInteractionId?: string
    }
  | { outcome: 'rejected'; receipt: CalendarIntakeReceipt; reason: string }
  | {
      outcome: 'resolution_required'
      receipt: CalendarIntakeReceipt
      reason: string
    }
  | { outcome: 'in_flight'; receipt: CalendarIntakeReceipt }

function outcomeFromReceipt(
  receipt: CalendarIntakeReceipt,
): CalendarEventProcessingOutcome {
  switch (receipt.status) {
    case 'completed':
      return {
        outcome: 'completed',
        receipt,
        interactionId: receipt.interactionId as string,
        created: false,
      }
    case 'duplicate':
      return { outcome: 'duplicate', receipt }
    case 'rejected':
      return { outcome: 'rejected', receipt, reason: 'previously_rejected' }
    case 'resolution_required':
      return { outcome: 'resolution_required', receipt, reason: 'previously_requiring_resolution' }
    default:
      return { outcome: 'in_flight', receipt }
  }
}

export type ProcessCalendarEventInput = {
  event: CalendarProviderEvent
  configuration: CalendarAdapterConfiguration
  repositories: CalendarIntakeRepositories
  durability: CalendarIntakeDurability
  /** The provider cursor current for this sync (recorded on new receipts). */
  cursor: string | null
  /** When the event was observed (sync time / webhook arrival time). */
  syncedAt: string
}

/**
 * Lower ONE provider event into a durable receipt + (when ready) exactly one
 * canonical interaction. Idempotent: a replayed event with a terminal receipt
 * returns the recorded outcome and writes nothing.
 */
export async function processCalendarEvent(
  input: ProcessCalendarEventInput,
): Promise<CalendarEventProcessingOutcome> {
  const { event, configuration, repositories, durability } = input
  const sourceSystem = `calendar:${event.provider}:${event.accountNamespace}`

  const inserted = await durability.insertOrReadReceipt({
    sourceSystem,
    sourceExternalId: event.providerEventId,
    providerCursor: input.cursor,
    syncedAt: input.syncedAt,
  })
  let receipt = inserted.receipt

  // Replay of an already-recorded source identity with a terminal status:
  // return the recorded outcome, write nothing.
  if (!inserted.created && isTerminal(receipt.status)) {
    return outcomeFromReceipt(receipt)
  }

  // Claim: a new 'received' receipt -> 'processing'; a stale in-flight
  // 'processing' receipt (crash between claim and transition) is re-claimed.
  // A fresh in-flight claim returns null -> in_flight (another worker owns it).
  const claimed = await durability.claimReceipt(receipt.id)
  if (!claimed) return { outcome: 'in_flight', receipt }
  receipt = claimed

  const result = await prepareCalendarIntake(event, configuration, repositories)

  switch (result.status) {
    case 'ready': {
      const interactionInput = result.intakeResult.interactionInput
      if (!interactionInput) {
        // Unreachable for a 'ready' intake result; fail closed without a write.
        return { outcome: 'rejected', receipt, reason: 'ready_without_interaction_input' }
      }
      const persisted = await durability.persistInteraction(interactionInput)
      const transitioned = await transition(
        durability,
        receipt,
        'completed',
        persisted.interactionId,
      )
      if (!transitioned) return { outcome: 'in_flight', receipt }
      return {
        outcome: 'completed',
        receipt: transitioned,
        interactionId: persisted.interactionId,
        created: persisted.created,
      }
    }
    case 'duplicate': {
      const transitioned = await transition(durability, receipt, 'duplicate')
      if (!transitioned) return { outcome: 'in_flight', receipt }
      return {
        outcome: 'duplicate',
        receipt: transitioned,
        existingInteractionId: result.existingInteractionId,
      }
    }
    case 'resolution_required': {
      const transitioned = await transition(durability, receipt, 'resolution_required')
      if (!transitioned) return { outcome: 'in_flight', receipt }
      return { outcome: 'resolution_required', receipt: transitioned, reason: result.reason }
    }
    case 'excluded':
    case 'rejected': {
      // 'excluded' outcomes (internal_only, configured_system_endpoint) are
      // deliberately not person interactions — recorded as rejected on the
      // receipt with the reason preserved.
      const transitioned = await transition(durability, receipt, 'rejected')
      if (!transitioned) return { outcome: 'in_flight', receipt }
      return { outcome: 'rejected', receipt: transitioned, reason: result.reason }
    }
  }
}

function isTerminal(status: CalendarIntakeReceiptStatus): boolean {
  return (
    status === 'completed' ||
    status === 'rejected' ||
    status === 'resolution_required' ||
    status === 'duplicate'
  )
}

async function transition(
  durability: CalendarIntakeDurability,
  receipt: CalendarIntakeReceipt,
  to: CalendarTerminalReceiptStatus,
  interactionId?: string,
): Promise<CalendarIntakeReceipt | null> {
  const ok = await durability.transitionReceipt({
    receiptId: receipt.id,
    claimToken: receipt.processingStartedAt as string,
    from: 'processing',
    to,
    interactionId,
  })
  if (!ok) return null
  return {
    ...receipt,
    status: to,
    interactionId,
    processingStartedAt: null,
  }
}

export type CalendarSyncResult = {
  sourceSystem: string
  /** The cursor this sync started from. */
  cursor: string | null
  /** The cursor to advance to (recorded on newly seen receipts). */
  nextCursor: string | null
  outcomes: CalendarEventProcessingOutcome[]
}

export type SyncCalendarEventsInput = {
  provider: CalendarProvider
  configuration: CalendarAdapterConfiguration
  repositories: CalendarIntakeRepositories
  durability: CalendarIntakeDurability
  now?: () => string
}

/**
 * Poller path: read the current cursor, list events since it, lower each
 * event through processCalendarEvent (dedupe via the unique source identity),
 * and record the next cursor on every newly seen receipt. The canonical
 * interaction is persisted only for 'ready' events; everything else is
 * recorded on the receipt.
 */
export async function syncCalendarEvents(
  input: SyncCalendarEventsInput,
): Promise<CalendarSyncResult> {
  const { provider, configuration, repositories, durability } = input
  const sourceSystem = `calendar:${provider.name}:${provider.accountNamespace}`
  const cursor = await durability.readCurrentCursor(sourceSystem)
  const list = await provider.listEventsSince(cursor)
  const syncedAt = (input.now ?? (() => new Date().toISOString()))()

  const outcomes: CalendarEventProcessingOutcome[] = []
  for (const event of list.events) {
    // eslint-disable-next-line no-await-in-loop
    outcomes.push(
      await processCalendarEvent({
        event,
        configuration,
        repositories,
        durability,
        cursor: list.nextCursor,
        syncedAt,
      }),
    )
  }

  return { sourceSystem, cursor, nextCursor: list.nextCursor, outcomes }
}

export type HandleCalendarWebhookInput = SyncCalendarEventsInput & {
  provider: CalendarProvider
  payload: unknown
  signature: Record<string, string | string[] | undefined>
}

export type CalendarWebhookHandlingResult = {
  verification: CalendarWebhookVerification
  /** Present when the verified notification triggered a sync. */
  sync?: CalendarSyncResult
}

/**
 * Webhook path: verify the provider notification, then run a sync. The
 * initial channel 'sync' notification carries no data (no-op); 'exists',
 * 'updated', and 'deleted' notifications trigger a sync that re-lists since
 * the cursor — replays dedupe on the unique source identity.
 */
export async function handleCalendarWebhook(
  input: HandleCalendarWebhookInput,
): Promise<CalendarWebhookHandlingResult> {
  const { payload, signature, ...syncInput } = input
  const verification = await input.provider.verifyWebhook(payload, signature)
  if (verification.resourceState === 'sync') {
    return { verification }
  }
  const sync = await syncCalendarEvents(syncInput)
  return { verification, sync }
}
