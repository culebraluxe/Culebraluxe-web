import type { AgentRuntimeRegistry } from './registry'
import { loadSkillText } from './skills'
import type { AgentRole, ModelProfile } from './types'
import {
  DEFAULT_LANES,
  type LaneBinding,
  type LaneId,
  type ModelLineage,
} from './lanes'
import {
  DEFAULT_FORGE_TEAM,
  resolveForgeAssignment,
  type ForgeAssignmentGrade,
  type ForgeTeam,
} from './team'

export type SmithGrade = ForgeAssignmentGrade

export interface LaneSession {
  smithLineage?: ModelLineage | null
  smithProfile?: ModelProfile | null
  smithGrade?: SmithGrade
  hasInlineDiff?: boolean
  hasScoutPacket?: boolean
  hasArchitectBrief?: boolean
  hasAssayPlan?: boolean
}

export interface LaneLaunch {
  lane: LaneId
  role: AgentRole
  modelProfile: ModelProfile
  lineage: ModelLineage
  maxSteps: number
  toolPolicy: LaneBinding['toolPolicy']
  specialInstructions: string
}

export type LaneRejectCode =
  | 'unknown-lane'
  | 'assignment-unavailable'
  | 'profile-unregistered'
  | 'same-lineage-review'
  | 'missing-diff'
  | 'missing-scout-packet'
  | 'missing-architect-brief'
  | 'missing-assay-plan'
  | 'full-not-authorized'
  | 'architect-must-not-tool'
  | 'emergency-not-authorized'

export type LaneDecision =
  | { ok: true; launch: LaneLaunch }
  | { ok: false; code: LaneRejectCode; reason: string }

export interface ResolveLaneInput {
  lane: LaneId
  session?: LaneSession
  smithGrade?: SmithGrade
  extraInstructions?: string | null
  registry?: Pick<AgentRuntimeRegistry, 'hasProfile'>
  lanes?: Record<LaneId, LaneBinding>
  team?: ForgeTeam
  authorizeEmergency?: boolean
}

const LANE_PREAMBLE: Record<LaneId, string> = {
  scout:
    'Lane=scout. Volume context gathering only. Do not design, edit, or commit. Cap tool calls. ' +
    'Checklist: 1) locate the story surface (routes/components/tables named in the brief); ' +
    '2) rank files by relevance with one-line why-each-matters; ' +
    '3) capture signatures (exports, props, column names) verbatim, never paraphrased; ' +
    '4) name the 3–7 files the next lane must read and what to look for in each. ' +
    'Return a packet: ranked files, signatures, and must-reads. Cost: flash volume — be thorough but cheap.',
  architect:
    'Lane=architect. Own design truth. Produce a complete contract for Lead: scope, constraints, execution-relevant architecture, acceptance checks, and Assay plan. ' +
    'Template: Goal (one line) / Non-goals (explicit) / Touched surfaces (files, tables, routes) / Contract (inputs, outputs, invariants) / Acceptance (verifiable checks, each with its Assay command) / Risks (what could invalidate this plan). ' +
    'Do not perform implementation. Cost: pro judgment — precision here saves every downstream token.',
  lead:
    'Lane=lead. Own execution strategy and integration. Validate the frozen Architect contract against repository reality, veto bad scope/architecture, choose the cheapest sound implementation shape, and protect the Assay handoff. Never silently rewrite architectural truth. ' +
    'Workflow: PRE (vet contract → SOLO/SMITH/HOLD with priced reason) → IMPLEMENT (solo builds, exact candidate) → POST (integrate Smith candidate, ASSAY/HOLD). ' +
    'Cost: pro judgment — your grade call is the spend decision.',
  smith:
    'Lane=smith. Implement against the frozen Architect contract and Lead assignment. Do not review your own diff. Stop when the assigned work is done or maxSteps is hit. ' +
    'Workflow: 1) read the 3–7 must-read files; 2) make the smallest change satisfying acceptance; ' +
    '3) run the frozen Assay commands locally before finishing; 4) leave the worktree clean with one candidate commit. ' +
    'Never invent scope, never weaken acceptance, never push. Cost: flash by default, pro on upgrade — act like the meter is running.',
  inspector:
    'Lane=inspector. Second opinion on the inline diff. Different lineage from Smith. Read-only. Disagreement is the point. Do not silently patch. ' +
    'Rubric: 1) does the diff do ONLY what the contract says (scope discipline)? 2) does it satisfy EVERY acceptance check (no partial credit)? ' +
    '3) does it introduce coupling, secrets, or migration risk the brief forbids? Report verdict + specific line refs; a clean pass needs no essay. ' +
    'Cost: pro review — cheaper than shipping a defect.',
  assay:
    'Lane=assay. Read-only TUNIT/evidence check. Instruction-following only. Inventiveness is a defect. Willing to return nothing. ' +
    'Execute exactly the frozen Assay commands against the exact candidate SHA, in order. ' +
    'Exit codes and numeric counters are the verdict; prose never overrides arithmetic. Cost: free (deterministic, model-free).',
  archive:
    'Lane=archive. One huge input, few calls. Extract invariants. Do not implement. ' +
    'Output: durable facts (what is true), decisions (what was chosen and why), open risks. Compress, do not narrate.',
  night:
    'Lane=night. Same job as Smith, detached. Write factual progress. Stop cleanly on maxSteps. ' +
    'Same smith workflow and cost discipline; the only difference is unattended execution — leave breadcrumbs, never leave mess.',
}

