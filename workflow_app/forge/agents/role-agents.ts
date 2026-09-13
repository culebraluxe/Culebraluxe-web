import { parseLeadRouting, reviewLeadProposal, leadRoutingFacts } from '../forge-lead-routing'
import type { RoutingContext } from '../forge-lead-routing'
import { findingsFromArchitectEvidence } from '../forge-shaping'
import { parseForgeEvidenceMarker } from '../forge-role-mapping'
import { ForgePhaseAgent } from './forge-phase-agent'
import type { ForgeGateEvidence } from '../forge-facts'
import type { RoleEffectPorts } from './ports'
import { parseArchitectHandoff, handoffToFindings } from './architect-handoff'
import { assessArchitectHandoff } from './architect/assess'
import { persistArchitectBrief } from './architect/persist'
import { benchIntentErrors } from './bench-intent'
import { collectAssayEvidence } from './assay-collect'
import { parseFailureClass } from '../qa-classify-line'
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

  /**
   * Write-on-exit brief. When the reply carries a handoff, the brief is built by
   * persistArchitectBrief: the handoff LINE is never sliced (slicing a structured
   * line is how a contract becomes a coin flip) while the prose around it is
   * capped. Without a handoff the parent's bounded raw brief stands.
   */
  architectBrief(raw: string): string | null {
    if (!this.isArchitect || !raw) return null
    const handoff = parseArchitectHandoff(raw)
    if (handoff) return persistArchitectBrief(raw, handoff)
    return super.architectBrief(raw)
  }

  collect(evidence: ForgeGateEvidence, raw: string, ports: RoleEffectPorts = {}): ForgeGateEvidence {
    const marked = parseForgeEvidenceMarker(raw)
    const next: ForgeGateEvidence = { ...evidence, ...marked }

    if (this.nodeId === 'research_architect') return next

    const handoff = parseArchitectHandoff(raw)
    if (handoff) {
      // The handoff is assessed BEFORE it is believed: duplicate ids, empty or
      // oversized scope, illegal paths, a required HOLD with no named risk, no
      // required findings, and — fail-closed — every claimed seam must exist on
      // the pinned baseRef. A failure records the reasons and leaves findings
      // UNSET so the parent's architect-plan gate HOLDs with text the self-heal
      // reprompt can act on. (Previously the reasons were computed and thrown
      // away, and a written brief could let a failed handoff pass the gate.)
      const assessment = assessArchitectHandoff(handoff, {
        ...(ports.existsOnBaseRef ? { existsOnBaseRef: ports.existsOnBaseRef } : {}),
      })
      if (!assessment.ok) {
        return { ...next, deliverableRejection: assessment.reasons.join('; ') }
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
    if (!review.ok) {
      // The reviewer's errors ARE the reason this routing was refused. Without them
      // the self-heal only says "you did not deliver lead-decision", and the retry
      // has to guess what was wrong. Observed live 2026-09-13: attempt 1 proposed a
      // real SOLO plan and was refused for a structural reason the model never saw.
      return { ...evidence, deliverableRejection: review.errors.join('; ') }
    }

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

  /**
   * The Smith lane's evidence comes from the RUNNER's door 2 — the candidate SHA
   * and the live scope contract — not from this hook. `assessSmithExit` (the Smith
   * exit gate) is called there, where the worktree diff and the real
   * `SmithExecutionContract` exist, so collect only marshals markers.
   */
  collect(evidence: ForgeGateEvidence, raw: string, _ports: RoleEffectPorts = {}): ForgeGateEvidence {
    return { ...evidence, ...parseForgeEvidenceMarker(raw) }
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
    // The FAILURE_CLASS: line is THIS lane's line contract. It feeds the ENGINE
    // enum (CODE_DEFECT…), which is deliberately NOT the taxonomy in
    // failure-classifier.ts (BAD_IMPLEMENTATION…). Two enums, two jobs — not merged.
    const failureClass = parseFailureClass(raw)
    return { ...evidence, ...marked, ...(failureClass ? { failureClass } : {}) }
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
