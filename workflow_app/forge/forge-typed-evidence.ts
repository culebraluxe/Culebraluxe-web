import type { AgentRunEvidence } from '../../agent-runtime/types'
import type { ForgeGateEvidence } from './forge-facts'
import { parseForgeEvidenceMarker } from './forge-role-mapping'

const TYPED_REQUIRED_NODES = new Set([
  'research_scout',
  'feature_scout',
  'diagnose_scout',
  'repair_scout',
  'research_architect',
  'architect',
  'repair_architect',
  'lead_pre',
  'lead_solo_implement',
  'lead_post',
  'failure_classifier',
  'repair_devops',
])

export function forgeNodeRequiresTypedGateEvidence(nodeId: string): boolean {
  return TYPED_REQUIRED_NODES.has(nodeId)
}

export function readTypedGateEvidence(
  result: Pick<AgentRunEvidence, 'notes' | 'testsSummary'> & {
    gateEvidence?: ForgeGateEvidence | null
  },
): ForgeGateEvidence | null {
  const typed = result.gateEvidence
  if (typed && Object.keys(typed).length > 0) return typed
  return null
}

/** Legacy marker path — test-tagged only. Production routing must not rely on it. */
export function readLegacyMarkerGateEvidence(
  result: Pick<AgentRunEvidence, 'notes' | 'testsSummary'>,
): ForgeGateEvidence {
  return parseForgeEvidenceMarker([result.notes, result.testsSummary].filter(Boolean).join('\n'))
}

export function missingTypedGateHoldEvidence(nodeId: string): ForgeGateEvidence {
  return {
    failureClass: 'UNKNOWN_CAUSE',
    resumeTarget: nodeId.includes('scout')
      ? 'SCOUT'
      : nodeId.includes('architect')
        ? 'ARCHITECT'
        : nodeId.includes('lead')
          ? 'LEAD'
          : 'DEV_OPS',
  }
}
