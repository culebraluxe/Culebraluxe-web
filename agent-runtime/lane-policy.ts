import type { AgentRuntimeRegistry } from './registry'
import type { AgentRole, ModelProfile } from './types'
import {
  DEFAULT_LANES,
  type LaneBinding,
  type LaneId,
  type ModelLineage,
} from './lanes'

export type SmithGrade = 'default' | 'upgrade' | 'emergency'

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
  authorizeEmergency?: boolean
}

const LANE_PREAMBLE: Record<LaneId, string> = {
  scout:
    'Lane=scout. Volume context gathering only. Do not design, edit, or commit. Cap tool calls. Return a packet: ranked files, signatures, and the 3–7 files the next lane must read.',
  architect:
    'Lane=architect. Own design truth. Produce a complete contract for Lead: scope, constraints, execution-relevant architecture, acceptance checks, and Assay plan. Do not perform implementation.',
  lead:
    'Lane=lead. Own execution strategy and integration. Validate the frozen Architect contract against repository reality, veto bad scope/architecture, choose the cheapest sound implementation shape, and protect the Assay handoff. Never silently rewrite architectural truth.',
  smith:
    'Lane=smith. Implement against the frozen Architect contract and Lead assignment. Do not review your own diff. Stop when the assigned work is done or maxSteps is hit.',
  inspector:
    'Lane=inspector. Second opinion on the inline diff. Different lineage from Smith. Read-only. Disagreement is the point. Do not silently patch.',
  assay:
    'Lane=assay. Read-only TUNIT/evidence check. Instruction-following only. Inventiveness is a defect. Willing to return nothing.',
  archive:
    'Lane=archive. One huge input, few calls. Extract invariants. Do not implement.',
  night:
    'Lane=night. Same job as Smith, detached. Write factual progress. Stop cleanly on maxSteps.',
}

export function resolveLane(input: ResolveLaneInput): LaneDecision {
  const lanes = input.lanes ?? DEFAULT_LANES
  const binding = lanes[input.lane]
  if (!binding) {
    return { ok: false, code: 'unknown-lane', reason: `unknown lane '${input.lane}'` }
  }

  const grade = input.smithGrade ?? input.session?.smithGrade ?? 'default'
  let profile = binding.profile
  if ((input.lane === 'smith' || input.lane === 'night') && grade === 'upgrade') {
    profile = binding.upgradeProfile ?? binding.profile
  }
  if ((input.lane === 'smith' || input.lane === 'night') && grade === 'emergency') {
    if (!input.authorizeEmergency) {
      return {
        ok: false,
        code: 'emergency-not-authorized',
        reason: 'emergency Smith grade requires an explicit authorizeEmergency flag',
      }
    }
    profile = binding.emergencyProfile ?? binding.profile
  }

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
    if (smithLineage && smithLineage === binding.lineage) {
      return {
        ok: false,
        code: 'same-lineage-review',
        reason: `Inspector lineage '${binding.lineage}' equals Smith lineage — pick a different lab`,
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
  const specialInstructions = extra
    ? `${LANE_PREAMBLE[input.lane]}\n\n${extra}`
    : LANE_PREAMBLE[input.lane]

  return {
    ok: true,
    launch: {
      lane: input.lane,
      role: binding.role,
      modelProfile: profile,
      lineage: binding.lineage,
      maxSteps: binding.maxSteps,
      toolPolicy: binding.toolPolicy,
      specialInstructions,
    },
  }
}

export const DEFAULT_PIPELINE: LaneId[] = [
  'scout',
  'architect',
  'lead',
  'smith',
  'inspector',
  'assay',
]
