import type { AgentCapability } from './capabilities'
import { ASSAY_CAPABILITIES, READ_CAPABILITIES, WRITE_CAPABILITIES } from './lanes'

/**
 * Forge vocabulary boundary:
 * - Position = SDLC responsibility. It never names a model or provider.
 * - Profile = logical execution grade selected for a position.
 * - Player = provider/model identity that can fill a profile.
 * - Harness = software seam used to drive that player.
 * - Field = execution environment/topology where the run executes.
 *
 * The team map is the ONLY position -> profile/player/harness/field map.
 * Lane policy owns behavior/capabilities, not model selection.
 */
export type ForgePosition =
  | 'scout'
  | 'architect'
  | 'lead'
  | 'smith'
  | 'inspector'
  | 'assay'
  | 'archive'
  | 'night'

export type ForgeHarnessId =
  | 'forge-native'
  | 'forge-assay'
  | 'opencode'
  | 'openclaw'
  | 'pi'
  | 'warp-agent'

export type ForgeFieldId = 'local' | 'warp-swarm'

export type ForgeHarnessStatus = 'ready' | 'unconfigured' | 'interactive-only'

export interface ForgeHarness {
  id: ForgeHarnessId
  name: string
  status: ForgeHarnessStatus
  /** Provider families this harness can truthfully execute. */
  supportedProviders: string[]
  description: string
}

export interface ForgeField {
  id: ForgeFieldId
  name: string
  location: 'local' | 'cloud'
  topology: 'sequential' | 'parallel-capable'
  ready: boolean
  description: string
}

export interface ForgePlayer {
  id: string
  name: string
  provider: string
  /** Provider-local model id. Gateway harnesses receive provider/model explicitly. */
  model: string
  capabilities: AgentCapability[]
  /** Definition is usable; host credentials/readiness are checked by the harness. */
  ready: boolean
}

/** One concrete mapping choice. Same player may use different harnesses by role. */
export interface ForgeAssignmentVariant {
  profile: string
  playerId: string
  harnessId: ForgeHarnessId
  fieldId: ForgeFieldId
  /** Model-family/lab identity used only for independent-review separation. */
  lineage: string
}

export interface ForgePositionAssignment extends ForgeAssignmentVariant {
  position: ForgePosition
  upgrade?: ForgeAssignmentVariant
  emergency?: ForgeAssignmentVariant
}

export interface ForgeTeam {
  id: string
  name: string
  assignments: Record<ForgePosition, ForgePositionAssignment>
}

export const FORGE_HARNESSES: Record<ForgeHarnessId, ForgeHarness> = {
  'forge-native': {
    id: 'forge-native',
    name: 'Forge Native',
    status: 'ready',
    supportedProviders: ['deepseek'],
    description: 'Forge-owned DeepSeek Harness path. Host readiness is checked by the runtime registry.',
  },
  'forge-assay': {
    id: 'forge-assay',
    name: 'Forge Assay',
    status: 'ready',
    supportedProviders: ['forge'],
    description: 'Deterministic model-free exact-candidate verification harness.',
  },
  opencode: {
    id: 'opencode',
    name: 'OpenCode',
    status: 'ready',
    supportedProviders: ['deepseek'],
    description: 'Implemented OpenCode inner harness. The supported model is pinned and checked fail-closed.',
  },
  openclaw: {
    id: 'openclaw',
    name: 'OpenClaw',
    status: 'ready',
    supportedProviders: ['openai', 'anthropic', 'deepseek', 'google', 'ollama'],
    description: 'Headless gateway with explicit per-run provider/model selection.',
  },
  pi: {
    id: 'pi',
    name: 'Pi',
    status: 'unconfigured',
    supportedProviders: [],
    description: 'Reserved harness connection point; no runtime adapter is configured yet.',
  },
  'warp-agent': {
    id: 'warp-agent',
    name: 'Warp Agent',
    status: 'interactive-only',
    supportedProviders: ['openai', 'anthropic', 'deepseek', 'google'],
    description: 'Warp gateway exists, but unattended/headless readiness must be qualified on the execution host.',
  },
}

export const FORGE_FIELDS: Record<ForgeFieldId, ForgeField> = {
  local: {
    id: 'local',
    name: 'Local Mac',
    location: 'local',
    topology: 'sequential',
    ready: true,
    description: 'Current isolated-worktree execution field on the operator Mac.',
  },
  'warp-swarm': {
    id: 'warp-swarm',
    name: 'Warp Swarm',
    location: 'cloud',
    topology: 'parallel-capable',
    ready: false,
    description: 'Reserved future parallel field. Lead-controlled decomposition is required before activation.',
  },
}