export function resolveLane(input: ResolveLaneInput): LaneDecision {
  const lanes = input.lanes ?? DEFAULT_LANES
  const binding = lanes[input.lane]
  if (!binding) {
    return { ok: false, code: 'unknown-lane', reason: `unknown lane '${input.lane}'` }
  }

  const grade = input.smithGrade ?? input.session?.smithGrade ?? 'default'
  if (
    (input.lane === 'smith' || input.lane === 'night') &&
    grade === 'emergency' &&
    !input.authorizeEmergency
  ) {
    return {
      ok: false,
      code: 'emergency-not-authorized',
      reason: 'emergency Smith grade requires an explicit authorizeEmergency flag',
    }
  }

  const assignmentGrade: ForgeAssignmentGrade =
    input.lane === 'smith' || input.lane === 'night' ? grade : 'default'

  let assignment
  try {
    assignment = resolveForgeAssignment(
      binding.position,
      input.team ?? DEFAULT_FORGE_TEAM,
      assignmentGrade,
    )
  } catch (error) {
    return {
      ok: false,
      code: 'assignment-unavailable',
      reason: String((error as Error)?.message ?? error),
    }
  }

  const profile = assignment.profile
  if (input.registry && !input.registry.hasProfile(profile)) {
    return {
      ok: false,
      code: 'profile-unregistered',
      reason: `profile '${profile}' is not registered — do not invent a silent default`,
    }
  }

  if (input.lane === 'inspector') {
    if (!input.session?.hasInlineDiff) {
      return {
        ok: false,
        code: 'missing-diff',
        reason: 'Inspector requires an inline diff from Smith',
      }
    }
    const smithLineage = input.session.smithLineage
    if (smithLineage && smithLineage === assignment.lineage) {
      return {
        ok: false,
        code: 'same-lineage-review',
        reason: `Inspector lineage '${assignment.lineage}' equals Smith lineage — map a different player/lab.`,
      }
    }
  }

  if (input.lane === 'architect' && input.session && input.session.hasScoutPacket === false) {
    return {
      ok: false,
      code: 'missing-scout-packet',
      reason: 'Architect does not tool. Scout must produce a packet first.',
    }
  }

  if (
    (input.lane === 'lead' || input.lane === 'smith' || input.lane === 'night') &&
    input.session &&
    input.session.hasArchitectBrief === false
  ) {
    return {
      ok: false,
      code: 'missing-architect-brief',
      reason:
        `${input.lane === 'lead' ? 'Lead' : 'Smith'} requires the frozen Architect contract. Write the brief or run the Architect lane first.`,
    }
  }

  if (input.lane === 'assay' && input.session && input.session.hasAssayPlan === false) {
    return {
      ok: false,
      code: 'missing-assay-plan',
      reason: 'Assay needs the frozen Assay command plan.',
    }
  }

  if (input.lane === 'architect' && binding.toolPolicy !== 'plan-only') {
    return {
      ok: false,
      code: 'architect-must-not-tool',
      reason: 'Architect lane must stay plan-only',
    }
  }

  const extra = (input.extraInstructions ?? '').trim()
  // Spend vision: lane default skills guarantee domain knowledge even when
  // the packet omits Skills. Packet-listed skills are appended by
  // storyPacketInstructions via `extra`; defaults come first so the packet
  // can specialize without losing the baseline.
  const defaults = laneDefaultSkillInstructions(input.lane)
  const specialInstructions = [LANE_PREAMBLE[input.lane], defaults, extra || null]
    .filter(Boolean)
    .join('\n\n')

  return {
    ok: true,
    launch: {
      lane: input.lane,
      role: binding.role,
      modelProfile: profile,
      lineage: assignment.lineage,
      maxSteps: binding.maxSteps,
      toolPolicy: binding.toolPolicy,
      specialInstructions,
    },
  }
}

/**
 * Spend vision: default skill packs per lane. Packet `Skills` still wins when
 * present; these defaults guarantee every role starts with its domain
 * knowledge even when the packet omits the section.
 */
export const LANE_DEFAULT_SKILLS: Record<LaneId, string[]> = {
  scout: ['neon'],
  architect: ['planner', 'neon'],
  lead: ['planner'],
  smith: ['workflow', 'ui'],
  inspector: ['workflow'],
  assay: [],
  archive: ['planner'],
  night: ['workflow', 'ui'],
}

export const DEFAULT_PIPELINE: LaneId[] = [
  'scout',
  'architect',
  'lead',
  'smith',
  'inspector',
  'assay',
]

/** Rendered default skill pack for a lane, or null when the lane has none. */
export function laneDefaultSkillInstructions(lane: LaneId, repoRoot = process.cwd()): string | null {
  const ids = LANE_DEFAULT_SKILLS[lane] ?? []
  if (ids.length === 0) return null
  const body = loadSkillText(ids as never, repoRoot).trim()
  if (!body) return null
  return `Lane default skills (${ids.join(', ')}):\n${body}`
}
