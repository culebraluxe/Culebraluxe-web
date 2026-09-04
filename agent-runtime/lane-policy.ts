import type { AgentRuntimeRegistry } from './registry'
import type { AgentRole, ModelProfile } from './types'
import {
  DEFAULT_LANES,
  type LaneBinding,
  type LaneId,
  type ModelLineage,
} from './lanes'
import {
  ARCHITECT_HANDOFF_INSTRUCTIONS,
  SCOUT_HANDOFF_INSTRUCTIONS,
} from './handoff-contract'
import {
  DEFAULT_FORGE_TEAM,
  resolveForgeAssignment,
  type ForgeAssignmentGrade,
  type ForgeFieldId,
  type ForgeHarnessId,
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
  playerId: string
  providerId: string
  modelId: string
  harnessId: ForgeHarnessId
  fieldId: ForgeFieldId
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
  scout: [
    'Lane=scout. Volume context gathering only. Do not design, edit, or commit. Cap tool calls.',
    SCOUT_HANDOFF_INSTRUCTIONS,
  ].join('\n'),
  architect: [
    'Lane=architect. Own design truth. Produce a complete contract for Lead. Do not perform implementation.',
    ARCHITECT_HANDOFF_INSTRUCTIONS,
  ].join('\n'),
  lead:
    'Lane=lead. Own execution strategy and integration. Validate the frozen Architect contract against repository reality, veto bad scope/architecture, choose the cheapest sound implementation shape, and protect the QA/Assay handoff. Never silently rewrite architectural truth.',
  smith:
    'Lane=smith. Implement against the frozen Architect contract and Lead assignment. Do not review your own diff. Stop when the assigned work is done or maxSteps is hit.',
  inspector:
    'Lane=inspector. Independent QA judgment on the integrated candidate. Different lineage from Smith. Read-only. Challenge implementation and architecture conformance; do not silently patch.',
  assay:
    'Lane=assay. Deterministic exact-candidate evidence check. Instruction-following only. Inventiveness is a defect.',
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
      return { ok: false, code: 'missing-diff', reason: 'QA requires an integrated candidate/diff from Lead' }
    }
    const smithLineage = input.session.smithLineage
    if (smithLineage && smithLineage === assignment.lineage) {
      return {
        ok: false,
        code: 'same-lineage-review',
        reason: `QA lineage '${assignment.lineage}' equals Smith lineage — map a different player/lab.`,
      }
    }
  }

  if (input.lane === 'architect' && input.session && input.session.hasScoutPacket === false) {
    return { ok: false, code: 'missing-scout-packet', reason: 'Architect requires the Scout packet before planning.' }
  }

  if (
    (input.lane === 'lead' || input.lane === 'smith' || input.lane === 'night') &&
    input.session &&
    input.session.hasArchitectBrief === false
  ) {
    return {
      ok: false,
      code: 'missing-architect-brief',
      reason: `${input.lane === 'lead' ? 'Lead' : 'Smith'} requires the frozen Architect contract. Run the Architect lane first.`,
    }
  }

  if (input.lane === 'assay' && input.session && input.session.hasAssayPlan === false) {
    return { ok: false, code: 'missing-assay-plan', reason: 'Assay needs the frozen Assay command plan.' }
  }

  if (input.lane === 'architect' && binding.toolPolicy !== 'plan-only') {
    return { ok: false, code: 'architect-must-not-tool', reason: 'Architect lane must stay plan-only' }
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
      playerId: assignment.player.id,
      providerId: assignment.player.provider,
      modelId: assignment.player.model,
      harnessId: assignment.harness.id,
      fieldId: assignment.field.id,
      lineage: assignment.lineage,
      maxSteps: binding.maxSteps,
      toolPolicy: binding.toolPolicy,
      specialInstructions,
    },
  }
}

export const DEFAULT_PIPELINE: LaneId[] = [
  'scout', 'architect', 'lead', 'smith', 'inspector', 'assay',
]
