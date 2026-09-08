import type { ForgeGateEvidence } from '../forge-facts'
import {
  forgeRoleNodePlan,
  type ForgeRoleNodePlan,
} from '../forge-role-mapping'
import {
  findingsFromArchitectEvidence,
  validateLeadShapeChoice,
} from '../forge-shaping'

// ---------------------------------------------------------------------------
// ENG-FORGE-PHASE-AGENT — the role/phase contract layer. A ForgePhaseAgent owns
// the marshal/deliverable concern for ONE engine role node. The parent provides
// the shared behavior every role inherits (raw text, findings parse, lead shape
// override, deliverable spec); the role's node id selects its deliverable.
//
// This is layered over (NOT a replacement for) the execution machinery: the
// harness AgentRuntimeAdapter still runs the model; forge-executor still drives
// the engine. This layer makes "a role must deliver its output" a declared,
// inspectable contract instead of scattered `if`-branches in the role-runner.
//
// Enforcement (assertDeliverable throwing) is OFF by default so existing flaky
// runs keep their current behavior; enable the hard gate with
// FORGE_ENFORCE_DELIVERABLES=1 once role outputs are reliable.
// ---------------------------------------------------------------------------

const SCOUT_NODES = new Set(['research_scout', 'feature_scout', 'diagnose_scout', 'repair_scout'])
const ARCHITECT_NODES = new Set(['architect', 'repair_architect', 'research_architect'])

const RESEARCH_DISPOSITIONS = new Set(['IMPLEMENT', 'ARCHIVE', 'HOLD'])
const LEAD_DECISIONS = new Set(['SMITH', 'SPLIT', 'HOLD', 'SOLO'])

export type PhaseDeliverableKind = 'scout-packet' | 'architect-plan' | 'lead-decision' | 'smith-candidate' | 'qa-verdict' | 'devops-receipt' | 'none'

/** Raw role output = model notes + tests summary (what a deliverable is built from). */
export function rawRoleOutput(notes?: string | null, testsSummary?: string | null): string {
  return [notes, testsSummary].filter(Boolean).join('\n').trim()
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

  /** The durable deliverable this phase is contractually expected to produce. */
  deliverableKind(): PhaseDeliverableKind {
    if (this.isScout) return 'scout-packet'
    if (this.isArchitect) return 'architect-plan'
    if (this.plan.lane === 'lead') return 'lead-decision'
    if (this.plan.lane === 'smith') return 'smith-candidate'
    if (this.plan.lane === 'assay') return 'qa-verdict'
    if (this.plan.lane === 'dev_ops') return 'devops-receipt'
    return 'none'
  }

  /**
   * Parse FORGE_FINDINGS_JSON from raw role output into evidence.findings.
   * Shared by Scout (repo research) and Architect (plan findings) — the 
   * generalization of the isScoutNode/isArchitectNode branches.
   */
  marshalFindings(evidence: ForgeGateEvidence, raw: string): ForgeGateEvidence {
    if (!this.isScout && !this.isArchitect) return evidence
    const parsed = findingsFromArchitectEvidence(raw)
    if (parsed.length > 0) evidence.findings = parsed
    return evidence
  }

  /**
   * Lead PRE shape override: when Lead's decision conflicts with what the
   * Architect findings demand, route to the bounded SPLIT. Shared + authoritative.
   */
  applyLeadShape(evidence: ForgeGateEvidence, currentFindings: ForgeGateEvidence['findings']): ForgeGateEvidence {
    if (this.nodeId !== 'lead_pre' || !evidence.leadDecision) return evidence
    const findings = currentFindings ?? []
    if (findings.length > 0) {
      const verdict = validateLeadShapeChoice({
        findings,
        choice: evidence.leadDecision,
        splitCount: evidence.splitCount,
      })
      if (!verdict.ok) {
        evidence.leadDecision = verdict.authoritative.decision
        evidence.splitCount = verdict.authoritative.splitCount ?? evidence.splitCount
      }
    }
    return evidence
  }

  /** Build a bounded Scout packet for the Story context_refs handoff (or null). */
  scoutPacket(raw: string): string | null {
    if (!this.isScout || !raw) return null
    const cap = 5000
    return (
      `Scout research packet (engine node ${this.nodeId}):\n` +
      raw.slice(0, cap) +
      (raw.length > cap ? '\n[Scout packet truncated; full output in scout story run notes]' : '')
    )
  }

  /** Build a bounded Architect plan for the Story architect_brief handoff (or null). */
  architectBrief(raw: string): string | null {
    if (!this.isArchitect || !raw) return null
    const cap = 5000
    return (
      `Architect plan (engine node ${this.nodeId}):\n` +
      raw.slice(0, cap) +
      (raw.length > cap ? '\n[Architect plan truncated; full output in architect story run notes]' : '')
    )
  }

  /** The Story field this role's durable deliverable writes to on exit (or null).
   * Centralizes write-on-exit: a role declares its Story-field deliverable here
   * and the runner persists it generically — no per-role if/else, no reliance on
   * the model self-formatting a marker. New roles that hand off free text add one
   * field here. */
  storyDeliverable(
    raw: string,
  ): { field: 'context_refs' | 'architect_brief'; text: string } | null {
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
   * Enforced deliverable gate (the point of the abstraction). Returns the list of
   * missing deliverables per role flavor. Callers may HOLD/retry when non-empty.
   * Not thrown here so enforcement policy (env-gated) stays in the caller.
   *
   * Each flavor declares what it must hand off on exit:
   *   scout     -> a packet (context_refs) for the next role
   *   architect -> a research disposition OR findings (a plan)
   *   lead      -> a decision (with split when SPLIT)
   *   smith     -> a frozen candidate SHA (it wrote code)
   *   qa/assay  -> an exact verdict (PASS or FAIL both count as a verdict)
   *   dev_ops   -> a release/production receipt
   */
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
        // Delivered = the plan was PERSISTED to the Story architect_brief from the
        // architect's actual output (write-on-exit), OR the model emitted a
        // structured disposition/findings. Never requires the marker alone.
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
      case 'smith-candidate':
        if (evidence.candidateSha == null) missing.push('smith-candidate')
        break
      case 'qa-verdict':
        // PASS or FAIL are both a verdict; only an absent verdict is a miss.
        if (evidence.qaPassed === undefined) missing.push('qa-verdict')
        break
      case 'devops-receipt':
        if (evidence.deploymentReceipt == null && evidence.productionVerificationReceipt == null) {
          missing.push('devops-receipt')
        }
        break
      default:
        break
    }
    return missing
  }

  /**
   * Routing-decision validation. Separate from the deliverable gate: a phase may
   * persist its work (plan persisted to architect_brief, notes captured) yet STILL
   * leave the engine without a usable routing decision (does this RESEARCH get
   * implemented or archived? does this LEAD hand off to smith, split, or hold?).
   * Only roles whose engine node routes on a decision return a requirement here.
   * A null/absent or invalid decision is returned as the missing routing field;
   * otherwise null means the routing decision is present and valid.
   */
  routingDecisionMissing(evidence: ForgeGateEvidence): string | null {
    if (this.nodeId === 'research_architect') {
      return RESEARCH_DISPOSITIONS.has(String(evidence.researchDisposition ?? ''))
        ? null
        : 'research_disposition'
    }
    if (this.plan.lane === 'lead') {
      const d = String(evidence.leadDecision ?? '')
      if (!LEAD_DECISIONS.has(d)) return 'lead_decision'
      if (d === 'SPLIT' && !(Number(evidence.splitCount) > 0)) return 'lead_decision.splitCount'
      return null
    }
    return null
  }
}