// ---------------------------------------------------------------------------
// ENG-FORGE-V4-10B — Accepted Candidate Publish: decision orchestration.
//
// Publication is a POST-Assay acceptance action, owned by the OUTER Forge
// process (never by Smith execution and never by Assay itself). This module
// answers one question: given the terminal verification run that just
// finished, is the story's accepted Smith candidate eligible for direct
// publication to `origin/main`, and did that publication succeed?
//
// Gates (fail closed, in order):
//   1. the finished lane is a terminal Assay/verification lane
//   2. the Assay result is clean (Complete with no failure/violation/policy
//      evidence) — the same isCleanAssayResult semantic Assay repair uses
//   3. a real Smith candidate commit exists — an empty/no-change candidate is
//      never published
//   4. the git publish itself must be a safe non-force fast-forward
//      (lib/worker-workspace/publish.ts); divergence fails closed into a
//      factual publish-conflict that preserves the candidate
//
// No schema change, no provider change, no swarm/parallel behavior. Existing
// lane ordering, execution-contract, harness-owned commit, readiness, and
// Assay semantics are untouched — this module only OBSERVES their results.
// ---------------------------------------------------------------------------

import {
  publishAcceptedCandidate,
  type PublishAcceptedCandidateInput,
  type PublishAcceptedCandidateOutcome,
} from '../lib/worker-workspace'
import { isCleanAssayResult } from './orchestrate-apply'

/**
 * Roles that represent a terminal Assay/verification completion in the Forge
 * vocabulary. The codebase has used both `reviewer` (V3 Assay repair/terminal
 * semantics) and `verifier` (the DEFAULT_LANES.assay binding role) for this
 * lane; publication triggers on either so Assay semantics stay unchanged.
 */
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
  /** Role of the run that just finished (the Assay lane). */
  role: string | null | undefined
  /** Assay run result status (Complete / Partial / Hold / ...). */
  resultStatus?: string | null
  /** Assay run tests/checks summary (used by the clean-Assay gate). */
  testsSummary?: string | null
  /** Smith candidate commit hash recorded in the story's run evidence. */
  candidateCommit?: string | null
  /**
   * ENG-FORGE-V4-10C: the candidate SHA the clean Assay actually verified
   * (the workspace base its run evidence records). When supplied, accepted-
   * candidate publication requires EXACT equality with `candidateCommit` —
   * an Assay that verified `main` (or any other base) may never publish the
   * candidate. Omitted keeps the legacy V4-10B decision for direct callers.
   */
  assayedCandidate?: string | null
  /** Primary checkout root owning `origin` (outer Forge host). */
  repoRoot?: string
  /** Injectable git publish (tests substitute a spy / fake repo). */
  publish?: (
    input: PublishAcceptedCandidateInput,
  ) => Promise<PublishAcceptedCandidateOutcome>
}

/**
 * Run the accepted-candidate publication decision for a just-finished Assay
 * run. Never throws for publish decisions; every gate produces a typed report.
 */
export async function publishAcceptedCandidateAfterAssay(
  input: AcceptedCandidatePublishInput,
): Promise<AcceptedCandidatePublishReport> {
  if (!isTerminalAssayRole(input.role)) {
    return {
      action: 'not-eligible',
      reason: `finished lane '${input.role ?? '(none)'}' is not a terminal Assay/verification lane; publication is a post-Assay acceptance action`,
    }
  }

  const clean = isCleanAssayResult({
    resultStatus: input.resultStatus ?? null,
    testsSummary: input.testsSummary ?? null,
  })
  if (!clean) {
    return {
      action: 'not-eligible',
      reason: `Assay did not finish clean (resultStatus='${input.resultStatus ?? '(none)'}'); a failed/Hold Assay never publishes accepted code`,
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

  // ENG-FORGE-V4-10C: accepted-candidate publication requires a clean Assay
  // that verified EXACTLY the candidate being published. A mismatched (or
  // unprovable) verified base never reaches the git publish.
  if (input.assayedCandidate !== undefined) {
    const assayed = (input.assayedCandidate ?? '').trim().toLowerCase()
    if (!assayed || assayed !== candidate.toLowerCase()) {
      return {
        action: 'not-eligible',
        reason: `Assay verified candidate ${assayed ? assayed.slice(0, 12) : '(none)'}${assayed ? '' : ' (no workspace base evidence)'}, which does not equal the publish candidate ${candidate.slice(0, 12)}; accepted-candidate publication requires the clean Assay to have verified the exact candidate commit being published`,
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
      // Exhaustiveness guard — a new git outcome must be mapped explicitly.
      return {
        action: 'publish-conflict',
        candidateCommit: candidate,
        remoteMainHash: null,
        reason: `unhandled publish outcome: ${(outcome as { outcome: string }).outcome}`,
      }
  }
}
