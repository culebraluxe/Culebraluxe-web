import type { ForgeGateEvidence } from '../forge-facts'
import {
  forgeRoleNodePlan,
  type ForgeRoleNodePlan,
} from '../forge-role-mapping'
import { findingsFromArchitectEvidence } from '../forge-shaping'
import { lastMachineLine } from './architect-handoff'
import type { RoleEffectPorts } from './ports'

// ---------------------------------------------------------------------------
// ENG-FORGE-PHASE-AGENT — REPLACE.
//
// Grown: collect(evidence, raw, ports) is the subclass hook that FILLS evidence.
// Unchanged: missingDeliverables / routingDecisionMissing remain the decider.
// ---------------------------------------------------------------------------

const SCOUT_NODES = new Set(['research_scout', 'feature_scout', 'diagnose_scout', 'repair_scout'])
const ARCHITECT_NODES = new Set(['architect', 'repair_architect', 'research_architect'])

const RESEARCH_DISPOSITIONS = new Set(['IMPLEMENT', 'ARCHIVE', 'HOLD'])
const LEAD_DECISIONS = new Set(['SMITH', 'SPLIT', 'HOLD', 'SOLO'])
const FAILURE_CLASSES = new Set([
  'CODE_DEFECT',
  'TEST_DEFECT',
  'ARCHITECTURE_GAP',
  'REQUIREMENTS_GAP',
  'UNKNOWN_CAUSE',
  'ENVIRONMENT',
  'MIGRATION',
  'PUBLISH_CONFLICT',
  'DEPLOYMENT',
  'PRODUCTION_SMOKE',
  'HOLD',
])

export type PhaseDeliverableKind =
  | 'scout-packet'
  | 'architect-plan'
  | 'lead-decision'
  | 'failure-class'
  | 'smith-candidate'
  | 'qa-verdict'
  | 'devops-receipt'
  | 'none'

export function rawRoleOutput(notes?: string | null, testsSummary?: string | null): string {
  return [notes, testsSummary].filter(Boolean).join('\n').trim()
}

export function parseDeliverableRepromptBudget(raw: string | undefined): number {
  const n = Number.parseInt(raw ?? '', 10)
  return Number.isFinite(n) && n >= 0 ? n : 1
}

export function deliverableEnforcementEnabled(raw: string | undefined): boolean {
  if (raw == null) return true
  const v = raw.trim().toLowerCase()
  return v !== '0' && v !== 'false' && v !== 'off'
}

export function buildSelfHealDirective(
  nodeId: string,
  missing: readonly string[],
  evidenceInstruction: string | null | undefined,
): string {
  const fields = missing.length > 0 ? missing.join(', ') : '(unreported)'
  return (
    `SELF-HEAL REPROMPT (node ${nodeId}): this run was HELD because it did not deliver: ${fields}.\n` +
    (evidenceInstruction
      ? `The role's required structured output is: ${evidenceInstruction}\n`
      : '') +
    'Re-run this role. You MUST end your reply by emitting the required structured evidence so those fields are present and valid.'
  )
}

const PROSE_CAP = 4000

function withPreservedMarker(prose: string, raw: string, prefixes: string[]): string {
  const clipped =
    prose.length > PROSE_CAP
      ? `${prose.slice(0, PROSE_CAP)}\n[prose truncated; machine line preserved below]`
      : prose
  const markers = prefixes.map((p) => lastMachineLine(raw, p)).filter((l): l is string => !!l)
  if (!markers.length) return clipped
  return `${clipped}\n\n${markers.join('\n')}`
}

export class ForgePhaseAgent {
  readonly nodeId: string
  readonly plan: ForgeRoleNodePlan
  readonly isScout: boolean
  readonly isArchitect: boolean

  constructor(nodeId: string) {
    this.nodeId = nodeId
    this.plan = forgeRoleNodePlan(nodeId)
    this.isScout = SCOUT_NODES.has(nodeId)
    this.isArchitect = ARCHITECT_NODES.has(nodeId)
  }

  deliverableKind(): PhaseDeliverableKind {
    if (this.nodeId === 'failure_classifier') return 'failure-class'
    if (this.isScout) return 'scout-packet'
    if (this.isArchitect) return 'architect-plan'
    if (this.plan.lane === 'lead' && this.plan.leadPhase === 'pre' && this.nodeId !== 'failure_classifier') {
      return 'lead-decision'
    }
    if (this.plan.lane === 'smith') return 'smith-candidate'
    if (this.plan.lane === 'assay') return 'qa-verdict'
    if (this.plan.lane === 'dev_ops') return 'devops-receipt'
    return 'none'
  }

