// ---------------------------------------------------------------------------
// ENG-FORGE-SHAPE-01 — judgment layer between Architect discovery and Smith
// execution.
//
// Architect may discover unlimited work, but discovery must NOT silently
// enlarge the current executable Smith contract. This module is the small,
// explicit shaping contract that turns a set of Architect findings into a
// bounded execution decision (WorkShapeDecision) with explicit dispositions:
//
//   SAME_UNIT        -> coalesces into one Smith unit
//   SPLIT_CHILD      -> an explicitly independent bounded child unit
//   FOLLOW_UP_STORY  -> a valid adjacent finding that becomes a Story Board
//                       follow-up, NEVER part of the current Smith assignment
//   NOTE             -> informational only
//   HOLD             -> required work too ambiguous/large to execute safely
//
// Lead independently reassesses execution size/topology even when Architect
// emitted one packet: shapeArchitectFindings() ignores packet count and re-derives
// the shape from required seams + explicit SPLIT_CHILD declarations.
//
// Pure + DB-free. Deterministic.
// ---------------------------------------------------------------------------

export type FindingDisposition = 'SAME_UNIT' | 'SPLIT_CHILD' | 'FOLLOW_UP_STORY' | 'NOTE' | 'HOLD'

export type ArchitectFinding = {
  id: string
  summary: string
  /** Required to satisfy the ORIGINAL story. Adjacent-but-valid findings set false. */
  required: boolean
  /** File/domain seams this finding owns (prefix semantics, './' tolerated). */
  seams: string[]
  /** Architect advisory only. Lead reassesses from seams + topology. */
  hint?: FindingDisposition
}

export type ShapeUnit = {
  id: string
  /** Finding ids this bounded unit owns. Never includes follow-up/NOTE/HOLD findings. */
  findingIds: string[]
  /** Union of member seams; the mutation-authority scope for THIS unit only. */
  seams: string[]
  /** Coalesced objective for the bounded Smith contract. */
  objective: string
}

export type FollowUpStory = {
  findingIds: string[]
  summary: string
}

export type ShapingNote = {
  findingIds: string[]
  summary: string
}

export type WorkShapeMode = 'SINGLE' | 'SPLIT' | 'HOLD'

export type WorkShapeDecision = {
  mode: WorkShapeMode
  units: ShapeUnit[]
  followUps: FollowUpStory[]
  notes: ShapingNote[]
  holds: string[]
  reason: string
}

export type LeadShapePlan = {
  /** Gate-fact decision: SINGLE maps to SMITH (one bounded unit). */
  decision: 'SMITH' | 'SPLIT' | 'HOLD'
  splitCount: number | null
  unitsCount: number
}

const STRUCTURED_PREFIX = 'FORGE_FINDINGS_JSON:'

