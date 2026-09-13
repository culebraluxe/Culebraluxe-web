import { assessSmithDispatch } from './forge-dispatch-gate'
import { dispatchabilityFor } from './forge-dispatchability'
import type { DispatchabilityFeatures } from './forge-dispatchability'
import type { SmithExecutionPlan } from './forge-execution-shaping'

export type Route = 'SOLO' | 'SMITH' | 'SPLIT' | 'HOLD'
export type Size = 'SMALL' | 'MEDIUM' | 'LARGE'
export type BenchIntent = 'SOLO' | 'SMITH' | 'SPLIT' | 'HOLD' | null

export type LeadAssignment = {
  id: string
  findingIds: string[]
  dependsOn: string[]
  evidenceRefs: string[]
  reasoning: string
  features: DispatchabilityFeatures
  plan: SmithExecutionPlan
}
export type LeadProposal = {
  version: 1
  decision: Route
  size: Size
  sizeReason: string
  reason: string
  assignments: LeadAssignment[]
  mergeChecks: string[]
}
export type RoutingContext = {
  findings: Array<{ id: string; required: boolean; hint?: string; seams: string[] }>
  evidenceRefs: string[]
  splitEnabled: boolean
  maxSmiths: number
  allowedProofs: string[]
  /** Captain launch intent from the Bench. Absent/null = no cap (live default). */
  benchIntent?: BenchIntent
}
export type RoutingReview =
  | { ok: false; errors: string[]; advisories: string[] }
  | { ok: true; proposal: LeadProposal; advisories: string[] }

const object = (v: unknown): v is Record<string, unknown> =>
  v !== null && typeof v === 'object' && !Array.isArray(v)
const nonempty = (v: unknown): v is string => typeof v === 'string' && v.trim().length > 0
const strings = (v: unknown): v is string[] => Array.isArray(v) && v.every(nonempty)
const unique = (v: string[]) => new Set(v).size === v.length
const riskKeys = ['uncertainty', 'contextBurden', 'proofBurden', 'coupling', 'changeNovelty', 'workerFit'] as const

function featuresValid(v: unknown): v is DispatchabilityFeatures {
  return object(v) && ['semanticSurface', 'dependencyDepth'].every(k =>
    Number.isInteger(v[k]) && Number(v[k]) >= 1 && Number(v[k]) <= 100
  ) && riskKeys.every(k => Number.isInteger(v[k]) && Number(v[k]) >= 1 && Number(v[k]) <= 5)
}

function chunkSurface(c: Record<string, unknown>): unknown {
  if (Array.isArray(c.surface)) return c.surface
  if (Array.isArray(c.scope)) return c.scope
  return c.surface
}

function planValidShape(v: unknown): v is SmithExecutionPlan {
  return object(v) && ['SMALL', 'MEDIUM', 'LARGE'].includes(String(v.size)) &&
    Array.isArray(v.chunks) && v.chunks.length >= 1 && v.chunks.length <= 3 &&
    v.chunks.every(c => object(c) && Number.isInteger(c.id) &&
      (nonempty(c.outcome) || nonempty(c.postconditions)) &&
      strings(chunkSurface(c)) && (chunkSurface(c) as string[]).length > 0 &&
      (nonempty(c.invariant) || nonempty(c.postconditions)) && nonempty(c.proof) &&
      (c.dependsOn === undefined || (Array.isArray(c.dependsOn) && c.dependsOn.every(d => Number.isInteger(d) && d > 0))))
}

export function proposalValidShape(v: unknown): v is LeadProposal {
  return object(v) && v.version === 1 && ['SOLO', 'SMITH', 'SPLIT', 'HOLD'].includes(String(v.decision)) &&
    ['SMALL', 'MEDIUM', 'LARGE'].includes(String(v.size)) && nonempty(v.sizeReason) && nonempty(v.reason) &&
    strings(v.mergeChecks) && Array.isArray(v.assignments) && v.assignments.length <= 8 &&
    v.assignments.every(a => object(a) && nonempty(a.id) && strings(a.findingIds) &&
      strings(a.dependsOn) && strings(a.evidenceRefs) && nonempty(a.reasoning) &&
      featuresValid(a.features) && planValidShape(a.plan))
}

function normalizeProposal(p: LeadProposal): LeadProposal {
  return {
    ...p,
    assignments: p.assignments.map((a) => ({
      ...a,
      plan: {
        ...a.plan,
        chunks: a.plan.chunks.map((c) => {
          const row = c as unknown as Record<string, unknown>
          const surface = strings(row.surface) ? (row.surface as string[]) : strings(row.scope) ? (row.scope as string[]) : c.surface
          const outcome = c.outcome || (typeof row.postconditions === 'string' ? row.postconditions : '')
          const invariant = c.invariant || (typeof row.postconditions === 'string' ? row.postconditions : '')
          return { ...c, surface, outcome, invariant }
        }),
      },
    })),
  }
}

