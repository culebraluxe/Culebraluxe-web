import type { AgentCapability } from './capabilities'
import type { AgentRole, ModelProfile } from './types'

export type LaneId =
  | 'scout'
  | 'architect'
  | 'lead'
  | 'smith'
  | 'inspector'
  | 'assay'
  | 'archive'
  | 'night'

export type ModelLineage = string

export type ToolPolicy = 'read-only' | 'plan-only' | 'write' | 'detach'

export interface LaneBinding {
  lane: LaneId
  role: AgentRole
  profile: ModelProfile
  upgradeProfile?: ModelProfile
  emergencyProfile?: ModelProfile
  lineage: ModelLineage
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
    role: 'scout',
    profile: 'scout-volume',
    lineage: 'volume-lab',
    maxSteps: 12,
    toolPolicy: 'read-only',
    requiredCapabilities: READ_CAPABILITIES,
    openClaudeKey: 'Explore',
  },
  architect: {
    lane: 'architect',
    role: 'architect',
    profile: 'architect-pro',
    lineage: 'judgment-lab',
    maxSteps: 1,
    toolPolicy: 'plan-only',
    requiredCapabilities: ['storyboard.read', 'config.read'],
    openClaudeKey: 'Plan',
  },
  lead: {
    lane: 'lead',
    role: 'lead',
    profile: 'lead-pro',
    lineage: 'judgment-lab',
    maxSteps: 20,
    // PRE is made read-only by execution policy. The same Lead position needs
    // write capability for SOLO implementation and POST integration.
    toolPolicy: 'write',
    requiredCapabilities: WRITE_CAPABILITIES,
    openClaudeKey: 'general-purpose',
  },
  smith: {
    lane: 'smith',
    role: 'builder',
    profile: 'builder-flash',
    upgradeProfile: 'builder-plus',
    emergencyProfile: 'builder-emergency',
    lineage: 'volume-lab',
    maxSteps: 40,
    toolPolicy: 'write',
    requiredCapabilities: WRITE_CAPABILITIES,
    openClaudeKey: 'general-purpose',
  },
  inspector: {
    lane: 'inspector',
    role: 'reviewer',
    profile: 'reviewer-other',
    lineage: 'judgment-lab',
    maxSteps: 8,
    toolPolicy: 'read-only',
    requiredCapabilities: [...READ_CAPABILITIES, 'git.diff', 'host.tests'],
    openClaudeKey: 'code-reviewer',
  },
  assay: {
    lane: 'assay',
    role: 'verifier',
    profile: 'verifier-mini',
    lineage: 'volume-lab',
    maxSteps: 6,
    toolPolicy: 'read-only',
    requiredCapabilities: ASSAY_CAPABILITIES,
    openClaudeKey: 'verification',
  },
  archive: {
    lane: 'archive',
    role: 'architect',
    profile: 'architect-pro',
    lineage: 'judgment-lab',
    maxSteps: 3,
    toolPolicy: 'plan-only',
    requiredCapabilities: ['storyboard.read', 'config.read'],
  },
  night: {
    lane: 'night',
    role: 'builder',
    profile: 'builder-flash',
    upgradeProfile: 'builder-plus',
    lineage: 'volume-lab',
    maxSteps: 40,
    toolPolicy: 'detach',
    requiredCapabilities: WRITE_CAPABILITIES,
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
]
