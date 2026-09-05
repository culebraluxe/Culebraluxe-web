import type { LaneId } from '../../agent-runtime/lanes'
import type { AgentRunEvidence } from '../../agent-runtime/types'
import type { ForgeGateEvidence } from './forge-facts'

export type ForgeRoleNodePlan = {
  lane: LaneId
  leadPhase?: 'pre' | 'implement' | 'post'
  evidenceInstruction?: string
}

const STRUCTURED_PREFIX = 'FORGE_EVIDENCE_JSON:'

export function forgeRoleNodePlan(nodeId: string): ForgeRoleNodePlan {
  switch (nodeId) {
    case 'research_scout':
    case 'feature_scout':
    case 'diagnose_scout':
    case 'repair_scout':
      return {
        lane: 'scout',
        evidenceInstruction:
          `${STRUCTURED_PREFIX} {"rootCauseKnown":true,"diagnosisBlocked":false} (include only facts this run actually established)`,
      }
    case 'research_architect':
      return {
        lane: 'architect',
        evidenceInstruction: `${STRUCTURED_PREFIX} {"researchDisposition":"IMPLEMENT|ARCHIVE|HOLD"}`,
      }
    case 'architect':
    case 'repair_architect':
      return {
        lane: 'architect',
        evidenceInstruction:
          `${STRUCTURED_PREFIX} {"migrationRequired":false,"migrationFiles":[],"derivedRefreshRequired":false,"derivedModels":[],"deploymentRequired":true} (declare actual release obligations)`,
      }
    case 'lead_pre':
      return { lane: 'lead', leadPhase: 'pre' }
    case 'lead_solo_implement':
      return { lane: 'lead', leadPhase: 'implement' }
    case 'lead_post':
      return { lane: 'lead', leadPhase: 'post' }
    case 'failure_classifier':
      return {
        lane: 'lead',
        leadPhase: 'pre',
        evidenceInstruction:
          `${STRUCTURED_PREFIX} {"failureClass":"CODE_DEFECT|TEST_DEFECT|ARCHITECTURE_GAP|REQUIREMENTS_GAP|UNKNOWN_CAUSE|ENVIRONMENT|MIGRATION|PUBLISH_CONFLICT|DEPLOYMENT|PRODUCTION_SMOKE|HOLD"}`,
      }
    case 'smith':
    case 'smith_split_work':
    case 'repair_smith':
      return { lane: 'smith' }
    case 'qa_review':
      return { lane: 'inspector' }
    case 'qa_verify':
      return { lane: 'assay' }
    case 'repair_devops':
    case 'deploy':
    case 'production_smoke':
      return { lane: 'dev_ops' }
    default:
      throw new Error(`No Forge agent-runtime mapping for engine node '${nodeId}'`)
  }
}

const allowedEvidenceKeys = new Set<keyof ForgeGateEvidence>([
  'researchDisposition',
  'scoutRequired',
  'rootCauseKnown',
  'diagnosisBlocked',
  'architectureSuspect',
  'leadDecision',
  'splitCount',
  'qaReviewRequired',
  'qaReviewPassed',
  'qaPassed',
  'failureClass',
  'failedReleaseStage',
  'migrationRequired',
  'migrationFiles',
  'derivedRefreshRequired',
  'derivedModels',
  'deploymentRequired',
  'resumeTarget',
])

const booleanEvidenceKeys = new Set<keyof ForgeGateEvidence>([
  'scoutRequired',
  'rootCauseKnown',
  'diagnosisBlocked',
  'architectureSuspect',
  'qaReviewRequired',
  'qaReviewPassed',
  'qaPassed',
  'migrationRequired',
  'derivedRefreshRequired',
  'deploymentRequired',
])

const stringArrayEvidenceKeys = new Set<keyof ForgeGateEvidence>([
  'migrationFiles',
  'derivedModels',
])

const enumEvidenceValues: Partial<Record<keyof ForgeGateEvidence, ReadonlySet<string>>> = {
  researchDisposition: new Set(['IMPLEMENT', 'ARCHIVE', 'HOLD']),
  leadDecision: new Set(['SOLO', 'SMITH', 'SPLIT', 'HOLD']),
  failureClass: new Set([
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
  ]),
  failedReleaseStage: new Set([
    'PUBLISH',
    'DEV_MIGRATION',
    'PROD_MIGRATION',
    'DERIVED_REFRESH',
    'DEPLOY',
    'SMOKE',
  ]),
  resumeTarget: new Set([
    'SCOUT',
    'DIAGNOSE',
    'ARCHITECT',
    'LEAD',
    'SMITH',
    'QA',
    'DEV_OPS',
    'PUBLISH',
    'DEPLOY',
    'SMOKE',
    'CANCEL',
  ]),
}

function validMarkerValue(key: keyof ForgeGateEvidence, value: unknown): boolean {
  if (booleanEvidenceKeys.has(key)) return typeof value === 'boolean'
  if (stringArrayEvidenceKeys.has(key)) {
    return (
      Array.isArray(value) &&
      value.length <= 100 &&
      value.every((item) => typeof item === 'string' && item.trim().length > 0)
    )
  }
  if (key === 'splitCount') return Number.isInteger(value) && Number(value) >= 2 && Number(value) <= 8
  const allowed = enumEvidenceValues[key]
  return Boolean(allowed && typeof value === 'string' && allowed.has(value))
}