function normalizeSeam(path: string): string {
  return path.trim().replace(/^\.\//, '').replace(/\/+$/, '')
}

function normalizeSeams(seams: unknown): string[] {
  if (!Array.isArray(seams)) return []
  return seams
    .filter((s): s is string => typeof s === 'string' && s.trim().length > 0)
    .map(normalizeSeam)
}

/** One seam overlaps another when they are equal or one is a directory/file prefix. */
function seamsOverlap(a: string[], b: string[]): boolean {
  const na = a.map(normalizeSeam)
  const nb = b.map(normalizeSeam)
  return na.some((x) => nb.some((y) => x === y || x.startsWith(`${y}/`) || y.startsWith(`${x}/`)))
}

function unionMembers(seams: string[]): string[] {
  const out: string[] = []
  for (const s of seams) if (!out.includes(s)) out.push(s)
  return out
}

function summaryFor(ids: string[], byId: Map<string, ArchitectFinding>): string {
  return ids.map((id) => byId.get(id)?.summary ?? id).join('; ')
}

// -- Disposition parser (mirrors FORGE_EVIDENCE_JSON marker style) ------------

export function parseForgeFindings(raw: unknown): ArchitectFinding[] {
  if (!Array.isArray(raw)) return []
  const findings: ArchitectFinding[] = []
  for (const row of raw) {
    if (!row || typeof row !== 'object') continue
    const f = row as Record<string, unknown>
    const id = String(f.id ?? '').trim()
    const summary = String(f.summary ?? '').trim()
    if (!id || !summary) continue
    const required = f.required === true
    const hint = String(f.hint ?? '').toUpperCase()
    findings.push({
      id,
      summary,
      required,
      seams: normalizeSeams(f.seams),
      hint: (['SAME_UNIT', 'SPLIT_CHILD', 'FOLLOW_UP_STORY', 'NOTE', 'HOLD'] as const).includes(
        hint as FindingDisposition,
      )
        ? (hint as FindingDisposition)
        : undefined,
    })
  }
  return findings
}

/**
 * Read Architect findings from the evidence marker in notes/testsSummary text.
 *
 * The marker is emitted by a MODEL, so the payload arrives in whatever shape the
 * model chose: inline, after a newline, inside a ```json fence, or spread over many
 * lines. An earlier implementation required `FORGE_FINDINGS_JSON:` to be followed
 * immediately by `[...]` and to end at a line end, so a fenced or multiline payload
 * silently produced ZERO findings — which starves the Lead's routing context (the
 * validator then refuses every non-HOLD proposal: "No required findings supplied")
 * and blocks the story at the human gate. Observed live 2026-09-10.
 *
 * Now: locate the marker, skip whitespace/fence, then scan for the matching close
 * bracket (string-aware) and parse exactly that slice.
 */
export function findingsFromArchitectEvidence(text: string | null | undefined): ArchitectFinding[] {
  if (!text) return []
  // Try every occurrence, LAST first: the model's real emission is at the end, and
  // an echoed instruction line must not shadow it. The first payload that parses wins.
  const positions: number[] = []
  for (let at = text.indexOf(STRUCTURED_PREFIX); at >= 0; at = text.indexOf(STRUCTURED_PREFIX, at + 1)) {
    positions.push(at)
  }
  for (let i = positions.length - 1; i >= 0; i--) {
    const slice = extractJsonArray(text.slice(positions[i] + STRUCTURED_PREFIX.length))
    if (!slice) continue
    try {
      const parsed = parseForgeFindings(JSON.parse(slice))
      if (parsed.length > 0) return parsed
    } catch {
      /* try an earlier occurrence */
    }
  }
  return []
}

/** First balanced `[...]` in `text`, string-aware. Null when there is no array. */
function extractJsonArray(text: string): string | null {
  const start = text.indexOf('[')
  if (start < 0) return null
  let depth = 0
  let inString = false
  let escaped = false
  for (let i = start; i < text.length; i++) {
    const ch = text[i]
    if (inString) {
      if (escaped) escaped = false
      else if (ch === '\\') escaped = true
      else if (ch === '"') inString = false
      continue
    }
    if (ch === '"') inString = true
    else if (ch === '[') depth++
    else if (ch === ']') {
      depth--
      if (depth === 0) return text.slice(start, i + 1)
    }
  }
  return null
}

export function findingsMarker(findings: ArchitectFinding[]): string {
  return `${STRUCTURED_PREFIX} ${JSON.stringify(findings)}`
}
// -- Shaping decision ---------------------------------------------------------

function groupUnits(executable: ArchitectFinding[], byId: Map<string, ArchitectFinding>): ShapeUnit[] {
  const explicitSplit = executable.filter((f) => f.hint === 'SPLIT_CHILD')
  const coalescible = executable.filter((f) => f.hint !== 'SPLIT_CHILD')

  // Union-find over coalescible findings by seam overlap.
  const parent = new Map<number, number>()
  const find = (i: number): number => {
    let root = i
    while (parent.get(root) !== undefined && parent.get(root) !== root) root = parent.get(root)!
    let cur = i
    while (parent.get(cur) !== undefined && parent.get(cur) !== root) {
      const next = parent.get(cur)!
      parent.set(cur, root)
      cur = next
    }
    return root
  }
  const union = (a: number, b: number): void => {
    const ra = find(a)
    const rb = find(b)
    if (ra !== rb) parent.set(rb, ra)
  }
  coalescible.forEach((_, i) => parent.set(i, i))
  for (let i = 0; i < coalescible.length; i++) {
    for (let j = i + 1; j < coalescible.length; j++) {
      if (seamsOverlap(coalescible[i].seams, coalescible[j].seams)) union(i, j)
    }
  }

  const groups = new Map<number, number[]>()
  coalescible.forEach((_, i) => {
    const root = find(i)
    const list = groups.get(root) ?? []
    list.push(i)
    groups.set(root, list)
  })

  const units: ShapeUnit[] = [...groups.values()].map((members) => {
    const ids = members.map((i) => coalescible[i].id).sort()
    const all = members.map((i) => coalescible[i])
    return {
      id: `unit_${ids.join('+')}`,
      findingIds: ids,
      seams: unionMembers(all.flatMap((f) => f.seams)),
      objective: summaryFor(ids, byId),
    } satisfies ShapeUnit
  })

  // Explicitly-declared SPLIT_CHILD findings are ALWAYS their own bounded unit.
  for (const f of explicitSplit) {
    units.push({
      id: `unit_${f.id}`,
      findingIds: [f.id],
      seams: [...f.seams],
      objective: f.summary,
    })
  }

  // Deterministic order: coalesced groups first (input order), then explicit children.
  return units
}

/**
 * Lead's independent reassessment. Adjacent findings become follow-up stories;
 * informational findings become notes; HOLD-required work stops execution; the
 * remaining required findings coalesce by seam into bounded units (SINGLE when
 * they form exactly one unit, SPLIT when independent required seams remain).
 */
export function shapeArchitectFindings(input: { findings: ArchitectFinding[] }): WorkShapeDecision {
  const findings = input.findings ?? []
  const byId = new Map(findings.map((f) => [f.id, f]))
  const required = findings.filter((f) => f.required)
  const adjacent = findings.filter((f) => !f.required)

  const holds = required.filter((f) => f.hint === 'HOLD').map((f) => f.id)
  const notes = adjacent
    .filter((f) => f.hint === 'NOTE')
    .map((f) => ({ findingIds: [f.id], summary: f.summary } satisfies ShapingNote))
  const followUps = adjacent
    .filter((f) => f.hint !== 'NOTE')
    .map((f) => ({ findingIds: [f.id], summary: f.summary } satisfies FollowUpStory))

  const executable = required.filter((f) => f.hint !== 'HOLD')
  const units = groupUnits(executable, byId)

  let mode: WorkShapeMode = 'HOLD'
  let reason: string
  if (holds.length > 0) {
    reason = `HOLD: required finding(s) ${holds.join(', ')} are not safe to execute.`
  } else if (units.length === 0) {
    reason = 'HOLD: no required executable work remains in this unit (adjacent findings routed to follow-ups/notes).'
  } else if (units.length === 1) {
    mode = 'SINGLE'
    reason = `SINGLE: ${units[0].findingIds.length} required finding(s) form one bounded Smith unit (${units[0].id}). ${followUps.length} adjacent finding(s) kept out of Smith scope.`
  } else {
    mode = 'SPLIT'
    reason = `SPLIT(${units.length}): required findings form ${units.length} independent bounded units. ${followUps.length} adjacent finding(s) kept out of Smith scope.`
  }

  return { mode, units, followUps, notes, holds, reason }
}

/** Map a shaped decision to the existing engine gate shape facts (leadDecision). */
export function leadShapePlan(decision: WorkShapeDecision): LeadShapePlan {
  if (decision.mode === 'SINGLE') {
    return { decision: 'SMITH', splitCount: null, unitsCount: decision.units.length }
  }
  if (decision.mode === 'SPLIT') {
    return { decision: 'SPLIT', splitCount: decision.units.length, unitsCount: decision.units.length }
  }
  return { decision: 'HOLD', splitCount: null, unitsCount: 0 }
}

/**
 * Mutation-authority scope for a SINGLE selected unit. This is what keeps Smith
 * bounded: a chosen unit owns ONLY its seams (+ an explicit fallback when a unit
 * declares none), never the seams of sibling units or follow-up findings.
 */
export function smithScopeForUnit(decision: WorkShapeDecision, unitId: string, fallback: string[] = []): string[] {
  const unit = decision.units.find((u) => u.id === unitId)
  if (!unit) return []
  return unit.seams.length > 0 ? unit.seams : [...fallback]
}

// -- Authoritative shaping gate (the dogfood anti-pattern, in code) -----------
// ENG-FORGE-SHAPE-01 authoritative decisions. Lead PRE is the judgment layer:
// single-Smith over an Architect packet that contains multiple independent
// required seams is refused here — a deterministic rule, not a model habit.

export type AuthoritativeLeadDecision = LeadShapePlan & { mode: WorkShapeMode }

export function authoritativeLeadDecision(findings: ArchitectFinding[]): AuthoritativeLeadDecision {
  const shape = shapeArchitectFindings({ findings })
  const plan = leadShapePlan(shape)
  return { decision: plan.decision, splitCount: plan.splitCount, mode: shape.mode, unitsCount: shape.units.length }
}

export type ShapeChoice = 'SOLO' | 'SMITH' | 'SPLIT' | 'HOLD'

/**
 * Validate the shape a Lead emits against the deterministic authoritative shape
 * derived from the Architect findings. Adjacent/non-blocking findings never
 * change the authoritative shape (they are quarantined to follow-ups), so this
 * check is stable regardless of how much discovery Architect produced.
 */
export function validateLeadShapeChoice(input: {
  findings: ArchitectFinding[]
  choice: ShapeChoice
  splitCount?: number | null
}): { ok: boolean; errors: string[]; authoritative: AuthoritativeLeadDecision } {
  const authoritative = authoritativeLeadDecision(input.findings)
  const errors: string[] = []
  if (input.choice === 'SPLIT') {
    if (authoritative.decision !== 'SPLIT') {
      errors.push(`SPLIT chosen but authoritative shape is ${authoritative.decision}`)
    } else if ((input.splitCount ?? null) !== authoritative.splitCount) {
      errors.push(`SPLIT count ${String(input.splitCount)} does not match ${authoritative.splitCount} bounded units`)
    }
  } else if (input.choice === 'SMITH' || input.choice === 'SOLO') {
    if (authoritative.decision === 'SPLIT') {
      errors.push(
        `single ${input.choice} chosen over ${authoritative.unitsCount} independent required seams -> must SPLIT:${authoritative.unitsCount}`,
      )
    }
    if (authoritative.decision === 'HOLD') {
      errors.push('cannot execute: the required work is HOLD (ambiguous/unsafe)')
    }
    if (input.splitCount != null) errors.push(`${input.choice} must not carry a splitCount`)
  }
  return { ok: errors.length === 0, errors, authoritative }
}

/**
 * Bind a single Smith node to exactly one bounded unit — before it launches.
 * A plain `smith`/`repair_smith` may only run when the shape is a SINGLE unit;
 * a `smith_split_work` child may only run when the shape is SPLIT and its index
 * addresses a real unit. Anything else fails closed (never launches unbounded).
 */
export function smithUnitForNode(
  decision: WorkShapeDecision,
  nodeId: string,
  splitIndex?: number | null,
): { unit: ShapeUnit | null; error: string | null } {
  if (nodeId === 'smith_split_work') {
    if (decision.mode !== 'SPLIT') {
      return { unit: null, error: `smith_split_work launched but authoritative shape is ${decision.mode}` }
    }
    const i = splitIndex ?? 0
    if (!Number.isInteger(i) || i < 0 || i >= decision.units.length) {
      return { unit: null, error: `split index ${String(i)} is outside ${decision.units.length} bounded units` }
    }
    return { unit: decision.units[i], error: null }
  }
  if (decision.mode !== 'SINGLE' || decision.units.length !== 1) {
    return { unit: null, error: `single ${nodeId} launched but authoritative shape is ${decision.mode}` }
  }
  return { unit: decision.units[0], error: null }
}


// ---------------------------------------------------------------------------
// ENG-FORGE-ARCHITECT-BRIEF-01 (slice 1) — the Architect brief is a CONTRACT.
//
// The findings marker is emitted by a MODEL, so the payload arrives in whatever
// shape it chose. findingsFromArchitectEvidence() tolerates that SHAPE (correctly),
// but tolerating shape must never mean accepting SILENCE. A brief that parsed to
// zero findings degraded quietly to `[]`, so the failure surfaced one lane later as
// LEAD refusing to route ("No required findings supplied") — blaming the wrong role
// with a message the Architect could neither act on nor retry against. WS-09 then
// invented a calendar because its contract was empty.
//
// This gate runs at the ARCHITECT boundary for the nodes that owe findings
// (`architect`, `repair_architect`) — deliberately NOT lane-wide, because
// `research_architect` shares lane 'architect' and owes a disposition, not findings.
// It fails CLOSED with diagnostics written for the retry: that is what turned LEAD
// from a deterministic HOLD into a self-correcting attempt.
//
// No schema change: the findings jsonb column already exists.
// ---------------------------------------------------------------------------

/** The chunk ceiling a REQUIRED finding must respect before it must be decomposed. */
export const MAX_SEAMS_PER_FINDING = 3

export type ArchitectBriefAssessment = {
  verdict: 'OK' | 'HOLD'
  reasons: string[]
  findingCount: number
  seamCount: number
}

export const ARCHITECT_FINDINGS_MISSING =
  'ARCHITECT_BRIEF: no readable findings. End the reply with exactly ONE un-fenced single JSON line beginning ' +
  '`FORGE_FINDINGS_JSON:` whose value is an array of {"id":"<stable-key>","summary":"<one distinct finding>",' +
  '"required":true|false,"seams":["<repository-relative path>","<path#symbol>"],' +
  '"hint":"SAME_UNIT|SPLIT_CHILD|FOLLOW_UP_STORY|NOTE|HOLD"}.'

/**
 * A seam is usable only when it is a repository-relative path. Mirrors the LEAD
 * pathOf() rules — and additionally rejects embedded whitespace, because a seam is
 * ONE path: if a sentence can live inside the value ("ui/a b.ts"), prose has leaked
 * into a machine field and the value should be rejected at the Architect's own
 * boundary rather than die later as a silent scope mismatch at LEAD.
 */
export function isRepoRelativeSeam(scope: string): boolean {
  const p = String(scope).trim().split('#')[0].replace(/^\.\//, '').replace(/\/+$/, '')
  if (!p || p.startsWith('/')) return false
  if (/[\s*?[\]{}:]/.test(p)) return false
  return !p.split('/').some((segment) => !segment || segment === '.' || segment === '..')
}

export function assessArchitectBrief(
  text: string | null | undefined,
  options: { maxSeamsPerFinding?: number } = {},
): ArchitectBriefAssessment {
  const maxSeams = options.maxSeamsPerFinding ?? MAX_SEAMS_PER_FINDING
  const reasons: string[] = []
  const raw = text ?? ''

  // Last parseable marker wins: the model's real emission is at the end, and an
  // echoed instruction line must never shadow it.
  const positions: number[] = []
  for (let at = raw.indexOf(STRUCTURED_PREFIX); at >= 0; at = raw.indexOf(STRUCTURED_PREFIX, at + 1)) {
    positions.push(at)
  }
  let payload: unknown[] | null = null
  for (let i = positions.length - 1; i >= 0; i--) {
    const slice = extractJsonArray(raw.slice(positions[i] + STRUCTURED_PREFIX.length))
    if (!slice) continue
    try {
      const parsed: unknown = JSON.parse(slice)
      if (Array.isArray(parsed) && parsed.length > 0) {
        payload = parsed
        break
      }
    } catch {
      /* try an earlier occurrence */
    }
  }

  const findings = payload ? parseForgeFindings(payload) : []
  if (!payload || findings.length === 0) {
    reasons.push(ARCHITECT_FINDINGS_MISSING)
    return { verdict: 'HOLD', reasons, findingCount: 0, seamCount: 0 }
  }

  // Silent row loss is the same disease as silent parse loss: a partial payload must
  // not quietly shrink the story's finding set.
  if (payload.length > findings.length) {
    reasons.push(
      `ARCHITECT_BRIEF: ${payload.length - findings.length} of ${payload.length} finding rows were dropped as ` +
        `incomplete — every row needs a non-empty "id" AND "summary" (${findings.length} usable).`,
    )
  }

  for (const finding of findings) {
    if (!finding.seams.length) {
      reasons.push(
        `ARCHITECT_BRIEF ${finding.id}: declare at least one repository-relative seam — LEAD builds work orders ` +
          'from seams, not prose.',
      )
      continue
    }
    const invalid = finding.seams.filter((seam) => !isRepoRelativeSeam(seam))
    if (invalid.length) {
      reasons.push(
        `ARCHITECT_BRIEF ${finding.id}: invalid seam(s) ${invalid.map((s) => `"${s}"`).join(', ')} — use ` +
          'repository-relative paths or path#symbol (no leading "/", no "./", no globs, no spaces).',
      )
    }
    if (finding.required && finding.seams.length > maxSeams) {
      reasons.push(
        `ARCHITECT_BRIEF ${finding.id}: ${finding.seams.length} seams exceeds the chunk ceiling of ${maxSeams} — ` +
          'decompose it into separate findings (or mark it hint SPLIT_CHILD / FOLLOW_UP_STORY).',
      )
    }
  }

  return {
    verdict: reasons.length ? 'HOLD' : 'OK',
    reasons,
    findingCount: findings.length,
    seamCount: findings.reduce((n, f) => n + f.seams.length, 0),
  }
}

