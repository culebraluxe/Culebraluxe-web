import { findingsFromArchitectEvidence } from '@/legacy/workflow_app/forge/forge-shaping'
import { parseForgeEvidenceMarker } from '@/legacy/workflow_app/forge/forge-role-mapping'
import { ForgePhaseAgent } from '@/legacy/workflow_app/forge/agents/forge-phase-agent'
import type { ForgeGateEvidence } from '@/legacy/workflow_app/forge/forge-facts'
import type { RoleEffectPorts } from '@/legacy/workflow_app/forge/agents/ports'
import { parseArchitectHandoff, handoffToFindings } from '@/legacy/workflow_app/forge/agents/architect-handoff'
import { ARCHITECT_HANDOFF_MISSING, assessArchitectHandoff } from '@/legacy/workflow_app/forge/agents/architect/assess'
import { persistArchitectBrief } from '@/legacy/workflow_app/forge/agents/architect/persist'
import { collectAssayEvidence } from '@/legacy/workflow_app/forge/agents/assay-collect'
import { parseFailureClass } from '@/legacy/workflow_app/forge/qa-classify-line'
import { forgeRoleNodePlan } from '@/legacy/workflow_app/forge/forge-role-mapping'

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
    if (legacy.length) {
      next.findings = legacy
      return next
    }
    // NEITHER contract produced a plan. Say so here, in this attempt's rejection
    // sidecar, so the bounded self-heal reprompt names the marker the directive
    // actually asked for; this lane's own gate then HOLDs on 'architect-plan'.
    //
    // Before this, the constant was UNREACHABLE: assessArchitectHandoff is only
    // called for a handoff that PARSED, so a reply with no plan left the rejection
    // undefined and the run was held by the runner's LEGACY fallback instead —
    // whose text asks for FORGE_FINDINGS_JSON. A retry therefore read a correct
    // directive, failed, and was told to emit the fallback marker, teaching itself
    // the wrong contract on the one attempt it gets.
    return { ...next, deliverableRejection: ARCHITECT_HANDOFF_MISSING }
  }
}

export class LeadAgent extends ForgePhaseAgent {
  readonly roleName = 'lead'

  collect(evidence: ForgeGateEvidence, raw: string, ports: RoleEffectPorts = {}): ForgeGateEvidence {
    if (this.plan.leadPhase !== 'pre' || this.nodeId === 'failure_classifier') {
      const marked = parseForgeEvidenceMarker(raw)
      const { leadDecision: _d, splitCount: _s, ...rest } = marked
      return { ...evidence, ...rest }
    }
    // PRE: decision lives in forge_role_contract / forge_role_plan rows.
    // Chat JSON cannot set leadDecision. The runner reviews fields only.
    void raw
    void ports
    return evidence
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
    switch (receipt.kind) {
      case 'deployment':
        next.deploymentReceipt = receipt.receiptId.trim()
        if (receipt.artifactSha) next.deployedSha = receipt.artifactSha.toLowerCase()
        break
      case 'production_verification':
        next.productionVerificationReceipt = receipt.receiptId.trim()
        if (receipt.artifactSha) next.productionVerifiedSha = receipt.artifactSha.toLowerCase()
        break
      case 'integration':
      default:
        break
    }
    return next
  }
}

/** ADD class. failure_classifier is not Lead even though the mapping lane is lead. */
export class FailureClassifierAgent extends ForgePhaseAgent {
  readonly roleName = 'failure_classifier'

  collect(evidence: ForgeGateEvidence, raw: string, _ports: RoleEffectPorts = {}): ForgeGateEvidence {
    const marked = parseForgeEvidenceMarker(raw)
    const failureClass = parseFailureClass(raw)
    const classifierLabel =
      failureClass ?? ((marked.failureClass as ForgeGateEvidence['failureClass'] | undefined) ?? null)
    // A STAGE THAT ALREADY RECORDED ITS CLASS KEEPS IT (captain, 2026-09-16).
    //
    // This collect() is the LAST writer before the evidence row is persisted, so an unguarded
    // write here is what actually replaced PUBLISH_CONFLICT with ENVIRONMENT on
    // ENG-FORGE-RECEIPT-KIND-01 — after the publisher had already recorded the accurate class
    // with the stage that failed. When a release stage recorded both a class and the stage,
    // that class is the record of why and the classifier's own label is kept as metadata.
    // `stageFailureClass` is the durable copy because a marker/typed label may have overwritten
    // `failureClass` earlier in the runner's merge. With no stage-recorded class the classifier
    // label stands exactly as it did before.
    const stageRecorded = evidence.failedReleaseStage
      ? (evidence.stageFailureClass ?? evidence.failureClass)
      : undefined
    if (stageRecorded) {
      return {
        ...evidence,
        ...marked,
        failureClass: stageRecorded,
        ...(classifierLabel ? { classifierFailureClass: classifierLabel } : {}),
      }
    }
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
