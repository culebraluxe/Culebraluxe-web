// FIELD MEDIATOR — the failsafe at the write boundary (captain, 2026-09-16).
//
// Neon is the wire format. A model may emit any shape it likes; this is the one piece of code that turns a
// value into a row, and it is the ONLY writer. It exists because telling a lane the rules is not enough — the
// rules have to be a failsafe, not a request.
//
// IT MAY (mechanical and lossless): strip code fences, backticks and stray quotes; trim; accept `key: value`,
// `key = value`, or a bare token; case-fold into a DECLARED closed set; apply a DECLARED alias table; coerce
// declared boolean and numeric spellings; take free text for a DESCRIPTIVE field.
//
// IT MAY NOT — the whole safety property: infer a value; supply one that is missing; choose between two
// candidates; "repair" a malformed handoff; or default a DECISION field (a decision with a default is a
// decision nobody made). Any of those makes this a SECOND WRITER, which AGENTS.md (Never) forbids outright.
//
// On refusal it returns the field, the accepted set and the reason. The caller re-asks ONCE with the accepted
// set spelled out (`describeRefusal`), then HOLDs. A mediator that cannot decide HOLDs; it never guesses.

export type FieldKind = 'closed' | 'text' | 'number' | 'boolean' | 'sha'

export type FieldDeclaration = {
  field: string
  kind: FieldKind
  /** The closed set, for kind 'closed'. Members are matched case-insensitively. */
  accepted?: readonly string[]
  /** Declared synonyms only. An alias that is not declared is NOT a synonym — it is a refusal. */
  aliases?: Readonly<Record<string, string>>
  maxLength?: number
  /** True when this value IS a decision: a missing value is then never defaulted, ever. */
  decision?: boolean
  /** Only honoured when `decision` is not set, and only when the value is genuinely absent. */
  default?: string | number | boolean
  /** Free text may be mined for the value (descriptive fields only; never for a decision). */
  allowProse?: boolean
}

export type FieldReason =
  | 'EMPTY'
  | 'DECISION_MISSING'
  | 'NOT_IN_SET'
  | 'AMBIGUOUS'
  | 'NOT_A_NUMBER'
  | 'NOT_A_BOOLEAN'
  | 'NOT_A_SHA'
  | 'TOO_LONG'

export type FieldSuccess = { ok: true; value: string | number | boolean; source: 'raw' | 'default' }
export type FieldRefusal = {
  ok: false
  field: string
  accepted: readonly string[]
  reason: FieldReason
  raw: string
}
export type FieldMediation = FieldSuccess | FieldRefusal

