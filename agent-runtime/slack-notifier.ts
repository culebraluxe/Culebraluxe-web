// ---------------------------------------------------------------------------
// ENG-FORGE-V4-11 — optional Slack run notifications (human cockpit mirror).
//
// Slack is a human cockpit only; Forge/Neon own truth. This notifier sits
// OUTSIDE orchestration semantics: it observes lifecycle outcomes that are
// already durable in Neon and mirrors them. It must never become a dependency
// of claim, execution, commit, Assay, or story completion, so delivery is
// strictly one-way and strictly fail-open:
//
//   - FORGE_SLACK_WEBHOOK_URL unset        -> no request is ever made
//   - malformed URL / timeout / network error / non-2xx / invalid response
//     -> returns false, never throws, never retries, never touches Forge state
//
// Payloads are one concise, phone-readable plain-text message per event. They
// carry only durable identifiers (story id, work item id, role/profile,
// external run id, commit SHA, outcome) plus bounded, redacted context. The
// context type deliberately has NO field for free-form model output, prompts,
// or transcripts, so a transcript cannot leak by accident; free-text detail is
// single-lined, truncated, and secret-redacted before it may appear.
//
// No schema change and no new dependency: enabling/disabling is the single
// FORGE_SLACK_WEBHOOK_URL variable and delivery uses the platform's native
// fetch (Node >= 18).
// ---------------------------------------------------------------------------

/** Single opt-in environment variable. Absent/empty => Slack is disabled. */
export const FORGE_SLACK_WEBHOOK_URL = 'FORGE_SLACK_WEBHOOK_URL'

/**
 * Structural environment map the notifier reads. Deliberately not
 * `NodeJS.ProcessEnv` (whose global declaration makes NODE_ENV a required
 * member): the notifier must accept any plain env map, including test maps
 * with only FORGE_SLACK_WEBHOOK_URL set.
 */
export type SlackEnv = Record<string, string | undefined>

/** Upper bound on one webhook delivery attempt (fail-open after this). */
export const SLACK_DELIVERY_TIMEOUT_MS = 5_000

/** Upper bound for free-text detail carried into a message. */
const DETAIL_MAX_LENGTH = 200

/** Lifecycle outcomes the local Forge worker mirrors to Slack. */
export type ForgeSlackEvent =
  | 'lane-started'
  | 'lane-completed'
  | 'lane-follow'
  | 'lane-terminal'

/**
 * Everything a message may need. Only durable identifiers and a bounded,
 * redacted outcome — never secrets, database URLs, provider keys, prompts,
 * or model transcripts. There is intentionally no field that accepts free-form
 * model output.
 */
export type ForgeSlackContext = {
  event: ForgeSlackEvent
  /** Canonical story id (e.g. ENG-FORGE-V4-11). Always included. */
  storyId: string
  /** Durable agent work item id. Always included. */
  workItemId: string
  /** Human story title when available. */
  storyTitle?: string | null
  /** Logical role: builder | verifier | reviewer | scout | ... */
  role?: string | null
  /** Logical model profile (builder-flash, verifier-mini, ...). */
  modelProfile?: string | null
  /** Runtime adapter id (deepseek-harness, gateway-*, ...). */
  runtimeAdapter?: string | null
  /** Opaque external run / session correlation id when known. */
  externalRunId?: string | null
  /** Full candidate commit SHA (40 hex) when the lane produced one. */
  commitHash?: string | null
  /** Terminal outcome text: Complete | Assay Failed | Failed | Cancelled ... */
  resultStatus?: string | null
  /** Run completion 0-100 when known. */
  completion?: number | null
  /** Lane the story now follows to (lane-follow): smith | assay | scout. */
  toLane?: string | null
  /** Concise one-line failure context (terminal only). Bounded + redacted. */
  detail?: string | null
}

const PLAYER_LABELS: Record<string, string> = {
  // logical roles
  builder: 'Smith',
  verifier: 'Assay',
  reviewer: 'Assay',
  scout: 'Scout',
  architect: 'Architect',
  // lane ids resolve through the same human vocabulary
  smith: 'Smith',
  assay: 'Assay',
}

/**
 * Human, phone-readable label for a role or a lane id: builder -> Smith,
 * verifier/reviewer/assay -> Assay, scout -> Scout, smith -> Smith,
 * architect -> Architect; anything else is capitalized verbatim.
 */
