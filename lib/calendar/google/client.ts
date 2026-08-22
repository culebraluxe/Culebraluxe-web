// ---------------------------------------------------------------------------
// CRM-08 — Google Calendar adapter: typed Google Calendar API v3 client.
//
// A thin typed wrapper over the Google Calendar REST API v3
// (developers.google.com/calendar/api/v3/reference). The adapter talks to
// THIS client, never to Google directly; all Google wire shapes are confined
// here (and to the webhook header parser). The client:
//   - refreshes the short-lived OAuth access token from env credentials
//     (client id/secret/refresh token) via the token endpoint and caches it
//     in the provider-side token store (lib/calendar/token-store.ts) — the
//     secret material is never written to canonical tables and never logged;
//   - authenticates with `Authorization: Bearer <access_token>`;
//   - enforces a per-attempt timeout (AbortController);
//   - retries only TRANSIENT failures (network/timeout/408/429/5xx) with
//     capped exponential backoff — no retry storms; non-transient failures
//     fail immediately;
//   - follows nextPageToken (bounded) so a sync never silently drops a page.
//
// Endpoints used:
//   POST https://oauth2.googleapis.com/token      (OAuth refresh_token grant)
//   GET  https://www.googleapis.com/calendar/v3/calendars/{calendarId}/events
//   GET  https://www.googleapis.com/calendar/v3/calendars/{calendarId}/events/{id}
// ---------------------------------------------------------------------------

import type { GoogleCalendarConfig } from './config'
import type { CalendarTokenStore } from '../token-store'
import {
  GoogleCalendarProviderError,
  classifyGoogleCalendarError,
  isTransientHttpStatus,
} from './errors'

/** RAW Google Calendar event payload (confined to this module + adapter). */
export type GoogleCalendarRawAttendee = {
  email?: string
  displayName?: string
  organizer?: boolean
  self?: boolean
  responseStatus?: string
  resource?: boolean
}

export type GoogleCalendarRawEvent = {
  id?: string
  status?: string
  summary?: string
  description?: string
  start?: { dateTime?: string; date?: string; timeZone?: string }
  end?: { dateTime?: string; date?: string; timeZone?: string }
  updated?: string
  organizer?: { email?: string; self?: boolean; displayName?: string }
  creator?: { email?: string; self?: boolean }
  attendees?: GoogleCalendarRawAttendee[]
  iCalUID?: string
  hangoutLink?: string
  extendedProperties?: unknown
}

export type GoogleCalendarListParams = {
  updatedMin?: string
  timeMin?: string
  syncToken?: string
  maxResults?: number
  orderBy?: string
  pageToken?: string
}

export type GoogleCalendarListResponse = {
  items: GoogleCalendarRawEvent[]
  nextSyncToken: string | null
}

export type GoogleCalendarClientDeps = {
  tokenStore: CalendarTokenStore
  fetchFn?: typeof fetch
  sleep?: (ms: number) => Promise<void>
  now?: () => Date
}

/** Cap on list pages followed per sync (maxResults 250/page). */
const MAX_LIST_PAGES = 20

export class GoogleCalendarClient {
  constructor(
    private readonly config: GoogleCalendarConfig,
    private readonly deps: GoogleCalendarClientDeps,
  ) {}

  private now(): Date {
    return this.deps.now ? this.deps.now() : new Date()
  }

  // -- OAuth ----------------------------------------------------------------

  /**
   * The current access token for the configured account. Uses the cached
   * provider-side token when it is still valid (refreshed 60s before expiry);
   * otherwise refreshes via the OAuth token endpoint and caches the result.
   */
  async accessToken(): Promise<string> {
    const namespace = this.config.accountNamespace
    const cached = await this.deps.tokenStore.getAccessToken(namespace)
    if (cached && cached.expiresAt.getTime() > this.now().getTime() + 60_000) {
      return cached.accessToken
    }
    const refreshed = await this.refreshAccessToken()
    await this.deps.tokenStore.setAccessToken(
      namespace,
      refreshed.accessToken,
      refreshed.expiresAt,
    )
    return refreshed.accessToken
  }

  private async refreshAccessToken(): Promise<{
    accessToken: string
    expiresAt: Date
  }> {
    const body = new URLSearchParams({
      client_id: this.config.clientId,
      client_secret: this.config.clientSecret,
      refresh_token: this.config.refreshToken,
      grant_type: 'refresh_token',
    })
    const parsed = await this.requestWithRetry<{ access_token?: unknown; expires_in?: unknown }>({
      method: 'POST',
      url: this.config.tokenEndpoint,
      headers: {
        accept: 'application/json',
        'content-type': 'application/x-www-form-urlencoded',
      },
      body: body.toString(),
      parse: (json) => json as { access_token?: unknown; expires_in?: unknown },
    })
    if (
      typeof parsed?.access_token !== 'string' ||
      parsed.access_token.trim() === ''
    ) {
      throw new Error('Google OAuth token response is missing access_token.')
    }
    const expiresIn =
      typeof parsed.expires_in === 'number' && Number.isFinite(parsed.expires_in)
        ? parsed.expires_in
        : 3600
    return {
      accessToken: parsed.access_token,
      expiresAt: new Date(this.now().getTime() + expiresIn * 1000),
    }
  }

  // -- Calendar API ---------------------------------------------------------

