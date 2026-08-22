// ---------------------------------------------------------------------------
// CRM-08 — Google Calendar adapter: CalendarProvider implementation.
//
// The Google Calendar connector BEHIND the neutral CalendarProvider seam
// (lib/calendar/contracts.ts), like BoldSign behind SignatureProvider. It
// owns ALL Google specifics — OAuth (config.ts + token-store.ts), the typed
// API client (client.ts), and the push-notification verification
// (webhook.ts) — and lowers RAW Google payloads into the neutral
// CalendarProviderEvent (lib/crm-calendar-types.ts). Only normalized
// transport facts cross into CRM; provider SDK objects, credentials, and
// tokens never do.
//
// Cursor semantics:
//   - updated_time mode (default): the cursor is the RFC3339 updated-time
//     watermark. The first sync uses a bounded lookback (timeMin); each sync
//     advances to max(updated)+1ms so the next poll is exclusive.
//   - sync_token mode: the cursor is Google's opaque nextSyncToken; the first
//     sync uses the bounded lookback, subsequent syncs pass the token.
//   - cancelled events are dropped at the lowering boundary (they are not
//     appointments to intake).
// ---------------------------------------------------------------------------

import type { CalendarProvider } from '../contracts'
import type {
  CalendarListResult,
  CalendarWebhookVerification,
} from '../contracts'
import type { CalendarProviderEvent } from '../../crm-calendar-types'
import type { GoogleCalendarConfig } from './config'
import type {
  GoogleCalendarClient,
  GoogleCalendarRawEvent,
} from './client'
import { verifyGoogleCalendarWebhook } from './webhook'

function unique(values: string[]): string[] {
  return [...new Set(values)]
}

function nextUpdatedCursor(
  items: GoogleCalendarRawEvent[],
  fallback: string | null,
): string | null {
  let maxUpdated: Date | null = null
  for (const item of items) {
    if (typeof item?.updated !== 'string') continue
    const parsed = new Date(item.updated)
    if (!Number.isNaN(parsed.getTime()) && (!maxUpdated || parsed > maxUpdated)) {
      maxUpdated = parsed
    }
  }
  if (!maxUpdated) return fallback
  // +1ms makes the next poll exclusive of this watermark.
  return new Date(maxUpdated.getTime() + 1).toISOString()
}

function uniqueEmail(value: string | undefined): string | null {
  if (typeof value !== 'string') return null
  const normalized = value.trim()
  return normalized ? normalized : null
}

/**
 * Lower a RAW Google Calendar event into the neutral CalendarProviderEvent.
 * Returns null when the event carries no intakable appointment fact
 * (cancelled, missing id, missing start time, no attendee/organizer email).
 * This is the ONLY place Google wire shapes become CRM transport facts.
 */
export function lowerGoogleCalendarEvent(
  raw: GoogleCalendarRawEvent,
  accountNamespace: string,
): CalendarProviderEvent | null {
  if (typeof raw?.id !== 'string' || raw.id.trim() === '') return null
  if (raw.status === 'cancelled') return null

  const start = raw.start?.dateTime ?? raw.start?.date ?? raw.updated
  if (typeof start !== 'string' || Number.isNaN(new Date(start).getTime())) {
    return null
  }

  const attendeeEmails = unique(
    (raw.attendees ?? [])
      .map((attendee) => uniqueEmail(attendee?.email))
      .filter((email): email is string => email !== null),
  )
  const organizerEmail = uniqueEmail(raw.organizer?.email)
  const allEmails =
    attendeeEmails.length > 0
      ? attendeeEmails
      : organizerEmail
        ? [organizerEmail]
        : []
  if (allEmails.length === 0) return null

  // self === true means the event lives on the authenticated owned calendar;
  // anything else (including an absent organizer) is never assumed owned.
  const organizer = raw.organizer?.self === true ? 'owned' : 'external'

  return {
    provider: 'google',
    accountNamespace,
    providerEventId: raw.id,
    occurredAt: start,
    organizer,
    attendees: allEmails.map((value) => ({ kind: 'email', value })),
    actorAssurance: 'transport_observed',
    title: typeof raw.summary === 'string' ? raw.summary : undefined,
    description:
      typeof raw.description === 'string' ? raw.description : undefined,
  }
}

export class GoogleCalendarProvider implements CalendarProvider {
  readonly name = 'google'

  constructor(
    private readonly config: GoogleCalendarConfig,
    private readonly client: GoogleCalendarClient,
  ) {}

  get accountNamespace(): string {
    return this.config.accountNamespace
  }

  async listEventsSince(cursor: string | null): Promise<CalendarListResult> {
    const lookbackStart = new Date(
      Date.now() - this.config.lookbackDays * 24 * 60 * 60 * 1000,
    ).toISOString()

    let page
    if (this.config.syncMode === 'sync_token') {
      page = await this.client.listEvents(
        cursor
          ? { syncToken: cursor, maxResults: 250 }
          : { timeMin: lookbackStart, maxResults: 250 },
      )
    } else {
      page = await this.client.listEvents(
        cursor
          ? { updatedMin: cursor, maxResults: 250 }
          : { timeMin: lookbackStart, maxResults: 250, orderBy: 'updated' },
      )
    }

    const events = (page.items ?? []).flatMap((raw) => {
      const lowered = lowerGoogleCalendarEvent(raw, this.config.accountNamespace)
      return lowered ? [lowered] : []
    })

    const nextCursor =
      this.config.syncMode === 'sync_token'
        ? (page.nextSyncToken ?? cursor)
        : nextUpdatedCursor(page.items ?? [], cursor)

    return { events, nextCursor }
  }

  async getEvent(id: string): Promise<CalendarProviderEvent | null> {
    const raw = await this.client.getEvent(id)
    return raw ? lowerGoogleCalendarEvent(raw, this.config.accountNamespace) : null
  }

  async verifyWebhook(
    payload: unknown,
    signature: Record<string, string | string[] | undefined>,
  ): Promise<CalendarWebhookVerification> {
    return verifyGoogleCalendarWebhook(
      payload,
      signature,
      this.config.webhookChannelToken,
    )
  }
}