export function forgePlayerLabel(
  roleOrLane: string | null | undefined,
): string | null {
  const key = (roleOrLane ?? '').trim().toLowerCase()
  if (!key) return null
  const known = PLAYER_LABELS[key]
  if (known) return known
  return key.charAt(0).toUpperCase() + key.slice(1)
}

/** Resolve the webhook URL from the environment, or null when disabled. */
export function forgeSlackWebhookUrl(
  env: SlackEnv = process.env,
): string | null {
  const raw = env?.[FORGE_SLACK_WEBHOOK_URL]
  const url = (raw ?? '').trim()
  return url || null
}

/** True when a webhook URL is configured (empty/whitespace counts as off). */
export function isSlackConfigured(
  env: SlackEnv = process.env,
): boolean {
  return forgeSlackWebhookUrl(env) !== null
}

/** Collapse any value to one trimmed single line, capped at `max` chars. */
function singleLine(
  value: string | null | undefined,
  max: number,
): string | null {
  const line = (value ?? '').replace(/\s+/g, ' ').trim()
  if (!line) return null
  return line.length > max ? `${line.slice(0, max - 1)}…` : line
}

/**
 * Redact anything that looks like a secret before it may reach Slack. Applied
 * to the only free-text field (detail) so provider keys, tokens, and database
 * URLs cannot leak even when a failure excerpt mentions them.
 */
const SECRET_PATTERNS: RegExp[] = [
  /\bhttps?:\/\/[^\s]+/gi,
  /\bpostgres(?:ql)?:\/\/[^\s]+/gi,
  /\b(?:sk|rk)-[A-Za-z0-9_-]{12,}/g,
  /\bxox[baprs]-[A-Za-z0-9-]{10,}/g,
  /\bAIza[0-9A-Za-z_-]{15,}/g,
  /\bgh[pousr]_[A-Za-z0-9]{20,}/g,
  /\bAKIA[0-9A-Z]{16}\b/g,
  /\bbearer\s+[A-Za-z0-9._~+/=-]+/gi,
  /\b(?:api[_-]?key|token|secret|password|authorization)\b\s*[:=]\s*[^\s]+/gi,
]

/** Redact known secret shapes from free text. Never throws. */
export function redactForSlack(
  text: string | null | undefined,
): string | null {
  const value = singleLine(text, DETAIL_MAX_LENGTH * 4)
  if (!value) return null
  let redacted = value
  for (const pattern of SECRET_PATTERNS) {
    redacted = redacted.replace(pattern, '[redacted]')
  }
  return redacted
}

/** Safe terminal detail: single line, redacted, hard-truncated. */
function safeDetail(text: string | null | undefined): string | null {
  const redacted = redactForSlack(text)
  if (!redacted) return null
  return redacted.length > DETAIL_MAX_LENGTH
    ? `${redacted.slice(0, DETAIL_MAX_LENGTH - 1)}…`
    : redacted
}

function pushLine(
  lines: string[],
  label: string,
  value: string | null | undefined,
  max: number,
): void {
  const clean = singleLine(value, max)
  if (clean) lines.push(`${label} ${clean}`)
}

/**
 * Build the concise plain-text message body for one event, or null when the
 * message cannot be correlated (missing story id or work item id). The text
 * always carries both durable identifiers and never includes model output.
 */