  /** List events with bounded pagination; merged items + the sync token. */
  async listEvents(
    params: GoogleCalendarListParams,
  ): Promise<GoogleCalendarListResponse> {
    const items: GoogleCalendarRawEvent[] = []
    let nextSyncToken: string | null = null
    let pageToken: string | undefined = params.pageToken
    let pages = 0

    while (pages < MAX_LIST_PAGES) {
      const query = new URLSearchParams()
      if (params.updatedMin) query.set('updatedMin', params.updatedMin)
      if (params.timeMin) query.set('timeMin', params.timeMin)
      if (params.syncToken) query.set('syncToken', params.syncToken)
      if (params.orderBy) query.set('orderBy', params.orderBy)
      if (params.maxResults) query.set('maxResults', String(params.maxResults))
      if (pageToken) query.set('pageToken', pageToken)

      const page = await this.requestWithRetry<{
        items?: unknown
        nextSyncToken?: unknown
        nextPageToken?: unknown
      }>({
        method: 'GET',
        url: `${this.config.apiBaseUrl}/calendars/${encodeURIComponent(this.config.calendarId)}/events?${query.toString()}`,
        parse: (json) => json as {
          items?: unknown
          nextSyncToken?: unknown
          nextPageToken?: unknown
        },
      })

      if (Array.isArray(page.items)) {
        items.push(...(page.items as GoogleCalendarRawEvent[]))
      }
      if (typeof page.nextSyncToken === 'string' && nextSyncToken === null) {
        nextSyncToken = page.nextSyncToken
      }
      if (typeof page.nextPageToken !== 'string') break
      pageToken = page.nextPageToken
      pages += 1
    }

    return { items, nextSyncToken }
  }

  /** Fetch one event by provider id. Returns null on a 404. */
  async getEvent(eventId: string): Promise<GoogleCalendarRawEvent | null> {
    try {
      return await this.requestWithRetry<GoogleCalendarRawEvent>({
        method: 'GET',
        url: `${this.config.apiBaseUrl}/calendars/${encodeURIComponent(this.config.calendarId)}/events/${encodeURIComponent(eventId)}`,
        parse: (json) => json as GoogleCalendarRawEvent,
      })
    } catch (err) {
      if (
        err instanceof GoogleCalendarProviderError &&
        err.status === 404
      ) {
        return null
      }
      throw err
    }
  }

  // -- transport ------------------------------------------------------------

  private async requestWithRetry<T>(opts: {
    method: string
    url: string
    headers?: Record<string, string>
    body?: string
    parse: (json: unknown) => T
  }): Promise<T> {
    const maxAttempts = Math.max(1, this.config.maxAttempts)
    let lastError: unknown
    for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
      try {
        return await this.requestOnce(opts)
      } catch (err) {
        const classified = classifyGoogleCalendarError(err)
        if (!classified.retryable || attempt >= maxAttempts) {
          throw err
        }
        lastError = err
        const delay = Math.min(
          this.config.retryBaseDelayMs * 2 ** (attempt - 1),
          this.config.retryMaxDelayMs,
        )
        if (this.deps.sleep) {
          // eslint-disable-next-line no-await-in-loop
          await this.deps.sleep(delay)
        } else {
          // eslint-disable-next-line no-await-in-loop
          await new Promise((resolve) => setTimeout(resolve, delay))
        }
      }
    }
    throw lastError
  }

  private async requestOnce<T>(opts: {
    method: string
    url: string
    headers?: Record<string, string>
    body?: string
    parse: (json: unknown) => T
  }): Promise<T> {
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), this.config.timeoutMs)
    const fetchFn = this.deps.fetchFn ?? fetch
    try {
      const headers: Record<string, string> = {
        accept: 'application/json',
        ...opts.headers,
      }
      // Calendar API calls carry the Bearer token; the OAuth token endpoint
      // call (no opts.headers.authorization) uses its own credentials body.
      if (!headers.authorization && opts.url.startsWith(this.config.apiBaseUrl)) {
        headers.authorization = `Bearer ${await this.accessToken()}`
      }

      const response = await fetchFn(opts.url, {
        method: opts.method,
        headers,
        body: opts.body,
        signal: controller.signal,
      })
      const raw = await response.text()
      if (!response.ok) {
        const excerpt = raw.length > 200 ? `${raw.slice(0, 200)}…` : raw
        throw new GoogleCalendarProviderError(
          `Google Calendar API ${opts.method} ${opts.url} failed with HTTP ${response.status}: ${excerpt}`,
          response.status,
          isTransientHttpStatus(response.status),
        )
      }
      if (raw.trim() === '') {
        throw new Error(`Google Calendar API ${opts.method} ${opts.url} returned an empty body.`)
      }
      let json: unknown
      try {
        json = JSON.parse(raw)
      } catch {
        throw new Error(`Google Calendar API ${opts.method} ${opts.url} returned non-JSON content.`)
      }
      return opts.parse(json)
    } catch (err) {
      if (err instanceof Error && err.name === 'AbortError') {
        const timeoutErr = new Error(
          `Google Calendar API ${opts.method} ${opts.url} timed out after ${this.config.timeoutMs}ms.`,
        )
        timeoutErr.name = 'TimeoutError'
        throw timeoutErr
      }
      throw err
    } finally {
      clearTimeout(timeout)
    }
  }
}