/** Parse one explicit machine marker; arbitrary prose never becomes routing. */
export function parseForgeEvidenceMarker(text: string | null | undefined): ForgeGateEvidence {
  const line = (text ?? '')
    .split(/\r?\n/)
    .find((candidate) => candidate.trim().startsWith(STRUCTURED_PREFIX))
  if (!line) return {}
  const raw = line.slice(line.indexOf(STRUCTURED_PREFIX) + STRUCTURED_PREFIX.length).trim()
  try {
    const parsed = JSON.parse(raw) as unknown
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {}
    const evidence: Record<string, unknown> = {}
    for (const [key, value] of Object.entries(parsed)) {
      const evidenceKey = key as keyof ForgeGateEvidence
      if (allowedEvidenceKeys.has(evidenceKey) && validMarkerValue(evidenceKey, value)) {
        evidence[key] = value
      }
    }
    return evidence as ForgeGateEvidence
  } catch {
    return {}
  }
}

function cleanResult(result: AgentRunEvidence): boolean {
  return result.completion === 100 && /^(complete|success|pass)$/i.test(result.resultStatus.trim())
}

function commitSha(value: string | null | undefined): string | null {
  const sha = value?.trim().toLowerCase() ?? ''
  return /^[0-9a-f]{7,64}$/.test(sha) ? sha : null
}

export function forgeEvidenceFromAgentResult(input: {
  nodeId: string
  result: AgentRunEvidence
  current: ForgeGateEvidence
  leadDecision?: { decision: string | null; splitCount: number | null } | null
}): ForgeGateEvidence {
  const { nodeId, result, current } = input
  const marked = parseForgeEvidenceMarker([result.notes, result.testsSummary].filter(Boolean).join('\n'))
  const clean = cleanResult(result)
  switch (nodeId) {
    case 'lead_pre':
      return {
        ...marked,
        ...(input.leadDecision?.decision
          ? {
              leadDecision: input.leadDecision.decision as ForgeGateEvidence['leadDecision'],
              splitCount: input.leadDecision.splitCount ?? undefined,
            }
          : {}),
      }
    case 'lead_solo_implement':
    case 'smith':
    case 'smith_split_work':
    case 'repair_smith': {
      const candidateSha = commitSha(result.commitHash)
      return candidateSha ? { ...marked, candidateSha } : marked
    }
    case 'lead_post': {
      const candidateSha = commitSha(result.commitHash) ?? commitSha(current.candidateSha)
      return candidateSha ? { ...marked, candidateSha } : marked
    }
    case 'qa_review':
      return { ...marked, qaReviewPassed: clean }
    case 'qa_verify': {
      const candidate = commitSha(current.candidateSha)
      const verified = commitSha(result.assayEvidence?.verifiedSha)
      const exact = Boolean(
        clean &&
          result.assayEvidence?.verdict === 'PASS' &&
          !result.assayEvidence.failureCode &&
          candidate &&
          verified === candidate,
      )
      return {
        ...marked,
        qaPassed: exact,
        ...(verified ? { qaVerifiedSha: verified } : {}),
        ...(!exact ? { failureClass: 'CODE_DEFECT' as const } : {}),
      }
    }
    case 'deploy': {
      const published = commitSha(current.publishedSha)
      const receipt = result.releaseEvidence
      const deployed = commitSha(receipt?.artifactSha)
      const exact = Boolean(
        clean &&
          published &&
          receipt?.kind === 'deployment' &&
          receipt.success &&
          receipt.receiptId.trim() &&
          deployed === published,
      )
      return {
        ...marked,
        deploymentSucceeded: exact,
        ...(exact && deployed
          ? { deployedSha: deployed, deploymentReceipt: receipt!.receiptId.trim() }
          : {}),
        ...(!exact
          ? { failureClass: 'DEPLOYMENT' as const, failedReleaseStage: 'DEPLOY' as const }
          : {}),
      }
    }
    case 'production_smoke': {
      const artifact = current.deploymentRequired
        ? commitSha(current.deployedSha)
        : commitSha(current.publishedSha)
      const receipt = result.releaseEvidence
      const verified = commitSha(receipt?.artifactSha)
      const exact = Boolean(
        clean &&
          artifact &&
          receipt?.kind === 'production_verification' &&
          receipt.success &&
          receipt.receiptId.trim() &&
          verified === artifact,
      )
      return {
        ...marked,
        productionVerified: exact,
        ...(exact && verified
          ? {
              productionVerifiedSha: verified,
              productionVerificationReceipt: receipt!.receiptId.trim(),
            }
          : {}),
        ...(!exact
          ? { failureClass: 'PRODUCTION_SMOKE' as const, failedReleaseStage: 'SMOKE' as const }
          : {}),
      }
    }
    default:
      return marked
  }
}
