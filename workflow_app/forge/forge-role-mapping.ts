import type { LaneId } from '../../agent-runtime/lanes'
import type { AgentRunEvidence } from '../../agent-runtime/types'
import type { ForgeGateEvidence } from './forge-facts'
import { isPlaceholderReceiptId } from './forge-release-receipt'

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
          'FORGE_ARCHITECT_HANDOFF: {"version":1,"baseRef":"<the exact sha you inspected>","findings":[{"id":"<stable-key>","required":true|false,"summary":"<one distinct finding>","preconditions":["<already-true dependency>"],"scope":["<repository-relative path that EXISTS on baseRef>","<path#symbol>"],"postconditions":["<invariant that must still hold>"],"classes":["<TypeName>"],"risks":["<concrete risk>"],"hint":"SAME_UNIT|SPLIT_CHILD|FOLLOW_UP_STORY|NOTE|HOLD"}]} ' +
          'RECORD EACH FINDING AS A ROW — do NOT emit a JSON findings line. For every finding, run this with the identity from your task line:\n' +
          '`node --import tsx --env-file=.env.local scripts/forge-handoff.mjs --story <story> --process <process> --task <task> --node architect --attempt <attempt> --finding-id <stable-key> --summary "<one distinct finding>" --seams "<path[,path]>" [--required true|false] [--hint SAME_UNIT|SPLIT_CHILD|FOLLOW_UP_STORY|NOTE|HOLD] [--risks "<concrete risk>"]`\n' +
          'The database refuses exactly what this gate refuses and NAMES the failing constraint: a finding with no seam, more than three seams, a required HOLD with no named risk, an unknown hint, a blank summary. Fix the named field and run it again rather than rewording the reply. seams must EXIST on baseRef — an invented path is a HOLD, not a plan. required=true only when it must land to satisfy the ORIGINAL story. The FORGE_ARCHITECT_HANDOFF JSON line is still accepted as a FALLBACK while rows are introduced; rows win.',
      }
    case 'lead_pre':
      return {
        lane: 'lead',
        leadPhase: 'pre',
        evidenceInstruction:
          `${STRUCTURED_PREFIX} {"leadDecision":"SMITH|SPLIT|HOLD|SOLO","splitCount":0,"leadReason":"..."}` +
          ' (REQUIRED routing decision — SMITH = implement in one smith lane, SPLIT = parallelize into splitCount child lanes, SOLO = lead implements solo, HOLD = cannot proceed. When SPLIT, splitCount must be > 1. Without a valid leadDecision the engine cannot route and this phase is HELD.)\n' +
          'In leadReason, capture your EXECUTION-SHAPE reasoning as structured, auditable lines. ' +
          'When you choose SPLIT you MUST include, one per split lane: `Split lane N scope: <the single bounded unit of work that lane N owns, no more>` and a `Merge gate: <the concrete check (files converge, exact tests pass) that proves lane N is done and the split can rejoin>`. ' +
          'RECORD YOUR DECISION IN FIELDS — do NOT emit a JSON routing line. Run this once, with the identity from your task line:\n' +
          '`node --import tsx --env-file=.env.local scripts/forge-handoff.mjs --story <story> --process <process> --task <task> --node lead_pre --attempt <attempt> --decision SOLO|SMITH|SPLIT|HOLD --size SMALL|MEDIUM|LARGE --size-reason "<why this size>" --reason "<why this route>" --assignments <n> --findings <comma-separated finding ids> --merge-checks "<exact frozen command>" --scope <comma-separated paths>`\n' +
          'The database validates those fields and the engine reads them. If the row is rejected it names the failing field: fix it and run the command again. An unwritten decision is a HOLD.\n' +
          'REQUIRED for SMITH and SOLO — the PLAN goes in rows too. For EACH chunk run:\n' +
          '`node --import tsx --env-file=.env.local scripts/forge-handoff.mjs --story <story> --process <process> --task <task> --node lead_pre --attempt <attempt> --chunk <1..3> --assignment <id> --surface <path[,path]> --proof "<exact frozen command>" --invariant "<what must still hold>" --finding <finding id> --evidence <ref> --size SMALL|MEDIUM --semantic-surface <1-100> --dependency-depth <1-100> --uncertainty <1-5> --context-burden <1-5> --proof-burden <1-5> --coupling <1-5> --change-novelty <1-5> --worker-fit <1-5>`\n' +
          'A chunk with no surface or no proof is REFUSED by the database — that is the point. Base every chunk on the story\'s ACTUAL goal + final acceptance + architect brief and the Scout surfaces in your context — never invent scope. Maximum 3 chunks; a fourth is a recut, not a plan. (There is NO chat fallback: a decision or a plan that was never written to rows is a HOLD.)\n' +
          'Rules: scope is symbol/function-level within a file, with an explicit do-not-touch list in postconditions when needed. Each chunk\'s acceptance is a NEW targeted test you write first (red->green) unless you explicitly reuse an existing one. Never invent acceptance beyond the story; never fake a proof. Size anchor: SMALL ~ one file + one new test, MEDIUM ~ a few files/2-3 tests. If the story cannot become <=3 honest, runnable chunks, choose HOLD and say why instead of forcing a bad plan.',
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
    case 'lead_pre': {
      // THE DURABLE FINDINGS SNAPSHOT — the Lead's actual input.
      //
      // `ForgeGateEvidence.findings` is documented as the durable Architect snapshot for
      // the Lead shaping gate, and NOTHING populated it. So `LeadAgent.collect` read an
      // empty list, built a RoutingContext with zero required findings, and the reviewer
      // refused every proposal with "No required findings supplied; obtain the bounded
      // Architect handoff" — even when the model's routing decision was valid and its
      // evidence refs, finding ids and proof command all matched. Live on 2026-09-13 that
      // HOLDed the smoke story after both attempts while the durable row held a perfectly
      // good finding.
      //
      // It hid for so long because the runner ran its OWN review as a fallback (built from
      // the durable row, so correct); removing that duplicate seat exposed this. The second
      // seat was masking a broken first seat.
      const findings =
        Array.isArray(current.findings) && current.findings.length > 0
          ? { findings: current.findings }
          : {}
      return {
        ...marked,
        ...findings,
        ...(input.leadDecision?.decision
          ? {
              leadDecision: input.leadDecision.decision as ForgeGateEvidence['leadDecision'],
              splitCount: input.leadDecision.splitCount ?? undefined,
            }
          : {}),
      }
    }
    // EVERY NODE THAT CAN PRODUCE A CANDIDATE, INCLUDING THE FAST LANE.
    //
    // fast_smith and fast_repair_smith were missing from this list, so the commit the FAST
    // smith had made never reached `evidence.candidateSha`. The runner then had no diff to
    // offer the Smith exit gate, which refused the work it had just watched land:
    // "role did not deliver smith-candidate" — with the change committed and its tests
    // passing in the same evidence. Observed live on 2026-09-13.
    //
    // A candidate-producing lane is defined by WHAT IT PRODUCES, not by the route it took
    // to get there, so the FAST nodes belong here beside their serial siblings.
    case 'lead_solo_implement':
    case 'smith':
    case 'smith_split_work':
    case 'repair_smith':
    case 'fast_smith':
    case 'fast_repair_smith': {
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
      // THE CANDIDATE SHA MUST RIDE THE EVIDENCE.
      //
      // `collectAssayEvidence` reads `evidence.candidateSha` to bind the assay to the
      // candidate. Computing it here and NOT returning it left the deterministic Assay
      // with NO_CANDIDATE, which the adjudicator scores INCOMPLETE -> the QA lane
      // reported a verification GAP on every story, so no story could ever pass QA.
      //
      // Worse, it read as a contradiction: the harness adapter passed the same candidate
      // and its verified SHA was durable, so the row showed `qa_verified_sha` set while
      // `qa_passed` was false. The gap branch recorded no reason, so nothing said why.
      // Live on 2026-09-13, found by running the chain end to end.
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
        ...(candidate ? { candidateSha: candidate } : {}),
        qaPassed: exact,
        ...(verified ? { qaVerifiedSha: verified } : {}),
        ...(!exact ? { failureClass: 'CODE_DEFECT' as const } : {}),
      }
    }
    case 'deploy': {
      const published = commitSha(current.publishedSha)
      const receipt = result.releaseEvidence
      const deployed = commitSha(receipt?.artifactSha)
      // TECH-DEBT-07 (dialed back 2026-09-12): a provider deployment id is a HUMAN
      // domain — nothing here can observe Vercel's deploy state — so a story that does
      // NOT require a deployment is released on the release-engineer attestation
      // (integration + a build that actually ran + the git sha). A story that DOES
      // require a deployment still needs a real deployment receipt, unchanged.
      const receiptKindOk = Boolean(
        receipt?.success &&
          receipt.receiptId.trim() &&
          // A placeholder id ("n/a", "test", "tbd") is not evidence of anything.
          !isPlaceholderReceiptId(receipt.receiptId) &&
          (current.deploymentRequired
            ? receipt.kind === 'deployment'
            : receipt.kind === 'deployment' || receipt.kind === 'integration'),
      )
      const exact = Boolean(clean && published && receiptKindOk && deployed === published)
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
