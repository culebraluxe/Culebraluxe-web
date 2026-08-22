// ---------------------------------------------------------------------------
// CRM-08 — Google Calendar adapter: configuration.
//
// All Google credentials/configuration come from config/env — NEVER hardcoded
// and NEVER logged. The required keys fail closed (missing or blank -> the
// adapter refuses to start); error messages name the KEYS, never the values.
// The short-lived OAuth access token is refreshed from these env credentials
// and cached in the provider-side token store (migration 041) — the secrets
// themselves are never written to any table.
//
// Env contract (also the Vercel production env contract):
//   GOOGLE_CLIENT_ID                  required — OAuth client id
//   GOOGLE_CLIENT_SECRET              required — OAuth client secret
//   GOOGLE_REFRESH_TOKEN              required — long-lived OAuth refresh token
//   GOOGLE_CALENDAR_ID                required — calendar to sync ('primary'
//                                     or a Google Calendar resource id/email)
//   GOOGLE_CALENDAR_ACCOUNT_NAMESPACE optional (default 'primary') — source
//                                     identity segment: calendar:google:<ns>
//   GOOGLE_CALENDAR_SYNC_MODE         optional (default 'updated_time') —
//                                     'updated_time' | 'sync_token'
//   GOOGLE_CALENDAR_WEBHOOK_CHANNEL_TOKEN optional — shared push channel
//                                     token; webhook verification fails closed
//                                     when unset
//   GOOGLE_CALENDAR_LOOKBACK_DAYS     optional (default 14) — initial lookback
//   GOOGLE_CALENDAR_TIMEOUT_MS        optional (default 10000)
//   GOOGLE_CALENDAR_MAX_ATTEMPTS      optional (default 3)
//   GOOGLE_CALENDAR_RETRY_BASE_DELAY_MS optional (default 150)
//   GOOGLE_CALENDAR_RETRY_MAX_DELAY_MS optional (default 1200)
//   GOOGLE_CALENDAR_TOKEN_ENDPOINT    optional — OAuth token endpoint
//                                     (default https://oauth2.googleapis.com/token)
//   GOOGLE_CALENDAR_API_BASE          optional — Calendar API v3 base URL
//                                     (default https://www.googleapis.com/calendar/v3)
// ---------------------------------------------------------------------------

export const GOOGLE_CALENDAR_CONFIG_KEYS = {
  clientId: 'GOOGLE_CLIENT_ID',
  clientSecret: 'GOOGLE_CLIENT_SECRET',
  refreshToken: 'GOOGLE_REFRESH_TOKEN',
  calendarId: 'GOOGLE_CALENDAR_ID',
  accountNamespace: 'GOOGLE_CALENDAR_ACCOUNT_NAMESPACE',
  syncMode: 'GOOGLE_CALENDAR_SYNC_MODE',
  webhookChannelToken: 'GOOGLE_CALENDAR_WEBHOOK_CHANNEL_TOKEN',
  lookbackDays: 'GOOGLE_CALENDAR_LOOKBACK_DAYS',
  timeoutMs: 'GOOGLE_CALENDAR_TIMEOUT_MS',
  maxAttempts: 'GOOGLE_CALENDAR_MAX_ATTEMPTS',
  retryBaseDelayMs: 'GOOGLE_CALENDAR_RETRY_BASE_DELAY_MS',
  retryMaxDelayMs: 'GOOGLE_CALENDAR_RETRY_MAX_DELAY_MS',
  tokenEndpoint: 'GOOGLE_CALENDAR_TOKEN_ENDPOINT',
  apiBaseUrl: 'GOOGLE_CALENDAR_API_BASE',
} as const

export type GoogleCalendarSyncMode = 'updated_time' | 'sync_token'

export type GoogleCalendarConfig = {
  clientId: string
  clientSecret: string
  refreshToken: string
  /** The calendar resource to sync (e.g. 'primary'). */
  calendarId: string
  /** Source identity segment: calendar:google:<accountNamespace>. */
  accountNamespace: string
  syncMode: GoogleCalendarSyncMode
  /** Shared push channel token; null fails webhook verification closed. */
  webhookChannelToken: string | null
  /** Initial full-sync lookback window in days. */
  lookbackDays: number
  /** Per-attempt HTTP timeout. */
  timeoutMs: number
  /** Cap on retry attempts for transient provider errors (capped backoff). */
  maxAttempts: number
  retryBaseDelayMs: number
  retryMaxDelayMs: number
  /** OAuth token endpoint (defaults to Google's; overridable for tests). */
  tokenEndpoint: string
  /** Calendar API v3 base URL (defaults to Google's; overridable for tests). */
  apiBaseUrl: string
}

export const GOOGLE_CALENDAR_DEFAULTS = {
  accountNamespace: 'primary',
  syncMode: 'updated_time',
  lookbackDays: 14,
  timeoutMs: 10_000,
  maxAttempts: 3,
  retryBaseDelayMs: 150,
  retryMaxDelayMs: 1_200,
  tokenEndpoint: 'https://oauth2.googleapis.com/token',
  apiBaseUrl: 'https://www.googleapis.com/calendar/v3',
} as const

