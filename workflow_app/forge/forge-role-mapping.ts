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
          `${STRUCTURED_PREFIX} {"rootCauseKnown":true,"diagnosisBlocked":false} (include only facts this run actually established)\n` +
          `FORGE_FINDINGS_JSON: [{"id":"<stable-key>","summary":"one distinct finding about the real repo surface","required":false,"seams":["path/prefix",...],"hint":"FOLLOW_UP_STORY|NOTE|HOLD"}] (emit the distinct findings your repo research established — files/surfaces + why each matters — so Architect inherits real intel, not garbage)`,
      }
    case 'research_architect':
      return {
        lane: 'architect',
        evidenceInstruction: `${STRUCTURED_PREFIX} {"researchDisposition":"IMPLEMENT|ARCHIVE|HOLD"}` +
          ' (REQUIRED — end your reply with this routing decision. research_disposition decides whether research becomes implementation (IMPLEMENT) or closes (ARCHIVE). Without a valid disposition the engine cannot route and this phase is HELD.)',
      }
    case 'architect':
    case 'repair_architect':
      return {
        lane: 'architect',
        evidenceInstruction:
          `${STRUCTURED_PREFIX} {"migrationRequired":false,"migrationFiles":[],"derivedRefreshRequired":false,"derivedModels":[],"deploymentRequired":true} (declare actual release obligations) ` +
          `FORGE_FINDINGS_JSON: [{"id":"<stable-key>","summary":"one distinct finding","required":true|false,"seams":["path/prefix",...],"hint":"SAME_UNIT|SPLIT_CHILD|FOLLOW_UP_STORY|NOTE|HOLD"}] (list EVERY distinct finding; required=true only when it must land to satisfy the ORIGINAL story)`,
      }
    case 'lead_pre':
      return {
        lane: 'lead',
        leadPhase: 'pre',
        evidenceInstruction:
          `${STRUCTURED_PREFIX} {"leadDecision":"SMITH|SPLIT|HOLD|SOLO","splitCount":0,"leadReason":"..."}` +
          ' (REQUIRED routing decision — SMITH = implement in one smith lane, SPLIT = parallelize into splitCount child lanes, SOLO = lead implements solo, HOLD = cannot proceed. When SPLIT, splitCount must be > 1. Without a valid leadDecision the engine cannot route and this phase is HELD.) ' +
          'In leadReason, capture your EXECUTION-SHAPE reasoning as structured, auditable lines. ' +
          'When you choose SPLIT you MUST include, one per split lane: `Split lane N scope: <the single bounded unit of work that lane N owns, no more>` and a `Merge gate: <the concrete check (files converge, exact tests pass) that proves lane N is done and the split can rejoin>`. ' +
          'REQUIRED for SMITH and SPLIT — you MUST also emit a machine work-order plan before handing work to Smith. Base every chunk on the story\'s ACTUAL goal + final acceptance + architect brief and the Scout surfaces in your context — never invent scope. End your reply with one machine line:\n' +
          '`LEAD_PLAN: {"size":"SMALL|MEDIUM|LARGE","chunks":[{"id":1,"scope":["<symbol-or-file>",...],"acceptance":"<runnable targeted test command>","preconditions":["<already-true/dependsOn>"],"postconditions":"<invariant that must still hold after>"},...]}` (1..3 serial chunks).\n' +
          'Rules: scope is symbol/function-level within a file, with an explicit do-not-touch list in postconditions when needed. Each chunk\'s acceptance is a NEW targeted test you write first (red->green) unless you explicitly reuse an existing one. Never invent acceptance beyond the story; never fake a proof. Size anchor: SMALL ~ one file + one new test, MEDIUM ~ a few files/2-3 tests. If the story cannot become <=3 honest, runnable chunks, choose HOLD and say why instead of forcing a bad plan. Example: LEAD_PLAN: {"size":"SMALL","chunks":[{"id":1,"scope":["workflow_app/forge/forge-ready-gate.ts#storyReadyToRunReasons"],"acceptance":"pnpm exec tsx --test workflow_app/tests/forge-ready-gate.test.ts","preconditions":[],"postconditions":"zero-command assay recipe returns missing-assay-plan; forge-static-gate.ts untouched"},{"id":2,"scope":["workflow_app/forge/forge-static-gate.ts#runStaticGate"],"acceptance":"pnpm exec tsx --test workflow_app/tests/forge-static-gate.test.ts","preconditions":["chunk 1 merged"],"postconditions":"depcruise skips when tool unavailable; no migration touched"}]}`',
      }
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
    case 'fast_smith':
    case 'fast_repair_smith':
      return { lane: 'smith' }
    case 'qa_review':
      return { lane: 'inspector' }
    case 'fast_qa_verify':
      return { lane: 'assay' }
    case 'qa_verify':
      return {
        lane: 'assay',
        evidenceInstruction:
          `${STRUCTURED_PREFIX} {"disposition":"REPAIR|REPLAN|ESCALATE","failedCriteria":["..."],"failedCommands":["..."]} ` +
          '(on FAIL, emit the machine disposition REPAIR/REPLAN/ESCALATE plus failed criteria/commands; REPAIR = plan valid, REPLAN = plan invalid, ESCALATE = cannot auto-recover)',
      }
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
  'architectureReviewRequired',
  'leadDecision',
  'splitCount',
  'findings',
  'qaReviewRequired',
  'qaReviewPassed',
  'qaPassed',
  'disposition',
  'failedCriteria',
  'failedCommands',
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
  'failedCriteria',
  'failedCommands',
])

const enumEvidenceValues: Partial<Record<keyof ForgeGateEvidence, ReadonlySet<string>>> = {
  researchDisposition: new Set(['IMPLEMENT', 'ARCHIVE', 'HOLD']),
  leadDecision: new Set(['SOLO', 'SMITH', 'SPLIT', 'HOLD']),
  disposition: new Set(['REPAIR', 'REPLAN', 'ESCALATE']),
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
    case 'qa_verify':
    case 'fast_qa_verify': {
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
