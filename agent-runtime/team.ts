import type { AgentCapability } from './capabilities'
import { ASSAY_CAPABILITIES, READ_CAPABILITIES, WRITE_CAPABILITIES } from './lanes'

/**
 * Forge vocabulary boundary:
 * - Position = SDLC responsibility.
 * - Player = model/provider identity available to fill a position.
 * - Harness = software seam used to drive that player.
 * - Field = execution environment/topology where the player runs.
 *
 * Swarm/parallelism belongs to Field. It is never a Player or Position.
 */
export type ForgePosition = 'scout' | 'architect' | 'smith' | 'assay'

export type ForgeHarnessId =
  | 'forge-native'
  | 'opencode'
  | 'pi'
  | 'warp-agent'

export type ForgeFieldId = 'local' | 'warp-swarm'

export type ForgeHarnessStatus = 'ready' | 'unconfigured' | 'interactive-only'

export interface ForgeHarness {
  id: ForgeHarnessId
  name: string
  status: ForgeHarnessStatus
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
  model: string
  harness: ForgeHarnessId
  capabilities: AgentCapability[]
  ready: boolean
}

export interface ForgePositionAssignment {
  position: ForgePosition
  playerId: string
  fieldId: ForgeFieldId
  /** Existing logical runtime profile. Keeps V3 lane contracts provider-neutral. */
  profile: string
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
    description: 'Current local Forge runtime/harness boundary.',
  },
  opencode: {
    id: 'opencode',
    name: 'OpenCode',
    status: 'unconfigured',
    description: 'Reserved harness connection point; no runtime is configured yet.',
  },
  pi: {
    id: 'pi',
    name: 'Pi',
    status: 'unconfigured',
    description: 'Reserved harness connection point; no runtime is configured yet.',
  },
  'warp-agent': {
    id: 'warp-agent',
    name: 'Warp Agent',
    status: 'interactive-only',
    description: 'Warp CLI is installed but has no approved headless Forge contract yet.',
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
    description: 'Reserved future parallel field. Architect-controlled decomposition is required before activation.',
  },
}

/**
 * Current factual roster. Only players the operator has usable inference for
 * are marked ready. Additional vendors are added here only after credentials
 * and a harness path are actually qualified.
 */
export const FORGE_PLAYERS: Record<string, ForgePlayer> = {
  'deepseek-flash': {
    id: 'deepseek-flash',
    name: 'DeepSeek Flash',
    provider: 'deepseek',
    model: 'Flash',
    harness: 'forge-native',
    capabilities: WRITE_CAPABILITIES,
    ready: true,
  },
  'deepseek-pro': {
    id: 'deepseek-pro',
    name: 'DeepSeek Pro',
    provider: 'deepseek',
    model: 'Pro',
    harness: 'forge-native',
    capabilities: [...READ_CAPABILITIES, ...ASSAY_CAPABILITIES],
    ready: true,
  },
}

/**
 * The default sequential team deliberately mirrors today's working Forge.
 * Architect is represented now even though A1 still does not auto-queue it.
 */
export const DEFAULT_FORGE_TEAM: ForgeTeam = {
  id: 'default',
  name: 'Forge Default',
  assignments: {
    scout: {
      position: 'scout',
      playerId: 'deepseek-flash',
      fieldId: 'local',
      profile: 'scout-volume',
    },
    architect: {
      position: 'architect',
      playerId: 'deepseek-pro',
      fieldId: 'local',
      profile: 'architect-pro',
    },
    smith: {
      position: 'smith',
      playerId: 'deepseek-flash',
      fieldId: 'local',
      profile: 'builder-flash',
    },
    assay: {
      position: 'assay',
      playerId: 'deepseek-pro',
      fieldId: 'local',
      profile: 'verifier-mini',
    },
  },
}

export type ResolvedForgeAssignment = ForgePositionAssignment & {
  player: ForgePlayer
  harness: ForgeHarness
  field: ForgeField
}

export function resolveForgeAssignment(
  position: ForgePosition,
  team: ForgeTeam = DEFAULT_FORGE_TEAM,
): ResolvedForgeAssignment {
  const assignment = team.assignments[position]
  const player = FORGE_PLAYERS[assignment.playerId]
  if (!player) throw new Error(`unknown Forge player '${assignment.playerId}'`)
  if (!player.ready) throw new Error(`Forge player '${player.id}' is not ready`)

  const harness = FORGE_HARNESSES[player.harness]
  if (!harness) throw new Error(`unknown Forge harness '${player.harness}'`)
  if (harness.status !== 'ready') {
    throw new Error(`Forge harness '${harness.id}' is not ready (${harness.status})`)
  }

  const field = FORGE_FIELDS[assignment.fieldId]
  if (!field) throw new Error(`unknown Forge field '${assignment.fieldId}'`)
  if (!field.ready) throw new Error(`Forge field '${field.id}' is not ready`)

  return { ...assignment, player, harness, field }
}

export function listForgeTeamAssignments(
  team: ForgeTeam = DEFAULT_FORGE_TEAM,
): ResolvedForgeAssignment[] {
  const positions: ForgePosition[] = ['scout', 'architect', 'smith', 'assay']
  return positions.map((position) => resolveForgeAssignment(position, team))
}

/**
 * ENG-FORGE-V4-08 — non-throwing field fact for the position that owns the
 * Smith/builder lane. The execution-contract gate uses this instead of
 * `resolveForgeAssignment` so an unavailable field is REPORTED as a concrete
 * rejection reason rather than thrown as an exception.
 */
export function smithFieldFacts(): { id: string; ready: boolean } {
  const field = FORGE_FIELDS[DEFAULT_FORGE_TEAM.assignments.smith.fieldId]
  return { id: field.id, ready: field.ready }
}