/** Model/provider roster. Harness is deliberately NOT a player property. */
export const FORGE_PLAYERS: Record<string, ForgePlayer> = {
  'deepseek-flash': {
    id: 'deepseek-flash',
    name: 'DeepSeek V4 Flash',
    provider: 'deepseek',
    model: 'deepseek-v4-flash',
    capabilities: WRITE_CAPABILITIES,
    ready: true,
  },
  'deepseek-pro': {
    id: 'deepseek-pro',
    name: 'DeepSeek Pro',
    provider: 'deepseek',
    model: 'pro',
    capabilities: [...READ_CAPABILITIES, ...WRITE_CAPABILITIES, ...ASSAY_CAPABILITIES],
    ready: true,
  },
  'openai-gpt-5.6-sol': {
    id: 'openai-gpt-5.6-sol',
    name: 'GPT-5.6 Sol',
    provider: 'openai',
    model: 'gpt-5.6-sol',
    capabilities: [...READ_CAPABILITIES, ...WRITE_CAPABILITIES, ...ASSAY_CAPABILITIES],
    ready: true,
  },
  'anthropic-claude-sonnet-4-6': {
    id: 'anthropic-claude-sonnet-4-6',
    name: 'Claude Sonnet 4.6',
    provider: 'anthropic',
    model: 'claude-sonnet-4-6',
    capabilities: [...READ_CAPABILITIES, ...WRITE_CAPABILITIES, ...ASSAY_CAPABILITIES],
    ready: true,
  },
  'forge-deterministic-assay': {
    id: 'forge-deterministic-assay',
    name: 'Forge Deterministic Assay',
    provider: 'forge',
    model: 'deterministic',
    capabilities: ASSAY_CAPABILITIES,
    ready: true,
  },
}

/**
 * Default team is only a MAP. Roles remain model-agnostic.
 * Change this map or use FORGE_{PLAYER,HARNESS,FIELD,PROFILE,LINEAGE}_<POSITION>
 * to put GPT/Claude/DeepSeek/etc. into a position without changing lane code.
 */
export const DEFAULT_FORGE_TEAM: ForgeTeam = {
  id: 'default',
  name: 'Forge Default',
  assignments: {
    scout: {
      position: 'scout',
      profile: 'scout-volume',
      playerId: 'deepseek-flash',
      harnessId: 'forge-native',
      fieldId: 'local',
      lineage: 'deepseek-volume',
    },
    architect: {
      position: 'architect',
      profile: 'architect-pro',
      playerId: 'deepseek-pro',
      harnessId: 'forge-native',
      fieldId: 'local',
      lineage: 'deepseek-judgment',
    },
    lead: {
      position: 'lead',
      profile: 'lead-pro',
      playerId: 'deepseek-pro',
      harnessId: 'forge-native',
      fieldId: 'local',
      lineage: 'deepseek-judgment',
    },
    smith: {
      position: 'smith',
      profile: 'builder-flash',
      playerId: 'deepseek-flash',
      harnessId: 'opencode',
      fieldId: 'local',
      lineage: 'deepseek-volume',
      upgrade: {
        profile: 'builder-plus',
        playerId: 'deepseek-pro',
        harnessId: 'forge-native',
        fieldId: 'local',
        lineage: 'deepseek-judgment',
      },
      emergency: {
        profile: 'builder-emergency',
        playerId: 'deepseek-pro',
        harnessId: 'forge-native',
        fieldId: 'local',
        lineage: 'deepseek-judgment',
      },
    },
    inspector: {
      position: 'inspector',
      profile: 'reviewer-other',
      playerId: 'deepseek-pro',
      harnessId: 'forge-native',
      fieldId: 'local',
      lineage: 'deepseek-judgment',
    },
    assay: {
      position: 'assay',
      profile: 'verifier-mini',
      playerId: 'forge-deterministic-assay',
      harnessId: 'forge-assay',
      fieldId: 'local',
      lineage: 'forge-deterministic',
    },
    archive: {
      position: 'archive',
      profile: 'architect-pro',
      playerId: 'deepseek-pro',
      harnessId: 'forge-native',
      fieldId: 'local',
      lineage: 'deepseek-judgment',
    },
    night: {
      position: 'night',
      profile: 'builder-flash',
      playerId: 'deepseek-flash',
      harnessId: 'opencode',
      fieldId: 'local',
      lineage: 'deepseek-volume',
      upgrade: {
        profile: 'builder-plus',
        playerId: 'deepseek-pro',
        harnessId: 'forge-native',
        fieldId: 'local',
        lineage: 'deepseek-judgment',
      },
      emergency: {
        profile: 'builder-emergency',
        playerId: 'deepseek-pro',
        harnessId: 'forge-native',
        fieldId: 'local',
        lineage: 'deepseek-judgment',
      },
    },
  },
}

export type ForgeAssignmentGrade = 'default' | 'upgrade' | 'emergency'

export type ResolvedForgeAssignment = ForgeAssignmentVariant & {
  position: ForgePosition
  player: ForgePlayer
  harness: ForgeHarness
  field: ForgeField
}

function copyVariant(variant: ForgeAssignmentVariant): ForgeAssignmentVariant {
  return { ...variant }
}

