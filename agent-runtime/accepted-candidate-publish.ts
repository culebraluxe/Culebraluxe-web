// ---------------------------------------------------------------------------
// Accepted Candidate Publish — outer Forge only.
//
// V6 publication consumes structured Assay evidence when present. Narrative
// tests_summary is legacy compatibility only and cannot override a V6 verdict.
// Git publication remains safe non-force fast-forward only.
// ---------------------------------------------------------------------------

import {
  publishAcceptedCandidate,
  type PublishAcceptedCandidateInput,
  type PublishAcceptedCandidateOutcome,
} from '../lib/worker-workspace'
import { isCleanAssayResult } from './orchestrate-apply'
import type { AssayEvidence } from './assay-evidence'

export const ASSAY_FINISH_ROLES = ['reviewer', 'verifier'] as const

export function isTerminalAssayRole(role: string | null | undefined): boolean {
  const value = (role ?? '').trim().toLowerCase()
  return (ASSAY_FINISH_ROLES as readonly string[]).includes(value)
}

export type AcceptedCandidatePublishReport =
  | { action: 'not-eligible'; reason: string }
  | { action: 'no-candidate'; reason: string }
  | {
      action: 'published'
      candidateCommit: string
      publishedMainHash: string
    }
  | {
      action: 'publish-conflict'
      candidateCommit: string | null
      remoteMainHash: string | null
      reason: string
    }

export type AcceptedCandidatePublishInput = {
  role: string | null | undefined
  resultStatus?: string | null
  testsSummary?: string | null
  /** V6 source of truth. If present, prose cannot affect eligibility. */
  assayEvidence?: AssayEvidence | null
  candidateCommit?: string | null
  assayedCandidate?: string | null
  repoRoot?: string
  publish?: (
    input: PublishAcceptedCandidateInput,
  ) => Promise<PublishAcceptedCandidateOutcome>
}

export async function publishAcceptedCandidateAfterAssay(
  input: AcceptedCandidatePublishInput,
): Promise<AcceptedCandidatePublishReport> {
  if (!isTerminalAssayRole(input.role)) {
    return {
      action: 'not-eligible',
      reason: `finished lane '${input.role ?? '(none)'}' is not a terminal Assay/verification lane; publication is a post-Assay acceptance action`,
    }
  }

  const clean = input.assayEvidence
    ? input.assayEvidence.verdict === 'PASS' &&
      input.assayEvidence.failureCode === null
    : isCleanAssayResult({
        resultStatus: input.resultStatus ?? null,
        testsSummary: input.testsSummary ?? null,
      })
  if (!clean) {
    return {
      action: 'not-eligible',
      reason: input.assayEvidence
        ? `structured Assay verdict is ${input.assayEvidence.verdict}${input.assayEvidence.failureCode ? ` (${input.assayEvidence.failureCode})` : ''}; failed/Hold Assay never publishes accepted code`
        : `legacy Assay did not finish clean (resultStatus='${input.resultStatus ?? '(none)'}'); a failed/Hold Assay never publishes accepted code`,
    }
  }

  const candidate = (input.candidateCommit ?? '').trim()
  if (!candidate) {
    return {
      action: 'no-candidate',
      reason:
        'no Smith candidate commit was recorded for this story; an empty/no-change candidate is never published',
    }
  }

  const assayedSource = input.assayEvidence
    ? input.assayEvidence.verifiedSha
    : input.assayedCandidate
  if (assayedSource !== undefined) {
    const assayed = (assayedSource ?? '').trim().toLowerCase()
    if (!assayed || assayed !== candidate.toLowerCase()) {
      return {
        action: 'not-eligible',
        reason: `Assay verified candidate ${assayed ? assayed.slice(0, 12) : '(none)'}, which does not equal the publish candidate ${candidate.slice(0, 12)}; accepted-candidate publication requires the exact candidate commit`,
      }
    }
  }

  const repoRoot = input.repoRoot?.trim() || process.cwd()
  const publish = input.publish ?? publishAcceptedCandidate
  const outcome = await publish({
    repoRoot,
    candidateCommit: candidate,
  })

  switch (outcome.outcome) {
    case 'no-candidate':
      return { action: 'no-candidate', reason: outcome.reason }
    case 'published':
      return {
        action: 'published',
        candidateCommit: outcome.candidateCommit,
        publishedMainHash: outcome.publishedMainHash,
      }
    case 'publish-conflict':
      return {
        action: 'publish-conflict',
        candidateCommit: outcome.candidateCommit,
        remoteMainHash: outcome.remoteMainHash,
        reason: outcome.reason,
      }
    default:
      return {
        action: 'publish-conflict',
        candidateCommit: candidate,
        remoteMainHash: null,
        reason: `unhandled publish outcome: ${(outcome as { outcome: string }).outcome}`,
      }
  }
}