  marshalFindings(evidence: ForgeGateEvidence, raw: string): ForgeGateEvidence {
    if (!this.isScout && !this.isArchitect) return evidence
    const parsed = findingsFromArchitectEvidence(raw)
    if (parsed.length > 0) evidence.findings = parsed
    return evidence
  }

  scoutPacket(raw: string): string | null {
    if (!this.isScout || !raw) return null
    return withPreservedMarker(
      `Scout research packet (engine node ${this.nodeId}):\n${raw}`,
      raw,
      ['FORGE_FINDINGS_JSON:', 'FORGE_EVIDENCE_JSON:'],
    )
  }

  architectBrief(raw: string): string | null {
    if (!this.isArchitect || !raw) return null
    return withPreservedMarker(
      `Architect plan (engine node ${this.nodeId}):\n${raw}`,
      raw,
      ['FORGE_ARCHITECT_HANDOFF:', 'FORGE_FINDINGS_JSON:', 'FORGE_EVIDENCE_JSON:'],
    )
  }

  storyDeliverable(raw: string): { field: 'context_refs' | 'architect_brief'; text: string } | null {
    if (this.isScout) {
      const packet = this.scoutPacket(raw)
      return packet ? { field: 'context_refs', text: packet } : null
    }
    if (this.isArchitect) {
      const brief = this.architectBrief(raw)
      return brief ? { field: 'architect_brief', text: brief } : null
    }
    return null
  }

  /**
   * Subclass hook. Fill evidence only. Do not HOLD here — leave the field
   * empty and let missingDeliverables / routingDecisionMissing decide.
   */
  collect(
    evidence: ForgeGateEvidence,
    raw: string,
    _ports: RoleEffectPorts = {},
  ): ForgeGateEvidence {
    return this.marshalFindings({ ...evidence }, raw)
  }

  missingDeliverables(
    evidence: ForgeGateEvidence,
    raw: string,
    scoutContextRefsSet: boolean,
    architectBriefSet = false,
  ): string[] {
    const kind = this.deliverableKind()
    const missing: string[] = []
    switch (kind) {
      case 'scout-packet':
        if (!scoutContextRefsSet && !this.scoutPacket(raw)) missing.push('scout-packet')
        break
      case 'architect-plan':
        if (
          !architectBriefSet &&
          evidence.researchDisposition == null &&
          !(evidence.findings && evidence.findings.length > 0)
        ) {
          missing.push('architect-plan')
        }
        break
      case 'lead-decision':
        if (evidence.leadDecision == null) missing.push('lead-decision')
        break
      case 'failure-class':
        if (!FAILURE_CLASSES.has(String(evidence.failureClass ?? ''))) {
          missing.push('failure-class')
        }
        break
      case 'smith-candidate':
        if (evidence.candidateSha == null) missing.push('smith-candidate')
        break
      case 'qa-verdict':
        if (evidence.qaPassed === undefined) missing.push('qa-verdict')
        break
      case 'devops-receipt':
        if (
          evidence.deploymentReceipt == null &&
          evidence.productionVerificationReceipt == null &&
          evidence.deploymentDeferredToBatch == null
        ) {
          missing.push('devops-receipt')
        }
        break
      default:
        break
    }
    return missing
  }

  routingDecisionMissing(evidence: ForgeGateEvidence): string | null {
    if (this.nodeId === 'failure_classifier') {
      return FAILURE_CLASSES.has(String(evidence.failureClass ?? ''))
        ? null
        : 'failure_class'
    }
    if (this.nodeId === 'research_architect') {
      return RESEARCH_DISPOSITIONS.has(String(evidence.researchDisposition ?? ''))
        ? null
        : 'research_disposition'
    }
    if (this.plan.lane === 'lead' && this.plan.leadPhase === 'pre' && this.nodeId !== 'failure_classifier') {
      const d = String(evidence.leadDecision ?? '')
      if (!LEAD_DECISIONS.has(d)) return 'lead_decision'
      if (d === 'SPLIT' && !(Number(evidence.splitCount) > 0)) return 'lead_decision.splitCount'
      return null
    }
    return null
  }
}
