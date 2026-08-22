// ---------------------------------------------------------------------------
// CRM-08 — Calendar provider token store.
//
// The CalendarProvider seam's provider-side credential cache. Static OAuth
// credentials (client id / client secret / refresh token) live in env
// (lib/calendar/google/config.ts); the SHORT-LIVED access token the adapter
// refreshes is cached HERE — never in canonical CRM tables.
//
// The interface is neutral (one namespace per business account). Implementations:
//   - InMemoryCalendarTokenStore  — single-process/tests (this module);
//   - createPostgresCalendarTokenStore (db/google-calendar-token.ts) — the
//     durable provider-side store over google_calendar_token_store
//     (migration 041), isolated from interaction/person/task.
// ---------------------------------------------------------------------------

export type CalendarAccessToken = {
  accessToken: string
  expiresAt: Date
}

export interface CalendarTokenStore {
  /** The cached access token for the account, or null when absent/expired. */
  getAccessToken(accountNamespace: string): Promise<CalendarAccessToken | null>
  /** Cache a refreshed access token for the account. */
  setAccessToken(
    accountNamespace: string,
    accessToken: string,
    expiresAt: Date,
  ): Promise<void>
}

/**
 * Single-process / test token store. Expired tokens are dropped on read so a
 * stale token can never be presented to the provider.
 */
export class InMemoryCalendarTokenStore implements CalendarTokenStore {
  private readonly tokens = new Map<
    string,
    { accessToken: string; expiresAt: Date }
  >()

  constructor(private readonly now: () => Date = () => new Date()) {}

  async getAccessToken(
    accountNamespace: string,
  ): Promise<CalendarAccessToken | null> {
    const cached = this.tokens.get(accountNamespace)
    if (!cached) return null
    if (cached.expiresAt.getTime() <= this.now().getTime()) {
      this.tokens.delete(accountNamespace)
      return null
    }
    return { accessToken: cached.accessToken, expiresAt: cached.expiresAt }
  }

  async setAccessToken(
    accountNamespace: string,
    accessToken: string,
    expiresAt: Date,
  ): Promise<void> {
    this.tokens.set(accountNamespace, { accessToken, expiresAt })
  }
}
