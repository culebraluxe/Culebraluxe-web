// ---------------------------------------------------------------------------
// DOC-04 — BoldSign Integration: configuration.
//
// All BoldSign credentials/configuration come from config/env — NEVER
// hardcoded and NEVER logged. The four required keys fail closed (missing or
// blank -> the adapter refuses to start); error messages name the KEYS, never
// the values. Tuning knobs (timeouts/retries/webhook tolerance) have explicit
// defaults so a deploy only needs the four required values.
//
// Env contract (also the Vercel production env contract):
//   BOLDSIGN_API_KEY            required — BoldSign API key (X-API-KEY)
//   BOLDSIGN_BASE_URL           required — e.g. https://api.boldsign.com
//   BOLDSIGN_WEBHOOK_SECRET     required — webhook HMAC signing secret
//   BOLDSIGN_TEMPLATE_ID        OPTIONAL (legacy) — no longer required. The
//                               canonical send path uploads the existing PDF
//                               bytes CulebraLuxe already owns via
//                               POST /v1/document/send, so a BoldSign template
//                               is never needed.
//   BOLDSIGN_TIMEOUT_MS         optional (default 10000) per-attempt timeout
//   BOLDSIGN_MAX_ATTEMPTS       optional (default 3) capped retry attempts
//   BOLDSIGN_RETRY_BASE_DELAY_MS optional (default 150) backoff base
//   BOLDSIGN_RETRY_MAX_DELAY_MS optional (default 1200) backoff cap
//   BOLDSIGN_WEBHOOK_TOLERANCE_SECONDS optional (default 300) signature t skew
// ---------------------------------------------------------------------------

export const BOLD_SIGN_CONFIG_KEYS = {
  apiKey: 'BOLDSIGN_API_KEY',
  baseUrl: 'BOLDSIGN_BASE_URL',
  templateId: 'BOLDSIGN_TEMPLATE_ID',
  webhookSecret: 'BOLDSIGN_WEBHOOK_SECRET',
  timeoutMs: 'BOLDSIGN_TIMEOUT_MS',
  maxAttempts: 'BOLDSIGN_MAX_ATTEMPTS',
  retryBaseDelayMs: 'BOLDSIGN_RETRY_BASE_DELAY_MS',
  retryMaxDelayMs: 'BOLDSIGN_RETRY_MAX_DELAY_MS',
  webhookToleranceSeconds: 'BOLDSIGN_WEBHOOK_TOLERANCE_SECONDS',
} as const

export type BoldSignConfig = {
  apiKey: string
  baseUrl: string
  /**
   * Legacy template id. Kept for type/config compatibility but NOT required and
   * NOT used by the canonical send path (which uploads the PDF directly).
   * Defaults to an empty string when BOLDSIGN_TEMPLATE_ID is absent.
   */
  templateId: string
  webhookSecret: string
  /** Per-attempt HTTP timeout. */
  timeoutMs: number
  /** Cap on retry attempts for transient provider errors (capped backoff). */
  maxAttempts: number
  retryBaseDelayMs: number
  retryMaxDelayMs: number
  /** Max accepted skew between the webhook signature timestamp and now. */
  webhookToleranceSeconds: number
}

export const BOLD_SIGN_DEFAULTS = {
  timeoutMs: 10_000,
  maxAttempts: 3,
  retryBaseDelayMs: 150,
  retryMaxDelayMs: 1_200,
  webhookToleranceSeconds: 300,
} as const

function positiveInt(raw: string | undefined, fallback: number, key: string): number {
  if (raw === undefined || raw.trim() === '') return fallback
  const value = Number.parseInt(raw.trim(), 10)
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`BoldSign config ${key} must be a positive integer; got ${JSON.stringify(raw)}.`)
  }
  return value
}

/**
 * Load the BoldSign adapter configuration from the process environment.
 * FAIL CLOSED: every required key must be present and non-blank; the thrown
 * error lists only the missing key NAMES — values are never echoed.
 */
export function loadBoldSignConfig(env: NodeJS.ProcessEnv = process.env): BoldSignConfig {
  const required: Array<[string, string | undefined]> = [
    [BOLD_SIGN_CONFIG_KEYS.apiKey, env.BOLDSIGN_API_KEY],
    [BOLD_SIGN_CONFIG_KEYS.baseUrl, env.BOLDSIGN_BASE_URL],
    [BOLD_SIGN_CONFIG_KEYS.webhookSecret, env.BOLDSIGN_WEBHOOK_SECRET],
  ]
  const missing = required
    .filter(([, value]) => value === undefined || value.trim() === '')
    .map(([key]) => key)
  if (missing.length > 0) {
    throw new Error(
      `BoldSign config is incomplete; set the required env keys: ${missing.join(', ')}.`,
    )
  }
  return {
    apiKey: (env.BOLDSIGN_API_KEY as string).trim(),
    baseUrl: (env.BOLDSIGN_BASE_URL as string).trim().replace(/\/+$/, ''),
    templateId: (env.BOLDSIGN_TEMPLATE_ID ?? '').trim(),
    webhookSecret: env.BOLDSIGN_WEBHOOK_SECRET as string,
    timeoutMs: positiveInt(env.BOLDSIGN_TIMEOUT_MS, BOLD_SIGN_DEFAULTS.timeoutMs, BOLD_SIGN_CONFIG_KEYS.timeoutMs),
    maxAttempts: positiveInt(env.BOLDSIGN_MAX_ATTEMPTS, BOLD_SIGN_DEFAULTS.maxAttempts, BOLD_SIGN_CONFIG_KEYS.maxAttempts),
    retryBaseDelayMs: positiveInt(env.BOLDSIGN_RETRY_BASE_DELAY_MS, BOLD_SIGN_DEFAULTS.retryBaseDelayMs, BOLD_SIGN_CONFIG_KEYS.retryBaseDelayMs),
    retryMaxDelayMs: positiveInt(env.BOLDSIGN_RETRY_MAX_DELAY_MS, BOLD_SIGN_DEFAULTS.retryMaxDelayMs, BOLD_SIGN_CONFIG_KEYS.retryMaxDelayMs),
    webhookToleranceSeconds: positiveInt(
      env.BOLDSIGN_WEBHOOK_TOLERANCE_SECONDS,
      BOLD_SIGN_DEFAULTS.webhookToleranceSeconds,
      BOLD_SIGN_CONFIG_KEYS.webhookToleranceSeconds,
    ),
  }
}
