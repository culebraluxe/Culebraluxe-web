export type LeadRunPhase = 'pre' | 'implement' | 'post'

export type LeadDecisionCode = 'SOLO' | 'SMITH' | 'SPLIT' | 'ASSAY' | 'HOLD'

export type LeadDecision = {
  decision: LeadDecisionCode
  splitCount: number | null
  reason: string | null
}

const PHASE_DIRECTIVE = /\[forge\s+run-phase:\s*(pre|implement|post)\]/i
const DECISION_LINE = /^\s*LEAD_DECISION:\s*(SOLO|SMITH|ASSAY|HOLD|SPLIT(?:\s*:\s*\d+)?)\s*$/im
const REASON_LINE = /^\s*LEAD_REASON:\s*(.+)\s*$/im

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

export function parseLeadDecision(text: string | null | undefined): LeadDecision | null {
  if (!text) return null
  const match = text.match(DECISION_LINE)
  if (!match) return null
  const token = match[1].trim().toUpperCase()
  const reason = text.match(REASON_LINE)?.[1]?.trim() || null
  if (token.startsWith('SPLIT')) {
    const count = Number(token.split(':')[1]?.trim())
    if (!Number.isInteger(count) || count < 2) return null
    return { decision: 'SPLIT', splitCount: count, reason }
  }
  return {
    decision: token as Exclude<LeadDecisionCode, 'SPLIT'>,
    splitCount: null,
    reason,
  }
}

export function leadPhaseInstructions(phase: LeadRunPhase): string {
  if (phase === 'pre') {
    return withLeadRunPhaseDirective(
      [
        'Lead PRE gate. Validate the frozen Architect contract against repository reality before implementation.',
        'You may STOP when scope, architecture, dependencies, acceptance criteria, Assay plan, or execution assumptions are wrong. Do not silently rewrite the Architect contract.',
        'Choose the cheapest sound execution shape. Default to NOT splitting. SOLO when this is small enough for Lead to implement directly. SMITH for one coherent implementation assignment. SPLIT:n only when ownership boundaries and interfaces are stable enough for n developers to work with bounded coordination AND each assignment is substantial enough to repay coordination/token cost.',
        'A dependency between layers is NOT a reason to avoid splitting when the interface is stable. The decision is bounded coordination, not independence.',
        'Do not implement during PRE.',
        'End with exactly one machine line: LEAD_DECISION: SOLO | SMITH | SPLIT:n | HOLD',
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
      'Lead POST integration gate. Review and integrate the Smith candidate against the frozen Architect contract.',
      'You may edit/integrate and create a new candidate commit when required. You may STOP if implementation reveals an architectural or scope defect.',
      'Do not weaken acceptance criteria or Assay requirements to make the work pass.',
      'End with exactly one machine line: LEAD_DECISION: ASSAY | HOLD',
      'Then one concise line: LEAD_REASON: <why>.',
    ].join('\n'),
    phase,
  )
}