export function buildSlackMessage(
  context: ForgeSlackContext,
): string | null {
  const storyId = singleLine(context.storyId, 80)
  const workItemId = singleLine(context.workItemId, 80)
  if (!storyId || !workItemId) return null

  const player = forgePlayerLabel(context.role) ?? 'a lane'
  const profile = singleLine(context.modelProfile, 60)
  const attribution = profile ? `${player} (${profile})` : player
  const title = singleLine(context.storyTitle, 140)
  const titleLine = title ? `${storyId} — ${title}` : storyId
  const idsLine = `story ${storyId} | work item ${workItemId}`
  const result = singleLine(context.resultStatus, 60)
  const completion =
    typeof context.completion === 'number' &&
    Number.isFinite(context.completion)
      ? `${Math.round(context.completion)}%`
      : null

  const lines: string[] = []
  switch (context.event) {
    case 'lane-started':
      lines.push(`▶️ Forge · ${player} started`)
      lines.push(titleLine, idsLine)
      lines.push(attribution)
      break
    case 'lane-completed':
      lines.push(`✅ Forge · ${player} completed`)
      lines.push(titleLine, idsLine)
      lines.push(attribution)
      if (result) lines.push(`result ${result}${completion ? ` · ${completion}` : ''}`)
      pushLine(lines, 'commit', context.commitHash, 48)
      pushLine(lines, 'external run', context.externalRunId, 160)
      break
    case 'lane-follow': {
      const toLane = forgePlayerLabel(context.toLane) ?? 'the next lane'
      lines.push(`🔀 Forge · follows to ${toLane}`)
      lines.push(titleLine, idsLine)
      lines.push(`${attribution} → ${toLane}`)
      break
    }
    case 'lane-terminal': {
      lines.push(`⛔ Forge · ${player} terminal`)
      lines.push(titleLine, idsLine)
      lines.push(attribution)
      if (result) {
        lines.push(`outcome ${result}${completion ? ` · ${completion}` : ''}`)
      }
      const detail = safeDetail(context.detail)
      if (detail) lines.push(detail)
      break
    }
  }
  return lines.join('\n')
}

/** Minimal structural POST shape; native fetch and test fakes both satisfy it. */
export type SlackWebhookResponse = { ok: boolean; status: number }

export type SlackWebhookPost = (
  url: string,
  init: {
    method: string
    headers: Record<string, string>
    body: string
    signal?: AbortSignal
  },
) => Promise<SlackWebhookResponse>

export type SlackNotificationDeps = {
  /** Environment override (tests inject a fixed map). Defaults to process.env. */
  env?: SlackEnv
  /** POST implementation. Defaults to the platform's native fetch. */
  post?: SlackWebhookPost
  /** Delivery timeout in ms. Defaults to SLACK_DELIVERY_TIMEOUT_MS. */
  timeoutMs?: number
  /** Failure logger. Defaults to console.warn. Never throws. */
  log?: (message: string) => void
}

function warn(deps: SlackNotificationDeps, message: string): void {
  const log = deps.log ?? ((line: string) => console.warn(line))
  try {
    log(message)
  } catch {
    // Logging is also best-effort; it must never throw either.
  }
}

/**
 * Mirror one Forge lifecycle outcome to Slack.
 *
 * Fail-open contract: resolves `true` only when a message was posted and the
 * webhook answered 2xx. Resolves `false` — and NEVER rejects — when Slack is
 * unconfigured, the message cannot be built, the URL is malformed/non-https,
 * the POST times out, the network fails, or the webhook answers non-2xx. No
 * retry and no Forge state change are ever performed.
 */
export async function postSlackNotification(
  context: ForgeSlackContext,
  deps: SlackNotificationDeps = {},
): Promise<boolean> {
  try {
    const env = deps.env ?? process.env
    const webhook = forgeSlackWebhookUrl(env)
    if (!webhook) return false
    if (!/^https:\/\//i.test(webhook)) {
      warn(
        deps,
        'forge slack notify skipped: FORGE_SLACK_WEBHOOK_URL is not an https URL (fail-open, no request made).',
      )
      return false
    }
    const text = buildSlackMessage(context)
    if (!text) return false

    const post: SlackWebhookPost =
      deps.post ?? (globalThis.fetch as unknown as SlackWebhookPost)
    const timeoutMs = deps.timeoutMs ?? SLACK_DELIVERY_TIMEOUT_MS
    const signal =
      typeof AbortSignal !== 'undefined' &&
      typeof AbortSignal.timeout === 'function'
        ? AbortSignal.timeout(timeoutMs)
        : undefined
    const response = await post(webhook, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ text }),
      ...(signal ? { signal } : {}),
    })
    if (!response || response.ok !== true) {
      warn(
        deps,
        `forge slack notify skipped: webhook answered ${response?.status ?? 'invalid response'} (fail-open).`,
      )
      return false
    }
    return true
  } catch (error) {
    const reason = singleLine((error as Error)?.message, 140) ?? 'delivery failed'
    warn(deps, `forge slack notify skipped: ${reason} (fail-open, no retry).`)
    return false
  }
}
