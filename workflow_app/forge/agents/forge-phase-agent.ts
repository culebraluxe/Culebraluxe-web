import type { ForgeGateEvidence } from '../forge-facts'
import {
  forgeRoleNodePlan,
  type ForgeRoleNodePlan,
} from '../forge-role-mapping'
import { findingsFromArchitectEvidence } from '../forge-shaping'

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
// Enforcement (deliverable + routing-decision gate, with bounded self-heal) is
// ON by default now that role outputs are reliable. A successful role that
// misses its deliverable/decision is re-run up to FORGE_DELIVERABLE_RETRIES
// times before HOLDing. Explicitly disable with FORGE_ENFORCE_DELIVERABLES=0 if
// a run needs the old lenient behavior.
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

/** Raw role output = model notes + tests summary (what a deliverable is built from). */
export function rawRoleOutput(notes?: string | null, testsSummary?: string | null): string {
  return [notes, testsSummary].filter(Boolean).join('\n').trim()
}

/** Number of corrective re-runs allowed after the first model run, from
 * FORGE_DELIVERABLE_RETRIES (default 1). A clean, parseable non-negative int wins;
 * anything unset/invalid falls back to 1. Only meaningful when the enforced gate
 * is ON — otherwise there is never a HOLD to self-heal. */
export function parseDeliverableRepromptBudget(raw: string | undefined): number {
  const n = Number.parseInt(raw ?? '', 10)
  return Number.isFinite(n) && n >= 0 ? n : 1
}

/** Deliverable/routing enforcement is ON by default now that role outputs are
 * reliable (write-on-exit persistence + routing-decision validation + bounded
 * self-heal, all proven live). Explicitly disable with FORGE_ENFORCE_DELIVERABLES=0
 * (or "false"/"off") if a run needs the old lenient behavior. */
export function deliverableEnforcementEnabled(raw: string | undefined): boolean {
  if (raw == null) return true
  const v = raw.trim().toLowerCase()
  return v !== '0' && v !== 'false' && v !== 'off'
}

/** Bounded self-heal directive for a successful-but-HOLDed role run: tells the
 * model, on a corrective re-run, exactly which deliverable / routing field was
 * missing and restates the required structured output. */
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
    if (this.nodeId === 'failure_classifier') return 'failure-class'
    if (this.isScout) return 'scout-packet'
    if (this.isArchitect) return 'architect-plan'
    // Only lead_pre is an execution-shape decision node. lead_solo_implement and
    // lead_post emit no decision deliverable (their deliverable is the candidate
    // / the post-integration evidence, owned elsewhere).
    if (this.plan.lane === 'lead' && this.plan.leadPhase === 'pre') return 'lead-decision'
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
      case 'failure-class':
        if (!FAILURE_CLASSES.has(String(evidence.failureClass ?? ''))) {
          missing.push('failure-class')
        }
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
    // Only lead_pre is an EXECUTION-SHAPE decision node. lead_solo_implement and
    // lead_post do NOT emit a routing lead_decision (they have no decision
    // instruction) - requiring one there would wrongly HOLD the post-implement
    // merge/integration node.
    if (this.plan.lane === 'lead' && this.plan.leadPhase === 'pre') {
      const d = String(evidence.leadDecision ?? '')
      if (!LEAD_DECISIONS.has(d)) return 'lead_decision'
      if (d === 'SPLIT' && !(Number(evidence.splitCount) > 0)) return 'lead_decision.splitCount'
      return null
    }
    return null
  }
}