function positiveInt(raw: string | undefined, fallback: number, key: string): number {
  if (raw === undefined || raw.trim() === '') return fallback
  const value = Number.parseInt(raw.trim(), 10)
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(
      `Google Calendar config ${key} must be a positive integer; got ${JSON.stringify(raw)}.`,
    )
  }
  return value
}

/**
 * Load the Google Calendar adapter configuration from the process environment.
 * FAIL CLOSED: every required key must be present and non-blank; the thrown
 * error lists only the missing key NAMES — values are never echoed.
 */
export function loadGoogleCalendarConfig(
  env: NodeJS.ProcessEnv = process.env,
): GoogleCalendarConfig {
  const required: Array<[string, string | undefined]> = [
    [GOOGLE_CALENDAR_CONFIG_KEYS.clientId, env.GOOGLE_CLIENT_ID],
    [GOOGLE_CALENDAR_CONFIG_KEYS.clientSecret, env.GOOGLE_CLIENT_SECRET],
    [GOOGLE_CALENDAR_CONFIG_KEYS.refreshToken, env.GOOGLE_REFRESH_TOKEN],
    [GOOGLE_CALENDAR_CONFIG_KEYS.calendarId, env.GOOGLE_CALENDAR_ID],
  ]
  const missing = required
    .filter(([, value]) => value === undefined || value.trim() === '')
    .map(([key]) => key)
  if (missing.length > 0) {
    throw new Error(
      `Google Calendar config is incomplete; set the required env keys: ${missing.join(', ')}.`,
    )
  }

  const syncModeRaw = (env.GOOGLE_CALENDAR_SYNC_MODE ?? '')
    .trim()
    .toLowerCase()
  const syncMode: GoogleCalendarSyncMode =
    syncModeRaw === '' ? GOOGLE_CALENDAR_DEFAULTS.syncMode : (syncModeRaw as GoogleCalendarSyncMode)
  if (syncMode !== 'updated_time' && syncMode !== 'sync_token') {
    throw new Error(
      `Google Calendar config ${GOOGLE_CALENDAR_CONFIG_KEYS.syncMode} must be 'updated_time' or 'sync_token'.`,
    )
  }

  const webhookChannelToken = env.GOOGLE_CALENDAR_WEBHOOK_CHANNEL_TOKEN
  return {
    clientId: (env.GOOGLE_CLIENT_ID as string).trim(),
    clientSecret: env.GOOGLE_CLIENT_SECRET as string,
    refreshToken: env.GOOGLE_REFRESH_TOKEN as string,
    calendarId: (env.GOOGLE_CALENDAR_ID as string).trim(),
    accountNamespace: (
      env.GOOGLE_CALENDAR_ACCOUNT_NAMESPACE ??
      GOOGLE_CALENDAR_DEFAULTS.accountNamespace
    )
      .trim()
      .toLowerCase(),
    syncMode,
    webhookChannelToken:
      webhookChannelToken && webhookChannelToken.trim() !== ''
        ? webhookChannelToken
        : null,
    lookbackDays: positiveInt(
      env.GOOGLE_CALENDAR_LOOKBACK_DAYS,
      GOOGLE_CALENDAR_DEFAULTS.lookbackDays,
      GOOGLE_CALENDAR_CONFIG_KEYS.lookbackDays,
    ),
    timeoutMs: positiveInt(
      env.GOOGLE_CALENDAR_TIMEOUT_MS,
      GOOGLE_CALENDAR_DEFAULTS.timeoutMs,
      GOOGLE_CALENDAR_CONFIG_KEYS.timeoutMs,
    ),
    maxAttempts: positiveInt(
      env.GOOGLE_CALENDAR_MAX_ATTEMPTS,
      GOOGLE_CALENDAR_DEFAULTS.maxAttempts,
      GOOGLE_CALENDAR_CONFIG_KEYS.maxAttempts,
    ),
    retryBaseDelayMs: positiveInt(
      env.GOOGLE_CALENDAR_RETRY_BASE_DELAY_MS,
      GOOGLE_CALENDAR_DEFAULTS.retryBaseDelayMs,
      GOOGLE_CALENDAR_CONFIG_KEYS.retryBaseDelayMs,
    ),
    retryMaxDelayMs: positiveInt(
      env.GOOGLE_CALENDAR_RETRY_MAX_DELAY_MS,
      GOOGLE_CALENDAR_DEFAULTS.retryMaxDelayMs,
      GOOGLE_CALENDAR_CONFIG_KEYS.retryMaxDelayMs,
    ),
    tokenEndpoint: (
      env.GOOGLE_CALENDAR_TOKEN_ENDPOINT ?? GOOGLE_CALENDAR_DEFAULTS.tokenEndpoint
    ).trim().replace(/\/+$/, ''),
    apiBaseUrl: (
      env.GOOGLE_CALENDAR_API_BASE ?? GOOGLE_CALENDAR_DEFAULTS.apiBaseUrl
    ).trim().replace(/\/+$/, ''),
  }
}
