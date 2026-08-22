// ---------------------------------------------------------------------------
// CRM-08 — Calendar Provider Seam: provider-neutral contracts.
//
// This module is the compile-ready contract surface of the live calendar
// intake boundary, shaped like the workflow command seam
// (lib/workflow/contracts.ts) and the DOC-03 signature seam
// (lib/signature/contracts.ts): pure types + constants, no provider imports,
// no SDK objects, no credentials.
//
// Boundaries (architect brief):
//   - The provider adapter (lib/calendar/google) implements CalendarProvider
//     and lowers RAW provider payloads into the neutral CalendarProviderEvent
//     (lib/crm-calendar-types.ts) — only normalized transport facts cross into
//     CRM. Provider SDK objects, credentials and tokens never do.
//   - The durable receipt/cursor boundary is calendar_intake_receipt
//     (migration 040): UNIQUE (source_system, source_external_id) idempotency
//     exactly like website_intake_submission. It records only the neutral
//     source identity + neutral outcome — never provider payloads or tokens.
//   - OAuth credentials live in env (lib/calendar/google/config.ts); the
//     short-lived access token lives in a provider-side token store
//     (lib/calendar/token-store.ts, migration 041) — NEVER in canonical CRM
//     tables.
//   - The poller/webhook lowering path (lib/calendar/lowering.ts) persists
//     the canonical interaction ONLY when the intake result is 'ready' and
//     records duplicate / rejected / resolution_required on the receipt, then
//     advances the cursor. No person is ever auto-created from a calendar
//     event (allowCreation stays false) and no task noise is derived.
// ---------------------------------------------------------------------------

import type { CalendarProviderEvent } from '../crm-calendar-types'

// ---------------------------------------------------------------------------
// Durable receipt/cursor model (calendar_intake_receipt, migration 040)
// ---------------------------------------------------------------------------

export const CALENDAR_INTAKE_RECEIPT_STATUSES = [
  'received',
  'processing',
  'completed',
  'rejected',
  'resolution_required',
  'duplicate',
] as const

export type CalendarIntakeReceiptStatus =
  (typeof CALENDAR_INTAKE_RECEIPT_STATUSES)[number]

export type CalendarIntakeReceipt = {
  id: string
  sourceSystem: string
  sourceExternalId: string
  status: CalendarIntakeReceiptStatus
  /** The canonical interaction this event produced (completed only). */
  interactionId?: string
  /** Provider cursor (updated-time or syncToken) current when first seen. */
  providerCursor: string | null
  /** When the event was last observed by a sync/webhook. */
  lastSyncedAt: string | null
  /** Claim token while status === 'processing'. */
  processingStartedAt: string | null
  createdAt: string
  updatedAt: string
}

// ---------------------------------------------------------------------------
// CalendarProvider interface (neutral surface; adapters implement it)
// ---------------------------------------------------------------------------

/**
 * One page of a provider sync: the neutral events to lower plus the cursor to
 * advance to. `nextCursor === null` means "keep the current cursor" (no
 * provider-observable change), which is safe because replays dedupe on the
 * unique source identity.
 */
export type CalendarListResult = {
  events: CalendarProviderEvent[]
  nextCursor: string | null
}

/** Neutral Google Calendar push-notification resource states. */
export const CALENDAR_WEBHOOK_RESOURCE_STATES = [
  'sync',
  'exists',
  'updated',
  'deleted',
] as const

export type CalendarWebhookResourceState =
  (typeof CALENDAR_WEBHOOK_RESOURCE_STATES)[number]

export type CalendarWebhookVerification = {
  verified: true
  resourceState: CalendarWebhookResourceState
  /** The provider channel id the notification arrived on. */
  channelId: string
}

/**
 * The neutral live calendar provider. A provider adapter (Google Calendar
 * behind this interface, like BoldSign behind SignatureProvider) owns all
 * OAuth + API specifics and lowers RAW provider payloads into
 * CalendarProviderEvent — only normalized transport facts cross into CRM.
 */
export interface CalendarProvider {
  /** Stable provider identifier (e.g. 'google'). */
  readonly name: string
  /** The configured business account namespace (source identity segment). */
  readonly accountNamespace: string

  /**
   * List neutral events changed since the cursor. `cursor === null` performs
   * the initial bounded lookback sync. The returned events are already
   * lowered to CalendarProviderEvent; raw provider payloads never cross.
   */
  listEventsSince(cursor: string | null): Promise<CalendarListResult>

  /** Fetch a single event by provider id, lowered, or null when unknown. */
  getEvent(id: string): Promise<CalendarProviderEvent | null>

  /**
   * Verify a provider webhook (payload = raw request body; signature = the
   * provider's verification headers exactly as received). Throws on any
   * failure — forged signatures, unknown channel tokens, malformed headers.
   * Returns a NEUTRAL resource state; provider-specific semantics are
   * confined to the adapter.
   */
  verifyWebhook(
    payload: unknown,
    signature: Record<string, string | string[] | undefined>,
  ): Promise<CalendarWebhookVerification>
}
