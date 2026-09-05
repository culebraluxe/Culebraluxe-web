import type { AgentCapability } from './capabilities'
import type { AgentRole } from './types'
import type { ForgePosition } from './team'

export type LaneId =
  | 'scout'
  | 'architect'
  | 'lead'
  | 'smith'
  | 'inspector'
  | 'assay'
  | 'archive'
  | 'night'
  | 'dev_ops'

export type ModelLineage = string

export type ToolPolicy = 'read-only' | 'plan-only' | 'write' | 'detach'

/**
 * A lane defines behavior only. Model/profile/provider/harness/field selection
 * belongs exclusively to ForgeTeam mapping (team.ts).
 */
export interface LaneBinding {
  lane: LaneId
  position: ForgePosition
  role: AgentRole
  maxSteps: number
  toolPolicy: ToolPolicy
  requiredCapabilities: AgentCapability[]
  openClaudeKey?: string
}

export const READ_CAPABILITIES: AgentCapability[] = [
  'workspace.fs.read',
  'git.status',
  'git.diff',
  'git.history',
  'storyboard.read',
  'config.read',
  'host.logs',
]

export const ASSAY_CAPABILITIES: AgentCapability[] = [
  ...READ_CAPABILITIES,
  'host.tests',
  'host.typecheck',
]

export const WRITE_CAPABILITIES: AgentCapability[] = [
  ...READ_CAPABILITIES,
  'workspace.fs.write',
  'workspace.fs.delete',
  'git.commit',
  'data.db.read',
  'data.db.write',
  'data.schema.migrate',
  'storyboard.write',
  'host.exec',
  'host.process',
  'host.tests',
  'host.typecheck',
  'host.build',
  'host.lint',
  'host.repo-scripts',
]

export const DEFAULT_LANES: Record<LaneId, LaneBinding> = {
  scout: {
    lane: 'scout',
    position: 'scout',
    role: 'scout',
    maxSteps: 12,
    toolPolicy: 'read-only',
    requiredCapabilities: READ_CAPABILITIES,
    openClaudeKey: 'Explore',
  },
  architect: {
    lane: 'architect',
    position: 'architect',
    role: 'architect',
    maxSteps: 1,
    toolPolicy: 'plan-only',
    requiredCapabilities: ['storyboard.read', 'config.read'],
    openClaudeKey: 'Plan',
  },
  lead: {
    lane: 'lead',
    position: 'lead',
    role: 'lead',
    maxSteps: 20,
    // PRE is made read-only by execution policy. The same Lead position needs
    // write capability for SOLO implementation and POST integration.
    toolPolicy: 'write',
    requiredCapabilities: WRITE_CAPABILITIES,
    openClaudeKey: 'general-purpose',
  },
  smith: {
    lane: 'smith',
    position: 'smith',
    role: 'builder',
    maxSteps: 40,
    toolPolicy: 'write',
    requiredCapabilities: WRITE_CAPABILITIES,
    openClaudeKey: 'general-purpose',
  },
  inspector: {
    lane: 'inspector',
    position: 'inspector',
    role: 'reviewer',
    maxSteps: 8,
    toolPolicy: 'read-only',
    requiredCapabilities: [...READ_CAPABILITIES, 'git.diff', 'host.tests'],
    openClaudeKey: 'code-reviewer',
  },
  assay: {
    lane: 'assay',
    position: 'assay',
    role: 'verifier',
    maxSteps: 6,
    toolPolicy: 'read-only',
    requiredCapabilities: ASSAY_CAPABILITIES,
    openClaudeKey: 'verification',
  },
  archive: {
    lane: 'archive',
    position: 'archive',
    role: 'architect',
    maxSteps: 3,
    toolPolicy: 'plan-only',
    requiredCapabilities: ['storyboard.read', 'config.read'],
  },
  night: {
    lane: 'night',
    position: 'night',
    role: 'builder',
    maxSteps: 40,
    toolPolicy: 'detach',
    requiredCapabilities: WRITE_CAPABILITIES,
  },
  dev_ops: {
    lane: 'dev_ops',
    position: 'dev_ops',
    role: 'dev_ops',
    maxSteps: 20,
    toolPolicy: 'write',
    requiredCapabilities: WRITE_CAPABILITIES,
    openClaudeKey: 'general-purpose',
  },
}

export const LANE_ORDER: LaneId[] = [
  'scout',
  'architect',
  'lead',
  'smith',
  'inspector',
  'assay',
  'archive',
  'night',
  'dev_ops',
]
