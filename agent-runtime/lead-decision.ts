import { renderModelCostLines } from './model-prices'

export type LeadRunPhase = 'pre' | 'implement' | 'post'

export type LeadDecisionCode = 'SOLO' | 'SMITH' | 'SPLIT' | 'ASSAY' | 'HOLD'

export type LeadDecision = {
  decision: LeadDecisionCode
  splitCount: number | null
  reason: string | null
}

const PHASE_DIRECTIVE = /\[forge\s+run-phase:\s*(pre|implement|post)\]/i
const DECISION_PATTERN = /^\s*(?:\*\*)?LEAD_DECISION:\s*(?:\*\*)?\s*(SOLO|SMITH|ASSAY|HOLD|SPLIT(?:\s*:\s*\d+)?)\.?\s*$/i
const REASON_PATTERN = /^\s*(?:\*\*)?LEAD_REASON:\s*(?:\*\*)?\s*(.+?)\.?\s*$/i

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

function cleanMarkdownLine(text: string): string {
  let line = text.trim()
  // Strip fenced code markers and blockquote/bullet prefixes.
  line = line.replace(/^```.*$/, '').trim()
  line = line.replace(/^>\s?/, '').trim()
  line = line.replace(/^[-*]\s+/, '').trim()
  // Strip surrounding bold/italic/code markers: **, __, *, _, `.
  line = line.replace(/^(\*\*|__|`)+/, '').trim()
  line = line.replace(/(\*\*|__|`)+$/, '').trim()
  line = line.replace(/`/g, '').trim()
  // Allow "**LEAD_DECISION:** VALUE" -> "LEAD_DECISION: VALUE".
  line = line.replace(/\*\*/g, '').trim()
  return line
}

export function parseLeadDecision(text: string | null | undefined): LeadDecision | null {
  if (!text) return null
  const lines = text.split('\n').map(cleanMarkdownLine)
  let token: string | null = null
  let reason: string | null = null

  for (const line of lines) {
    const decisionMatch = line.match(DECISION_PATTERN)
    if (decisionMatch && !token) {
      token = decisionMatch[1].trim().toUpperCase()
    }
    const reasonMatch = line.match(REASON_PATTERN)
    if (reasonMatch && !reason) {
      reason = reasonMatch[1].trim()
    }
  }

  if (!token) return null

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
        'Choose the cheapest sound execution shape. Default to NOT splitting. SOLO when this is small enough for Lead to implement directly. SMITH for one coherent implementation assignment. SPLIT:n is currently DISABLED (single-active work queue; multi-worker decomposition is not yet activated): do NOT emit SPLIT:n — choosing it only pauses the story on Hold for human decomposition. Prefer SOLO or SMITH.',
        ...renderModelCostLines(),
        'Name the grade tradeoff in LEAD_REASON (e.g. "SOLO on flash: trivial edit, no Smith spend" or "SMITH upgrade to pro: cross-file refactor repays 10x").',
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
