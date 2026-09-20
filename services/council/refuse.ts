import type { ConveneRequest } from './types'

export type RefuseDecision = {
  refuse: boolean
  reason: string | null
}

const LOOKUP = /\b(what is|who is|when is|look ?up|define|price of)\b/i
const BLESS = /\b(confirm my|bless|rubber[- ]stamp|justify (my|this) (already|preferred))\b/i

/**
 * Mechanical refuse gate. Cheap, deterministic, no model.
 * Domain callers can still convene; the service may override to refuse.
 */
export function decideRefuse(request: ConveneRequest): RefuseDecision {
  if (request.mode === 'full') {
    return { refuse: true, reason: 'full council is unsupported; use duo or quick' }
  }

  const q = request.question.trim()
  if (q.length < 12) {
    return { refuse: true, reason: 'question is too thin to convene' }
  }
  if (LOOKUP.test(q)) {
    return { refuse: true, reason: 'factual lookup — do not convene' }
  }
  if (BLESS.test(q)) {
    return { refuse: true, reason: 'council will not manufacture support for a preferred answer' }
  }

  // Operational facts with a single writer elsewhere.
  if (
    request.subjectType === 'deal' &&
    /\b(last contact|imessage|whatsapp|attribution)\b/i.test(q)
  ) {
    return { refuse: true, reason: 'channel/identity facts have one writer; not a council subject' }
  }

  return { refuse: false, reason: null }
}
