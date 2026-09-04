export type LeadRunPhase = 'pre' | 'implement' | 'post'

export type LeadDecisionCode = 'SOLO' | 'SMITH' | 'SPLIT' | 'ASSAY' | 'HOLD'

export type LeadDecision = {
  decision: LeadDecisionCode
  splitCount: number | null
  assignments: string[]
  reason: string | null
}

const PHASE_DIRECTIVE = /\[forge\s+run-phase:\s*(pre|implement|post)\]/i
const DECISION_LINE = /^\s*LEAD_DECISION:\s*(SOLO|SMITH|ASSAY|HOLD|SPLIT(?:\s*:\s*\d+)?)\s*$/im
const REASON_LINE = /^\s*LEAD_REASON:\s*(.+)\s*$/im
const ASSIGNMENT_LINE = /^\s*LEAD_ASSIGNMENT_(\d+):\s*(.+)\s*$/gim

export function withLeadRunPhaseDirective(
  instructions: string,
  phase: LeadRunPhase,
): string {
  return `[forge run-phase: ${phase}] ${instructions}`.trim()
}

export function leadRunPhaseFromInstructions(
  instructions: string | null | undefined,
): LeadRunPhase | null {
  const match = instructions?.match(PHASE_DIRECTIVE)
  return (match?.[1]?.toLowerCase() as LeadRunPhase | undefined) ?? null
}

function splitAssignments(text: string): string[] {
  const indexed = new Map<number, string>()
  for (const match of text.matchAll(ASSIGNMENT_LINE)) {
    const slot = Number(match[1])
    const assignment = match[2]?.trim()
    if (Number.isInteger(slot) && slot > 0 && assignment) indexed.set(slot, assignment)
  }
  return [...indexed.entries()]
    .sort(([left], [right]) => left - right)
    .map(([, assignment]) => assignment)
}

export function parseLeadDecision(text: string | null | undefined): LeadDecision | null {
  if (!text) return null
  const match = text.match(DECISION_LINE)
  if (!match) return null
  const token = match[1].trim().toUpperCase()
  const reason = text.match(REASON_LINE)?.[1]?.trim() || null
  if (token.startsWith('SPLIT')) {
    const count = Number(token.split(':')[1]?.trim())
    if (!Number.isInteger(count) || count < 2 || count > 3) return null
    const assignments = splitAssignments(text)
    if (assignments.length !== count) return null
    return { decision: 'SPLIT', splitCount: count, assignments, reason }
  }
  return {
    decision: token as Exclude<LeadDecisionCode, 'SPLIT'>,
    splitCount: null,
    assignments: [],
    reason,
  }
}

export function leadPhaseInstructions(phase: LeadRunPhase): string {
  if (phase === 'pre') {
    return withLeadRunPhaseDirective(
      [
        'Lead PRE gate. Validate the frozen Architect contract against repository reality before implementation.',
        'You may STOP when scope, architecture, dependencies, acceptance criteria, Assay plan, or execution assumptions are wrong. Do not silently rewrite the Architect contract.',
        'Choose the cheapest sound execution shape. Default to NOT splitting. SOLO when this is small enough for Lead to implement directly. SMITH for one coherent implementation assignment. SPLIT:2 or SPLIT:3 only when ownership boundaries and interfaces are stable enough for those developers to work with bounded coordination AND each assignment is substantial enough to repay coordination/token cost.',
        'A dependency between layers is NOT a reason to avoid splitting when the interface is stable. The decision is bounded coordination, not independence.',
        'If you choose SPLIT:n, add exactly n lines before the decision: LEAD_ASSIGNMENT_1: <bounded work>, LEAD_ASSIGNMENT_2: <bounded work>, and LEAD_ASSIGNMENT_3 only for SPLIT:3. Each developer must own a concrete non-overlapping deliverable or explicit interface contract.',
        'Do not implement during PRE.',
        'End with exactly one machine line: LEAD_DECISION: SOLO | SMITH | SPLIT:2 | SPLIT:3 | HOLD',
        'Then one concise line: LEAD_REASON: <why>.',
      ].join('\n'),
      phase,
    )
  }

  if (phase === 'implement') {
    return withLeadRunPhaseDirective(
      [
        'Lead SOLO implementation. The Lead already accepted the frozen Architect contract and decided this story is cheaper to implement directly than delegate.',
        'Implement the complete contract, verify it within the frozen test policy, and create the candidate commit when code changes are required.',
        'If implementation reveals that the architecture or scope is wrong, stop and report the blocker rather than silently redesigning the story.',
      ].join('\n'),
      phase,
    )
  }

  return withLeadRunPhaseDirective(
    [
      'Lead POST integration gate. Review and integrate every typed Smith candidate against the frozen Architect contract.',
      'You may edit/integrate and create one new integrated candidate commit when required. You may STOP if implementation reveals an architectural or scope defect.',
      'Do not weaken acceptance criteria or Assay requirements to make the work pass.',
      'End with exactly one machine line: LEAD_DECISION: ASSAY | HOLD',
      'Then one concise line: LEAD_REASON: <why>.',
    ].join('\n'),
    phase,
  )
}
