import { ForgePhaseAgent } from './forge-phase-agent'
import { forgeRoleNodePlan } from '../forge-role-mapping'

// ---------------------------------------------------------------------------
// ENG-FORGE-PHASE-AGENT Phase 2 — concrete role subclasses. Each extends the
// shared ForgePhaseAgent (marshal/deliverable contract + lead shape override).
// Subclasses are thin today because most phase behavior is already shared; they
// exist as the real seam for role-specific instructions/deliverable enforcement
// as each role matures (e.g. ScoutAgent owns the packet contract, ArchitectAgent
// owns disposition, SmithAgent owns the candidate SHA).
// ---------------------------------------------------------------------------

export class ScoutAgent extends ForgePhaseAgent {
  readonly roleName = 'scout'
}
export class ArchitectAgent extends ForgePhaseAgent {
  readonly roleName = 'architect'
}
export class LeadAgent extends ForgePhaseAgent {
  readonly roleName = 'lead'
}
export class SmithAgent extends ForgePhaseAgent {
  readonly roleName = 'smith'
}
export class QAAgent extends ForgePhaseAgent {
  readonly roleName = 'assay'
}
export class DevOpsAgent extends ForgePhaseAgent {
  readonly roleName = 'dev_ops'
}

/** Resolve the concrete role agent for an engine node (by its lane). */
export function forgeAgentFor(nodeId: string): ForgePhaseAgent {
  const lane = forgeRoleNodePlan(nodeId).lane
  switch (lane) {
    case 'scout':
      return new ScoutAgent(nodeId)
    case 'architect':
      return new ArchitectAgent(nodeId)
    case 'lead':
      return new LeadAgent(nodeId)
    case 'smith':
      return new SmithAgent(nodeId)
    case 'assay':
      return new QAAgent(nodeId)
    case 'dev_ops':
      return new DevOpsAgent(nodeId)
    default:
      return new ForgePhaseAgent(nodeId)
  }
}
