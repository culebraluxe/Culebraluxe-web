export type QaDecisionCode = 'ASSAY' | 'HOLD'

export type QaDecision = {
  decision: QaDecisionCode
  reason: string | null
}

const DECISION_LINE = /^\s*QA_DECISION:\s*(ASSAY|HOLD)\s*$/im
const REASON_LINE = /^\s*QA_REASON:\s*(.+)\s*$/im

export function parseQaDecision(text: string | null | undefined): QaDecision | null {
  if (!text) return null
  const decision = text.match(DECISION_LINE)?.[1]?.toUpperCase() as QaDecisionCode | undefined
  if (!decision) return null
  return {
    decision,
    reason: text.match(REASON_LINE)?.[1]?.trim() || null,
  }
}

export function qaGateInstructions(candidateShas: readonly string[]): string {
  const candidates = candidateShas.filter(Boolean)
  return [
    'Independent QA gate. Review the integrated candidate against the frozen Architect contract and Lead handoff.',
    candidates.length
      ? `Candidate under review: ${candidates.join(', ')}`
      : 'Candidate identity is supplied by the typed Forge work envelope.',
    'Read-only: do not patch, commit, weaken acceptance criteria, or reinterpret the architecture.',
    'Use repository evidence and focused checks where useful. Deterministic Assay remains the final machine acceptance gate.',
    'If the implementation, architecture conformance, or acceptance evidence is not good enough, HOLD.',
    'End with exactly one machine line: QA_DECISION: ASSAY | HOLD',
    'Then one concise line: QA_REASON: <why>.',
  ].join('\n')
}
