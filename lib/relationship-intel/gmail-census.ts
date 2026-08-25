// ---------------------------------------------------------------------------
// REL-INTEL — bounded Gmail relationship-census importer.
//
// Parses Marlowe's partial Gmail relationship-census artifact (one row per
// normalized external email correspondent) into the source-neutral evidence
// shape. This is a BOUNDED batch, not a full-mailbox census: coverage bounds
// and limitations are preserved in `coverageNote` and batch accounting.
//
// The artifact contains aggregate correspondent evidence only — no message
// bodies, attachments, or snippets. This parser refuses body/snippet content
// and quarantines malformed rows rather than silently dropping them.
// ---------------------------------------------------------------------------

import type { IdentityEvidence, RelationshipEvidence } from './contracts'
import { fingerprint, normalizeEmail, sanitizeSpreadsheetCell } from './normalize'

export interface ParsedGmailEvidence {
  evidence: RelationshipEvidence
  fingerprint: string
}

export type GmailRowResult =
  | { ok: true; row: ParsedGmailEvidence }
  | { ok: false; reason: string; line: number; raw: string }

export interface GmailBatchAccounting {
  declared: number
  accepted: number
  rejected: number
  quarantined: number
  deduplicated: number
  rows: ParsedGmailEvidence[]
}

// Minimal CSV line parser that honours double-quoted fields (display-name
// candidates can contain commas).
export function parseCsvLine(line: string): string[] {
  const out: string[] = []
  let cur = ''
  let inQuotes = false
  for (let i = 0; i < line.length; i++) {
    const ch = line[i]
    if (inQuotes) {
      if (ch === '"') {
        if (line[i + 1] === '"') {
          cur += '"'
          i++
        } else {
          inQuotes = false
        }
      } else {
        cur += ch
      }
    } else if (ch === '"') {
      inQuotes = true
    } else if (ch === ',') {
      out.push(cur)
      cur = ''
    } else {
      cur += ch
    }
  }
  out.push(cur)
  return out
}

function toInt(value: string | undefined): number | null {
  const trimmed = (value ?? '').trim()
  if (!trimmed) return null
  const n = Number(trimmed)
  return Number.isFinite(n) ? n : null
}

function toBool(value: string | undefined): boolean | null {
  const trimmed = (value ?? '').trim().toLowerCase()
  if (trimmed === '') return null
  if (trimmed === 'true' || trimmed === '1') return true
  if (trimmed === 'false' || trimmed === '0') return false
  return null
}

const GMAIL_SOURCE = 'gmail_contacts' as const

export function parseGmailCensusRow(raw: string, line: number): GmailRowResult {
  const cols = parseCsvLine(raw)

  const normalizedEmail = (cols[0] ?? '').trim()
  const emailCheck = normalizeEmail(normalizedEmail)
  if (!emailCheck.ok) {
    return { ok: false, reason: emailCheck.reason, line, raw }
  }

  const displayNameRaw = sanitizeSpreadsheetCell(cols[1] ?? '')
  const displayName = displayNameRaw || null

  // Source pointers: never reveal credentials; keep the opaque account token.
  const sourceAccount = sanitizeSpreadsheetCell(cols[3] ?? '').trim() || 'unknown'

  const coverageNote = (cols[27] ?? '').trim() || null

  const evidence: RelationshipEvidence = {
    source: GMAIL_SOURCE,
    sourceAccount,
    sourceIdentityKey: emailCheck.value,
    sourceLabel: null,
    displayName,
    organization: null,
    emails: [{ value: normalizedEmail, normalized: emailCheck.value, label: null }],
    phones: [],
    firstObservedAt: (cols[4] ?? '').trim() || null,
    lastObservedAt: (cols[5] ?? '').trim() || null,
    lastInboundAt: (cols[7] ?? '').trim() || null,
    lastOutboundAt: (cols[9] ?? '').trim() || null,
    inboundCount: toInt(cols[10]),
    outboundCount: toInt(cols[11]),
    isTwoWay: toBool(cols[18]),
    isOwnerInitiated: toBool(cols[19]),
    isAutomatedOrBulk: toBool(cols[20]) ?? toBool(cols[21]) ?? false,
    isOrganizationOrService: toBool(cols[22]),
    knownAppleContact: false,
    hasEmail: true,
    hasPhone: false,
    coverageNote,
  }

  // Build a deterministic fingerprint over the neutral evidence so exact
  // replays are distinguishable from changed payloads (replay safety).
  const fp = fingerprint(
    JSON.stringify({
      source: evidence.source,
      sourceAccount: evidence.sourceAccount,
      sourceIdentityKey: evidence.sourceIdentityKey,
      displayName: evidence.displayName,
      lastInboundAt: evidence.lastInboundAt,
      lastOutboundAt: evidence.lastOutboundAt,
      inboundCount: evidence.inboundCount,
      outboundCount: evidence.outboundCount,
      isTwoWay: evidence.isTwoWay,
      isAutomatedOrBulk: evidence.isAutomatedOrBulk,
      coverageNote: evidence.coverageNote,
    }),
  )

  return { ok: true, row: { evidence, fingerprint: fp } }
}

/**
 * Parse a full bounded Gmail census CSV (header row excluded) into accepted,
 * rejected, and quarantined buckets. The batch balances:
 *   declared = accepted + rejected + quarantined
 * and deduplication is reported separately (first occurrence wins).
 */
export function parseGmailCensus(csv: string): GmailBatchAccounting {
  const lines = csv.replace(/\r/g, '').split('\n').filter((l) => l.trim().length > 0)
  // Drop the header row if it starts with the known first column.
  const body = lines[0]?.toLowerCase().startsWith('normalized_email')
    ? lines.slice(1)
    : lines

  const accepted: ParsedGmailEvidence[] = []
  const rejected: { line: number; reason: string; raw: string }[] = []
  const quarantined: { line: number; reason: string; raw: string }[] = []
  const seen = new Set<string>()
  let deduplicated = 0

  body.forEach((raw, i) => {
    const line = i + (lines.length !== body.length ? 1 : 0) + 1
    const result = parseGmailCensusRow(raw, line)
    if (!result.ok) {
      if (
        result.reason === 'missing_email' ||
        result.reason === 'invalid_format' ||
        result.reason === 'empty' ||
        result.reason === 'too_long'
      ) {
        rejected.push({ line, reason: result.reason, raw })
      } else {
        quarantined.push({ line, reason: result.reason, raw })
      }
      return
    }
    const key = `${result.row.evidence.sourceAccount}\u0000${result.row.evidence.sourceIdentityKey}`
    if (seen.has(key)) {
      deduplicated += 1
      return
    }
    seen.add(key)
    accepted.push(result.row)
  })

  return {
    declared: body.length,
    accepted: accepted.length,
    rejected: rejected.length,
    quarantined: quarantined.length,
    deduplicated,
    rows: accepted,
  }
}

