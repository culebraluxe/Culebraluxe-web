import { parseLeadRouting, reviewLeadProposal, leadRoutingFacts } from '../forge-lead-routing'
import type { RoutingContext } from '../forge-lead-routing'
import { findingsFromArchitectEvidence } from '../forge-shaping'
import { parseForgeEvidenceMarker } from '../forge-role-mapping'
import { ForgePhaseAgent } from './forge-phase-agent'
import type { ForgeGateEvidence } from '../forge-facts'
import type { RoleEffectPorts } from './ports'
import { parseArchitectHandoff, handoffToFindings } from './architect-handoff'
import { benchIntentErrors } from './bench-intent'
import { collectSmithEvidence } from './smith-collect'
import { collectAssayEvidence } from './assay-collect'
import { forgeRoleNodePlan } from '../forge-role-mapping'

// ---------------------------------------------------------------------------
// REPLACE role-agents.ts
// Logic lives in collect() overrides. Parent gates stay the decider.
// ---------------------------------------------------------------------------

export class ScoutAgent extends ForgePhaseAgent {
  readonly roleName = 'scout'

  collect(evidence: ForgeGateEvidence, raw: string, ports: RoleEffectPorts = {}): ForgeGateEvidence {
    const next = super.collect(evidence, raw, ports)
    const marked = parseForgeEvidenceMarker(raw)
    return { ...next, ...marked }
  }
}

export class ArchitectAgent extends ForgePhaseAgent {
  readonly roleName = 'architect'

  collect(evidence: ForgeGateEvidence, raw: string, ports: RoleEffectPorts = {}): ForgeGateEvidence {
    const marked = parseForgeEvidenceMarker(raw)
    const next: ForgeGateEvidence = { ...evidence, ...marked }

    if (this.nodeId === 'research_architect') return next

    const handoff = parseArchitectHandoff(raw)
    if (handoff) {
      if (ports.existsOnBaseRef && handoff.baseRef) {
        const missing = handoff.findings.flatMap((f) =>
          f.scope.filter((p) => {
            const file = p.split('#')[0]
            return !ports.existsOnBaseRef!(handoff.baseRef, file)
          }),
        )
        if (missing.length) return next
      }
      next.findings = handoffToFindings(handoff)
      return next
    }

    const legacy = findingsFromArchitectEvidence(raw)
    if (legacy.length) next.findings = legacy
    return next
  }
}

export class LeadAgent extends ForgePhaseAgent {
  readonly roleName = 'lead'

  collect(evidence: ForgeGateEvidence, raw: string, ports: RoleEffectPorts = {}): ForgeGateEvidence {
    if (this.plan.leadPhase !== 'pre' || this.nodeId === 'failure_classifier') {
      return { ...evidence, ...parseForgeEvidenceMarker(raw) }
    }

    const findings = evidence.findings ?? []
    const context: RoutingContext = {
      findings: findings.map((f) => ({
        id: f.id,
        required: f.required,
        hint: f.hint,
        seams: f.seams,
      })),
      evidenceRefs: ports.evidenceRefs ?? [],
      splitEnabled: ports.splitEnabled ?? false,
      maxSmiths: ports.maxSmiths ?? 1,
      allowedProofs: ports.allowedProofs ?? [],
    }
    const rawProposal = parseLeadRouting(raw)
    const review = reviewLeadProposal(rawProposal, context)
    if (!review.ok) return evidence

    const bench = benchIntentErrors(review.proposal.decision, ports.benchIntent)
    if (bench.length) return evidence

    const facts = leadRoutingFacts(review)
    return {
      ...evidence,
      leadDecision: facts.leadDecision,
      splitCount: facts.splitCount || undefined,
      leadRouting: facts.leadRouting,
    }
  }
}

export class SmithAgent extends ForgePhaseAgent {
  readonly roleName = 'smith'

  collect(evidence: ForgeGateEvidence, raw: string, ports: RoleEffectPorts = {}): ForgeGateEvidence {
    const marked = parseForgeEvidenceMarker(raw)
    const hasAssignment = Boolean(
      evidence.leadRouting &&
        typeof evidence.leadRouting === 'object' &&
        Array.isArray((evidence.leadRouting as { assignments?: unknown[] }).assignments) &&
        ((evidence.leadRouting as { assignments: unknown[] }).assignments.length > 0),
    )
    return collectSmithEvidence(this.nodeId, { ...evidence, ...marked }, ports, hasAssignment)
  }
}

export class QAAgent extends ForgePhaseAgent {
  readonly roleName = 'assay'

  collect(evidence: ForgeGateEvidence, raw: string, ports: RoleEffectPorts = {}): ForgeGateEvidence {
    const marked = parseForgeEvidenceMarker(raw)
    return collectAssayEvidence({ ...evidence, ...marked }, ports)
  }
}

export class DevOpsAgent extends ForgePhaseAgent {
  readonly roleName = 'dev_ops'

  collect(evidence: ForgeGateEvidence, raw: string, ports: RoleEffectPorts = {}): ForgeGateEvidence {
    const marked = parseForgeEvidenceMarker(raw)
    const next: ForgeGateEvidence = { ...evidence, ...marked }
    if (ports.deploymentDeferredToBatch != null) {
      next.deploymentDeferredToBatch = ports.deploymentDeferredToBatch
      return next
    }
    const receipt = ports.releaseEvidence
    if (!receipt?.success || !receipt.receiptId.trim()) return next
    if (receipt.kind === 'production_verification') {
      next.productionVerificationReceipt = receipt.receiptId.trim()
      if (receipt.artifactSha) next.productionVerifiedSha = receipt.artifactSha.toLowerCase()
    } else {
      next.deploymentReceipt = receipt.receiptId.trim()
      if (receipt.artifactSha) next.deployedSha = receipt.artifactSha.toLowerCase()
    }
    return next
  }
}

/** ADD class. failure_classifier is not Lead even though the mapping lane is lead. */
export class FailureClassifierAgent extends ForgePhaseAgent {
  readonly roleName = 'failure_classifier'

  collect(evidence: ForgeGateEvidence, raw: string, _ports: RoleEffectPorts = {}): ForgeGateEvidence {
    const marked = parseForgeEvidenceMarker(raw)
    return { ...evidence, ...marked }
  }
}

export function forgeAgentFor(nodeId: string): ForgePhaseAgent {
  if (nodeId === 'failure_classifier') return new FailureClassifierAgent(nodeId)
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
