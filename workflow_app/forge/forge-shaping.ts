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

/** Read Architect findings from the evidence marker in notes/testsSummary text. */
export function findingsFromArchitectEvidence(text: string | null | undefined): ArchitectFinding[] {
  if (!text) return []
  const match = text.match(/FORGE_FINDINGS_JSON:\s*(\[[\s\S]*\])\s*$/m)
  if (!match) return []
  try {
    return parseForgeFindings(JSON.parse(match[1]))
  } catch {
    return []
  }
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

