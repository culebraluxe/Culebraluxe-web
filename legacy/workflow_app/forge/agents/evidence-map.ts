/**
 * ADD. Integration contract — which evidence field each role's collect() fills.
 * Parent missingDeliverables / routingDecisionMissing read these and decide.
 */
export const ROLE_EVIDENCE_MAP = [
  { role: 'scout', kind: 'scout-packet', storyField: 'context_refs', evidence: 'findings (intel, required:false)' },
  { role: 'architect', kind: 'architect-plan', storyField: 'architect_brief', evidence: 'findings' },
  { role: 'research_architect', kind: 'architect-plan', storyField: 'architect_brief', evidence: 'researchDisposition' },
  { role: 'lead_pre', kind: 'lead-decision', storyField: null, evidence: 'leadDecision, splitCount, leadRouting' },
  { role: 'smith', kind: 'smith-candidate', storyField: null, evidence: 'candidateSha' },
  { role: 'assay', kind: 'qa-verdict', storyField: null, evidence: 'qaPassed, qaVerifiedSha, verificationGap' },
  { role: 'failure_classifier', kind: 'failure-class', storyField: null, evidence: 'failureClass' },
  { role: 'dev_ops', kind: 'devops-receipt', storyField: null, evidence: 'deploymentReceipt | productionVerificationReceipt | deploymentDeferredToBatch' },
] as const
