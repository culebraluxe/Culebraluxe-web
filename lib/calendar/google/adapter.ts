// ---------------------------------------------------------------------------
// CRM-08 — Google Calendar adapter: CalendarProvider implementation.
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
  return new Date(maxUpdated.getTime() + 1).toISOString()
}

function uniqueEmail(value: string | undefined): string | null {
  if (typeof value !== 'string') return null
  const normalized = value.trim()
  return normalized ? normalized : null
}

export function lowerGoogleCalendarEvent(
  raw: GoogleCalendarRawEvent,
  accountNamespace: string,
): CalendarProviderEvent | null {
  if (typeof raw?.id !== 'string' || raw.id.trim() === '') return null
  if (raw.status === 'cancelled') return null

  const start = raw.start?.dateTime ?? raw.start?.date ?? raw.updated
  if (typeof start !== 'string' || Number.isNaN(new Date(start).getTime())) return null

  const attendeeEmails = unique(
    (raw.attendees ?? [])
      .map((attendee) => uniqueEmail(attendee?.email))
      .filter((email): email is string => email !== null),
  )
  const organizerEmail = uniqueEmail(raw.organizer?.email)
  const allEmails = attendeeEmails.length > 0 ? attendeeEmails : organizerEmail ? [organizerEmail] : []
  if (allEmails.length === 0) return null

  return {
    provider: 'google',
    accountNamespace,
    providerEventId: raw.id,
    occurredAt: start,
    organizer: raw.organizer?.self === true ? 'owned' : 'external',
    attendees: allEmails.map((value) => ({ kind: 'email', value })),
    actorAssurance: 'transport_observed',
    title: typeof raw.summary === 'string' ? raw.summary : undefined,
    description: typeof raw.description === 'string' ? raw.description : undefined,
  }
}

export type GoogleCalendarProviderDeps = {
  now?: () => Date
}

export class GoogleCalendarProvider implements CalendarProvider {
  readonly name = 'google'

  constructor(
    private readonly config: GoogleCalendarConfig,
    private readonly client: GoogleCalendarClient,
    private readonly deps: GoogleCalendarProviderDeps = {},
  ) {}

  get accountNamespace(): string {
    return this.config.accountNamespace
  }

  private now(): Date {
    return this.deps.now ? this.deps.now() : new Date()
  }

  async listEventsSince(cursor: string | null): Promise<CalendarListResult> {
    const lookbackStart = new Date(
      this.now().getTime() - this.config.lookbackDays * 24 * 60 * 60 * 1000,
    ).toISOString()

    const page = this.config.syncMode === 'sync_token'
      ? await this.client.listEvents(
          cursor ? { syncToken: cursor, maxResults: 250 } : { timeMin: lookbackStart, maxResults: 250 },
        )
      : await this.client.listEvents(
          cursor
            ? { updatedMin: cursor, maxResults: 250 }
            : { timeMin: lookbackStart, maxResults: 250, orderBy: 'updated' },
        )

    const events = (page.items ?? []).flatMap((raw) => {
      const lowered = lowerGoogleCalendarEvent(raw, this.config.accountNamespace)
      return lowered ? [lowered] : []
    })

    const nextCursor = this.config.syncMode === 'sync_token'
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
    return verifyGoogleCalendarWebhook(payload, signature, this.config.webhookChannelToken)
  }
}