function pathOf(scope: string): string | null {
  const p = scope.trim().split('#')[0].replace(/^\.\//, '').replace(/\/+$/, '')
  if (!p || p.startsWith('/') || /[\\*?[\]{}:]/.test(p) ||
      p.split('/').some(s => !s || s === '.' || s === '..')) return null
  return p
}
const within = (path: string, area: string) => path === area || path.startsWith(area + '/')
const overlap = (a: string, b: string) => within(a, b) || within(b, a)

export const SPLIT_UNAVAILABLE =
  'SPLIT is not available this run; HOLD and recut into one resident Smith (1..3 serial chunks) or a new story'
export const LARGE_REQUIRES_RECUT =
  'LARGE work cannot dispatch while SPLIT is unavailable; HOLD and recut into one MEDIUM Smith assignment or a new story'
export const LARGE_REQUIRES_SPLIT =
  'LARGE requires two or more bounded Smith assignments'

function benchIntentErrors(decision: string, benchIntent: BenchIntent | undefined): string[] {
  if (!benchIntent) return []
  if (benchIntent === 'HOLD' && decision !== 'HOLD') return ['Bench intent is HOLD — Lead may only HOLD']
  if (benchIntent === 'SOLO' && decision !== 'SOLO' && decision !== 'HOLD') {
    return [`Bench intent is SOLO — Lead may SOLO or HOLD, not ${decision}`]
  }
  if (benchIntent === 'SMITH' && decision === 'SPLIT') {
    return ['Bench intent is SMITH — Lead may not escalate to SPLIT']
  }
  return []
}

/** The AI proposes; code accepts or returns corrections. Never silently reroute. */
export function reviewLeadProposal(raw: unknown, context: RoutingContext): RoutingReview {
  const errors: string[] = []
  const advisories: string[] = []
  if (raw === null || raw === undefined) {
    return {
      ok: false,
      errors: [
        'No LEAD_ROUTING line was emitted — end the reply with exactly one un-fenced single JSON line beginning LEAD_ROUTING:',
      ],
      advisories,
    }
  }
  if (!proposalValidShape(raw)) return { ok: false, errors: ['Malformed LEAD_ROUTING proposal'], advisories }
  const p = normalizeProposal(raw)
  errors.push(...benchIntentErrors(p.decision, context.benchIntent))
  if (p.decision === 'HOLD') {
    if (p.assignments.length || p.mergeChecks.length) errors.push('HOLD must not dispatch assignments or checks')
    return errors.length ? { ok: false, errors, advisories } : { ok: true, proposal: p, advisories }
  }
  if (!Number.isInteger(context.maxSmiths) || context.maxSmiths < 1 || context.maxSmiths > 8) {
    errors.push('Runtime maxSmiths must be 1..8 (current XML ceiling)')
  }
  const required = context.findings.filter(f => f.required)
  if (!required.length) errors.push('No required findings supplied; obtain the bounded Architect handoff')
  if (!unique(context.findings.map(f => f.id))) errors.push('Duplicate finding IDs in Architect handoff')
  if (required.some(f => f.hint === 'HOLD')) errors.push('Required Architect HOLD remains unresolved')
  const n = p.assignments.length
  if ((p.decision === 'SOLO' || p.decision === 'SMITH') && n !== 1) errors.push(p.decision + ' requires one assignment')
  if (p.decision === 'SPLIT') {
    if (!context.splitEnabled) {
      errors.push(SPLIT_UNAVAILABLE)
    } else if (n < 2 || n > context.maxSmiths) {
      errors.push('SPLIT requires 2..maxSmiths assignments')
    }
  }
  if (p.size === 'SMALL' && p.decision === 'SPLIT') errors.push('SMALL work does not justify a split; revise the size or assignment')
  if (p.size === 'LARGE' && p.decision !== 'SPLIT') {
    if (!context.splitEnabled) {
      errors.push(LARGE_REQUIRES_RECUT)
    } else {
      errors.push(LARGE_REQUIRES_SPLIT)
    }
  }
  if (p.decision === 'SOLO' && p.size !== 'SMALL') errors.push('SOLO requires SMALL work')
  if (!unique(p.assignments.map(a => a.id))) errors.push('Duplicate assignment IDs')
  if (!p.mergeChecks.length || p.mergeChecks.some(c => !context.allowedProofs.includes(c))) {
    errors.push('Integrated acceptance must use the frozen story proof commands')
  }
  const covered = p.assignments.flatMap(a => a.findingIds)
  if (!unique(covered)) errors.push('A finding has multiple assignment owners; refine findings before dispatch')
  for (const f of required) if (!covered.includes(f.id)) errors.push('Unassigned required finding: ' + f.id)
  const pathsByAssignment: string[][] = []
  for (const a of p.assignments) {
    const prefix = a.id + ': '
    if (!a.findingIds.length || a.findingIds.some(id => !required.some(f => f.id === id))) {
      errors.push(prefix + 'claim only named required findings; no adjacent discovery in this story')
    }
    if (!a.evidenceRefs.length || a.evidenceRefs.some(ref => !context.evidenceRefs.includes(ref))) {
      errors.push(prefix + 'cite evidence present in the current handoff')
    }
    if (a.dependsOn.length) {
      errors.push(prefix + 'unfinished assignment dependencies require staged execution; current XML only supports sibling fan-out')
    }
    if (a.plan.size === 'LARGE') errors.push(prefix + 'assignment is still LARGE; decompose before dispatch')
    const scopes = required.filter(f => a.findingIds.includes(f.id)).flatMap(f => f.seams).map(pathOf)
    if (!scopes.length || scopes.some(s => s === null)) errors.push(prefix + 'Architect must supply explicit repository-relative scopes')
    const paths = a.plan.chunks.flatMap(c => c.surface).map(pathOf)
    pathsByAssignment.push(paths.filter((s): s is string => s !== null))
    const illegal = paths.filter(path => !path || !scopes.some(scope => scope && within(path, scope)))
    if (illegal.length) {
      errors.push(
        prefix +
          'chunk edits exceed the assigned Architect scope or use an invalid path: ' +
          illegal.map(path => path ?? '<unparseable surface>').join(', ') +
          ' — allowed seams: ' +
          (scopes.filter((scope): scope is string => scope !== null).join(', ') || '<none declared>'),
      )
    }
    if (a.plan.chunks.some(c => !context.allowedProofs.includes(c.proof))) {
      errors.push(prefix + 'chunk proof is not in the frozen story acceptance commands')
    }
    const gate = assessSmithDispatch(a.plan)
    if (gate.verdict === 'HOLD') errors.push(...gate.reasons.map(r => prefix + r))
    else if (gate.verdict === 'FLAG') advisories.push(...gate.reasons.map(r => prefix + r))
    const selfRated = dispatchabilityFor(a.features)
    if (selfRated.verdict === 'NOT_DISPATCHABLE') {
      advisories.push(
        `${prefix}self-rated ${selfRated.verdict}: ${selfRated.reasons.join('; ')} — advisory only, not a dispatch veto`,
      )
    } else if (selfRated.verdict === 'HEAVY') {
      advisories.push(`${prefix}self-rated HEAVY (${selfRated.chunks} chunk(s)): ${selfRated.reasons.join('; ')}`)
    }
    if (p.decision === 'SOLO' && (a.plan.size !== 'SMALL' || a.plan.chunks.length !== 1 ||
        a.features.semanticSurface !== 1 || a.features.dependencyDepth !== 1 ||
        riskKeys.some(k => a.features[k] > 2))) {
      errors.push(prefix + 'SOLO requires one bounded chunk, one behavior, low uncertainty/coupling/proof/context burden')
    }
  }
  if (p.decision === 'SPLIT' && context.splitEnabled) {
    for (let i = 0; i < n; i++) for (let j = i + 1; j < n; j++) {
      if (pathsByAssignment[i].some(a => pathsByAssignment[j].some(b => overlap(a, b)))) {
        errors.push('Concurrent write conflict: ' + p.assignments[i].id + ' / ' + p.assignments[j].id)
      }
    }
  }
  return errors.length ? { ok: false, errors, advisories } : { ok: true, proposal: p, advisories }
}

export function parseLeadRouting(notes: string): unknown {
  const lines = notes.split(/\r?\n/).map(s => s.trim()).filter(s => s.startsWith('LEAD_ROUTING:'))
  if (lines.length !== 1) return null
  try { return JSON.parse(lines[0].slice('LEAD_ROUTING:'.length).trim()) } catch { return null }
}

export function leadRoutingFacts(review: Extract<RoutingReview, { ok: true }>) {
  const p = review.proposal
  return {
    leadDecision: p.decision,
    splitCount: p.decision === 'SPLIT' ? p.assignments.length : 0,
    splitPlan: p.decision === 'SPLIT' ? p.assignments : [],
    leadRouting: p,
  }
}
