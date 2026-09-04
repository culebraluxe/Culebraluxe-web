// ---------------------------------------------------------------------------
// Accepted Candidate Publish — outer Forge only.
//
// V6 publication consumes structured Story Run machine evidence when present.
// Narrative tests_summary is legacy compatibility only and cannot override a
// structured verdict. Git publication remains safe non-force fast-forward only.
// ---------------------------------------------------------------------------

import {
  publishAcceptedCandidate,
  type PublishAcceptedCandidateInput,
  type PublishAcceptedCandidateOutcome,
} from '../lib/worker-workspace'
import {
  isCleanRunMachineEvidence,
  type RunMachineEvidence,
} from '../lib/forge-run-evidence'
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
  /** V6 durable source of truth on storyboard_story_run. */
  machineEvidence?: RunMachineEvidence | null
  /** In-process deterministic Assay evidence; retained for compatibility/tests. */
  assayEvidence?: AssayEvidence | null
  candidateCommit?: string | null
  assayedCandidate?: string | null
  repoRoot?: string
  publish?: (
    input: PublishAcceptedCandidateInput,
  ) => Promise<PublishAcceptedCandidateOutcome>
}

/**
 * ENG-FORGE-V6-VIS — dry-run publish preview.
 *
 * Same gates as publishAcceptedCandidateAfterAssay (terminal Assay role, clean
 * evidence, exact verified==candidate, fast-forward over origin/main) but
 * performs zero mutations: no push, no update-ref. Returns the preview string
 * the Portal/Slack/CLI lenses share: publishable|conflict|no-candidate plus
 * the factual reason.
 */
export async function previewAcceptedCandidatePublish(
  input: Omit<AcceptedCandidatePublishInput, 'publish'>,
): Promise<{ preview: 'publishable' | 'conflict' | 'no-candidate'; detail: string }> {
  const dryRun = async (
    publishInput: PublishAcceptedCandidateInput,
  ): Promise<PublishAcceptedCandidateOutcome> => {
    const { execFile: execFileCb } = await import('node:child_process')
    const { promisify } = await import('node:util')
    const execFileAsync = promisify(execFileCb)
    const repoRoot = publishInput.repoRoot
    const candidate = (publishInput.candidateCommit ?? '').trim()
    const runGit = async (args: string[]) => {
      try {
        const { stdout } = await execFileAsync('git', args, { cwd: repoRoot, encoding: 'utf8' })
        return stdout.trim()
      } catch {
        return null
      }
    }
    if (!candidate) {
      return { outcome: 'no-candidate', reason: 'no candidate commit recorded' }
    }
    const remoteLine = await runGit(['ls-remote', 'origin', 'refs/heads/main'])
    const remoteMain = remoteLine?.split(/\s+/)[0] ?? ''
    if (!remoteMain) {
      return {
        outcome: 'publish-conflict',
        candidateCommit: candidate,
        remoteMainHash: null,
        reason: 'origin/main is unreadable (offline or no remote)',
      }
    }
    if (remoteMain === candidate) {
      return { outcome: 'published', candidateCommit: candidate, publishedMainHash: candidate }
    }
    try {
      await execFileAsync('git', ['merge-base', '--is-ancestor', remoteMain, candidate], {
        cwd: repoRoot,
      })
    } catch {
      return {
        outcome: 'publish-conflict',
        candidateCommit: candidate,
        remoteMainHash: remoteMain,
        reason: `origin/main (${remoteMain.slice(0, 12)}) is not an ancestor of candidate ${candidate.slice(0, 12)} — push would NOT fast-forward`,
      }
    }
    return { outcome: 'published', candidateCommit: candidate, publishedMainHash: remoteMain }
  }

  const report = await publishAcceptedCandidateAfterAssay({ ...input, publish: dryRun })
  switch (report.action) {
    case 'published':
      return {
        preview: 'publishable',
        detail: `candidate ${report.candidateCommit.slice(0, 12)} fast-forwards origin/main (dry-run, no push)`,
      }
    case 'no-candidate':
      return { preview: 'no-candidate', detail: report.reason }
    default:
      return { preview: 'conflict', detail: report.reason }
  }
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

  const completeStatus = /^complete$/i.test((input.resultStatus ?? '').trim())
  const clean = input.machineEvidence
    ? completeStatus &&
      (input.machineEvidence.commandsTotal ?? 0) > 0 &&
      isCleanRunMachineEvidence(input.machineEvidence)
    : input.assayEvidence
      ? completeStatus &&
        input.assayEvidence.verdict === 'PASS' &&
        input.assayEvidence.failureCode === null
      : isCleanAssayResult({
          resultStatus: input.resultStatus ?? null,
          testsSummary: input.testsSummary ?? null,
        })
  if (!clean) {
    return {
      action: 'not-eligible',
      reason: input.machineEvidence
        ? `structured Run evidence is not publishable: status=${input.resultStatus ?? '(none)'}, commands=${input.machineEvidence.commandsPassed ?? '?'}/${input.machineEvidence.commandsTotal ?? '?'}, tests=${input.machineEvidence.testsPassed ?? '?'}/${input.machineEvidence.testsTotal ?? '?'}, failures=${input.machineEvidence.testsFailed ?? '?'}, policy=${input.machineEvidence.policyViolationCount ?? '?'}, failure=${input.machineEvidence.failureCode ?? '(none)'}`
        : input.assayEvidence
          ? `structured Assay is not publishable: status=${input.resultStatus ?? '(none)'}, verdict=${input.assayEvidence.verdict}${input.assayEvidence.failureCode ? `, failure=${input.assayEvidence.failureCode}` : ''}`
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

  const assayedSource = input.machineEvidence?.baseCommitHash ??
    input.assayEvidence?.verifiedSha ??
    input.assayedCandidate
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
  const outcome = await publish({ repoRoot, candidateCommit: candidate })

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