/** Mechanical shape tolerance only: fences, backticks, quotes, collapsed whitespace. No meaning is touched. */
export function normalizeRaw(raw: unknown): string {
  if (typeof raw !== 'string') return ''
  return raw
    .replace(/```[a-zA-Z0-9_-]*/g, ' ')
    .replace(/[`"']/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
}

/** The value as stated: `field: value` or `field = value` when keyed, otherwise the whole normalized text. */
function statedValue(raw: string, field: string): string {
  const keyed = new RegExp(`(?:^|\\s)${field}\\s*[:=]\\s*([^,;]+)`, 'i').exec(raw)
  return (keyed?.[1] ?? raw).trim()
}

function refuse(d: FieldDeclaration, reason: FieldReason, raw: string): FieldRefusal {
  return { ok: false, field: d.field, accepted: d.accepted ?? [], reason, raw }
}

/**
 * Mediate ONE value. Pure: no clock, no database, no model — every input is an argument, so the tests can pin
 * every branch without a fixture.
 */
export function mediateField(declaration: FieldDeclaration, raw: unknown): FieldMediation {
  const d = declaration
  const text = normalizeRaw(raw)
  const accepted = d.accepted ?? []

  // ABSENCE IS NOT A VALUE. A decision field is refused rather than defaulted; anything else may take a
  // DECLARED default, and the success says so, so nothing downstream mistakes a default for a statement.
  if (text.length === 0) {
    if (d.decision) return refuse(d, 'DECISION_MISSING', '')
    if (d.default !== undefined) return { ok: true, value: d.default, source: 'default' }
    return refuse(d, 'EMPTY', '')
  }

  if (d.kind === 'text') {
    if (d.maxLength !== undefined && text.length > d.maxLength) return refuse(d, 'TOO_LONG', text)
    return { ok: true, value: text, source: 'raw' }
  }

  if (d.kind === 'number') {
    const n = Number(statedValue(text, d.field))
    if (!Number.isFinite(n)) return refuse(d, 'NOT_A_NUMBER', text)
    return { ok: true, value: n, source: 'raw' }
  }

  if (d.kind === 'boolean') {
    const v = statedValue(text, d.field).toLowerCase()
    if (v === 'true' || v === 'yes' || v === '1') return { ok: true, value: true, source: 'raw' }
    if (v === 'false' || v === 'no' || v === '0') return { ok: true, value: false, source: 'raw' }
    return refuse(d, 'NOT_A_BOOLEAN', text)
  }

  if (d.kind === 'sha') {
    const v = statedValue(text, d.field)
    // SHA-1 is 40 characters and SHA-256 is 64, so the window covers both: a rule that refused a value the
    // old normalizers accepted would be a regression dressed as consistency.
    if (!/^[0-9a-f]{7,64}$/i.test(v)) return refuse(d, 'NOT_A_SHA', text)
    return { ok: true, value: v.toLowerCase(), source: 'raw' }
  }

  // kind === 'closed' — the decision path. Exact match, then declared aliases, and nothing else.
  const table = new Map<string, string>()
  for (const value of accepted) {
    table.set(value.toLowerCase(), value)
    table.set(value.replace(/[^a-z0-9]/gi, '').toLowerCase(), value)
  }
  for (const [alias, target] of Object.entries(d.aliases ?? {})) table.set(alias.toLowerCase(), target)

  const stated = statedValue(text, d.field).toLowerCase()
  const exact = table.get(stated)
  if (exact !== undefined) return { ok: true, value: exact, source: 'raw' }

  // A value found inside prose is acceptable only for a DESCRIPTIVE closed field, and only when the text
  // offers exactly ONE candidate. Two candidates is a choice, and this code does not make choices.
  if (d.allowProse && !d.decision) {
    const tokens = text
      .toLowerCase()
      .split(/[^a-z0-9_]+/)
      .filter(Boolean)
    const hits = new Set<string>()
    for (const token of tokens) {
      const hit = table.get(token)
      if (hit !== undefined) hits.add(hit)
    }
    if (hits.size > 1) return refuse(d, 'AMBIGUOUS', text)
    if (hits.size === 1) return { ok: true, value: [...hits][0], source: 'raw' }
  }

  return refuse(d, 'NOT_IN_SET', text)
}

/** The ONE retry sentence the caller may send back, with the accepted set spelled out, before it HOLDs. */
export function describeRefusal(refusal: FieldRefusal): string {
  const accepted = refusal.accepted.length > 0 ? ` accepted: ${refusal.accepted.join(' | ')}` : ''
  const got = refusal.raw.length > 0 ? ` — got ${JSON.stringify(refusal.raw.slice(0, 80))}` : ''
  return `${refusal.field}: ${refusal.reason}${accepted}${got}`
}

// ---------------------------------------------------------------------------
// SHARED FIELD DECLARATIONS
//
// A field declared in the module that governs it, imported by every lane that carries it. Two call sites
// writing their own regex for the same value is how two opinions start (2026-09-16: the Assay pin judged a
// candidate sha with one rule and the role mapping with another).
// ---------------------------------------------------------------------------

/** The candidate sha every lane publishes and QA verifies. */
export const CANDIDATE_SHA: FieldDeclaration = { field: 'candidateSha', kind: 'sha', decision: true }

/** The Lead's routing decision — the one value that costs nights when it is wrong. */
export const LEAD_DECISION: FieldDeclaration = {
  field: 'leadDecision',
  kind: 'closed',
  accepted: ['SMITH', 'SPLIT', 'HOLD', 'SOLO'],
  aliases: { single: 'SOLO' },
  decision: true,
}

/** The Lead's size call: a closed set, never a free adjective. */
export const LEAD_SIZE: FieldDeclaration = {
  field: 'leadSize',
  kind: 'closed',
  accepted: ['TRIVIAL', 'SMALL', 'MEDIUM', 'LARGE'],
  aliases: { tiny: 'TRIVIAL', big: 'LARGE' },
  decision: true,
}
