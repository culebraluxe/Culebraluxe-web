import type { QueryExecutor } from './query-executor'
import type {
  CalendarAccessToken,
  CalendarTokenStore,
} from '../lib/calendar/token-store'

// ---------------------------------------------------------------------------
// Google Calendar provider token store (migration 041, CRM-08).
//
// PROVIDER-SIDE persistence, BEHIND the CalendarProvider seam. This is the
// durable home of the short-lived OAuth ACCESS token the Google Calendar
// adapter refreshes from env credentials (GOOGLE_CLIENT_ID / CLIENT_SECRET /
// REFRESH_TOKEN — the long-lived secrets are NEVER written to the database).
// It is NOT canonical CRM data: it is isolated from interaction/person/task
// and never referenced by CRM code.
//
// These functions never call the provider — the adapter
// (lib/calendar/google) composes OAuth refreshes with these writes.
// ---------------------------------------------------------------------------

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

/** Durable provider-side token store over google_calendar_token_store. */
export function createPostgresCalendarTokenStore(
  execute?: QueryExecutor,
): CalendarTokenStore {
  const q = execute ?? null
  return {
    async getAccessToken(
      accountNamespace: string,
    ): Promise<CalendarAccessToken | null> {
      const run = q ?? (await executor())
      const rows = await run`
        select access_token, access_token_expires_at
        from google_calendar_token_store
        where account_namespace = ${accountNamespace}
        limit 1
      `
      const row = rows[0] as
        | { access_token: string; access_token_expires_at: string }
        | undefined
      if (!row) return null
      return {
        accessToken: row.access_token,
        expiresAt: new Date(row.access_token_expires_at),
      }
    },
    async setAccessToken(
      accountNamespace: string,
      accessToken: string,
      expiresAt: Date,
    ): Promise<void> {
      const run = q ?? (await executor())
      await run`
        insert into google_calendar_token_store (
          account_namespace, access_token, access_token_expires_at
        ) values (
          ${accountNamespace}, ${accessToken}, ${expiresAt.toISOString()}
        )
        on conflict (account_namespace) do update
          set access_token = ${accessToken},
              access_token_expires_at = ${expiresAt.toISOString()},
              updated_at = now()
      `
    },
  }
}