function copyTeam(team: ForgeTeam): ForgeTeam {
  const assignments = {} as Record<ForgePosition, ForgePositionAssignment>
  for (const [position, assignment] of Object.entries(team.assignments) as Array<[
    ForgePosition,
    ForgePositionAssignment,
  ]>) {
    assignments[position] = {
      ...assignment,
      ...(assignment.upgrade ? { upgrade: copyVariant(assignment.upgrade) } : {}),
      ...(assignment.emergency ? { emergency: copyVariant(assignment.emergency) } : {}),
    }
  }
  return { ...team, assignments }
}

/** Host/operator overrides for the default position map. No silent fallback on invalid ids. */
export function configuredForgeTeam(
  env: Readonly<Record<string, string | undefined>> = process.env,
  base: ForgeTeam = DEFAULT_FORGE_TEAM,
): ForgeTeam {
  const team = copyTeam(base)
  for (const position of Object.keys(team.assignments) as ForgePosition[]) {
    const key = position.toUpperCase().replace(/[^A-Z0-9]+/g, '_')
    const assignment = team.assignments[position]
    const player = env[`FORGE_PLAYER_${key}`]?.trim()
    const harness = env[`FORGE_HARNESS_${key}`]?.trim()
    const field = env[`FORGE_FIELD_${key}`]?.trim()
    const profile = env[`FORGE_PROFILE_${key}`]?.trim()
    const lineage = env[`FORGE_LINEAGE_${key}`]?.trim()
    if (player) assignment.playerId = player
    if (harness) assignment.harnessId = harness as ForgeHarnessId
    if (field) assignment.fieldId = field as ForgeFieldId
    if (profile) assignment.profile = profile
    if (lineage) assignment.lineage = lineage
  }
  return team
}

export function assignmentVariant(
  position: ForgePosition,
  team: ForgeTeam = DEFAULT_FORGE_TEAM,
  grade: ForgeAssignmentGrade = 'default',
): ForgeAssignmentVariant {
  const base = team.assignments[position]
  if (!base) throw new Error(`unknown Forge position '${position}'`)
  if (grade === 'upgrade') {
    if (!base.upgrade) throw new Error(`Forge position '${position}' has no upgrade mapping`)
    return base.upgrade
  }
  if (grade === 'emergency') {
    if (!base.emergency) throw new Error(`Forge position '${position}' has no emergency mapping`)
    return base.emergency
  }
  return base
}

export function resolveForgeAssignment(
  position: ForgePosition,
  team: ForgeTeam = DEFAULT_FORGE_TEAM,
  grade: ForgeAssignmentGrade = 'default',
): ResolvedForgeAssignment {
  const variant = assignmentVariant(position, team, grade)
  const player = FORGE_PLAYERS[variant.playerId]
  if (!player) throw new Error(`unknown Forge player '${variant.playerId}'`)
  if (!player.ready) throw new Error(`Forge player '${player.id}' is not ready`)

  const harness = FORGE_HARNESSES[variant.harnessId]
  if (!harness) throw new Error(`unknown Forge harness '${variant.harnessId}'`)
  if (harness.status !== 'ready') {
    throw new Error(`Forge harness '${harness.id}' is not ready (${harness.status})`)
  }
  if (!harness.supportedProviders.includes(player.provider)) {
    throw new Error(
      `Forge harness '${harness.id}' cannot execute provider '${player.provider}' for player '${player.id}'`,
    )
  }

  const field = FORGE_FIELDS[variant.fieldId]
  if (!field) throw new Error(`unknown Forge field '${variant.fieldId}'`)
  if (!field.ready) throw new Error(`Forge field '${field.id}' is not ready`)

  return { ...variant, position, player, harness, field }
}

export function listForgeTeamAssignments(
  team: ForgeTeam = DEFAULT_FORGE_TEAM,
): ResolvedForgeAssignment[] {
  const positions: ForgePosition[] = ['scout', 'architect', 'lead', 'smith', 'inspector', 'assay']
  return positions.map((position) => resolveForgeAssignment(position, team))
}

export function allForgeAssignmentVariants(
  team: ForgeTeam = DEFAULT_FORGE_TEAM,
): Array<{ position: ForgePosition; variant: ForgeAssignmentVariant }> {
  const out: Array<{ position: ForgePosition; variant: ForgeAssignmentVariant }> = []
  for (const assignment of Object.values(team.assignments)) {
    out.push({ position: assignment.position, variant: assignment })
    if (assignment.upgrade) out.push({ position: assignment.position, variant: assignment.upgrade })
    if (assignment.emergency) out.push({ position: assignment.position, variant: assignment.emergency })
  }
  return out
}

/** Smith field is resolved from the same team map as Smith's model/harness. */
export function smithFieldFacts(
  team: ForgeTeam = DEFAULT_FORGE_TEAM,
): { id: string; ready: boolean } {
  const field = FORGE_FIELDS[team.assignments.smith.fieldId]
  return { id: field.id, ready: field.ready }
}
