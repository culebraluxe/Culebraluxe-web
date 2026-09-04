import {
  followFinishedLane,
  hydrateBareReadyItems,
  isCleanAssayResult,
} from '../agent-runtime/orchestrate-apply'
import { parseAssayCommands } from '../agent-runtime/assay-plan'
import { storyFieldsFromBoardAndGit } from '../agent-runtime/orchestrate'
import {
  isTerminalAssayRole,
  publishAcceptedCandidateAfterAssay,
  type AcceptedCandidatePublishReport,
} from '../agent-runtime/accepted-candidate-publish'
import {
  finishedRunCandidateSha,
  verifiedShaFromWorkspaceEvidence,
} from '../agent-runtime/candidate-assay-handoff'
import {
  decideForgeTransition,
  type ForgeTransitionDecision,
} from '../agent-runtime/forge-transition'
import type { ForgeFailureCode } from '../agent-runtime/forge-failure'
import {
  hasStructuredRunMachineEvidence,
  isCleanRunMachineEvidence,
} from '../lib/forge-run-evidence'
import {
  enqueueAgentWorkCommand,
  listAgentWorkItems,
} from '../db/agent-work'
import {
  appendForgeRunDetail,
  getForgeRunMachineEvidence,
} from '../db/forge-run'
import {
  markForgeStoryHumanHold,
  markForgeStoryPublishedComplete,
} from '../db/forge-story-state'
import {
  getStoryboardStory,
  listStoryRuns,
  updateStoryRunProgress,
  type StoryRun,
} from '../db/storyboard'

async function appendTransitionDecision(
  run: StoryRun | null,
  decision: ForgeTransitionDecision,
): Promise<void> {
  if (!run) return
  const failure = decision.failure
    ? ` failure=${decision.failure.code}: ${decision.failure.detail}`
    : ''
  await appendForgeRunDetail(
    run.id,
    `transition action=${decision.action} next_lane=${decision.nextLane ?? '(none)'} story_status=${decision.storyStatus ?? '(unchanged)'} human_required=${decision.humanRequired}${failure}`,
  )
}

export async function runForgeHydrate(): Promise<string[]> {
  return hydrateBareReadyItems({
    listItems: listAgentWorkItems,
    getStory: getStoryboardStory,
    enqueue: enqueueAgentWorkCommand,
  })
}

export async function runForgeFollow(input: {
  storyId: string
  finishedRole: string | null
  resultStatus?: string | null
  testsSummary?: string | null
}): Promise<string | null> {
  if (!input.finishedRole) return null
  const runs = await listStoryRuns(input.storyId)
  const newestRun = runs[0] ?? null
  const candidateSha = finishedRunCandidateSha(runs)
  const followed = await followFinishedLane({
    storyId: input.storyId,
    finishedRole: input.finishedRole,
    resultStatus: input.resultStatus,
    testsSummary: input.testsSummary ?? null,
    candidateSha,
    getStory: getStoryboardStory,
    enqueue: enqueueAgentWorkCommand,
  })

  const okResult =
    !input.resultStatus || /complete|success|pass/i.test(input.resultStatus)

  if (input.finishedRole === 'builder' && okResult) {
    const transition = decideForgeTransition({
      type: 'smith-complete',
      candidateSha,
    })
    await appendTransitionDecision(newestRun, transition)

    if (transition.action === 'hold-human') {
      await markForgeStoryHumanHold(input.storyId)
      if (newestRun && transition.failure) {
        await updateStoryRunProgress(newestRun.id, {
          note: `${transition.failure.code}: ${transition.failure.detail} Human intervention required.`,
        })
      }
      return null
    }

    if (followed) return followed

    // Defensive compatibility for a Smith item launched before V6 preflight.
    // New V6 Smith work cannot reach this condition because the Assay recipe is
    // now required before token spend.
    const story = await getStoryboardStory(input.storyId)
    const merged = story
      ? storyFieldsFromBoardAndGit(story, input.storyId)
      : null
    const missingPlan =
      !merged || parseAssayCommands(merged.assayCommands).length === 0
    const detail = missingPlan
      ? `Smith candidate ${candidateSha} was preserved, but its historical work item has no Assay plan. Human intervention required; do not rebuild Smith automatically.`
      : `Smith candidate ${candidateSha} could not hand off to Assay. Human intervention required; do not retry Smith automatically.`
    const holdDecision = decideForgeTransition({
      type: 'smith-failed',
      code: missingPlan ? 'MISSING_ASSAY_PLAN' : 'HUMAN_DECISION_REQUIRED',
      detail,
    })
    await appendTransitionDecision(newestRun, holdDecision)
    await markForgeStoryHumanHold(input.storyId)
    if (newestRun) await updateStoryRunProgress(newestRun.id, { note: detail })
    return null
  }

  return followed
}

export type ForgePublishAfterAssayInput = {
  storyId: string
  finishedRole: string | null
  resultStatus?: string | null
  testsSummary?: string | null
  repoRoot?: string
  publish?: typeof publishAcceptedCandidateAfterAssay
}

export type ForgePublishAfterAssayOutcome =
  | { kind: 'published'; candidateCommit: string; publishedMainHash: string }
  | { kind: 'publish-conflict'; detail: string }
  | { kind: 'no-candidate'; detail: string }
  | { kind: 'skipped'; detail: string }

/**
 * V6 outer publication gate. Generic structured fields on the Assay Story Run
 * are authoritative for new runs. Legacy text scanning is used only when those
 * fields are absent.
 */
export async function runForgePublishAfterAssay(
  input: ForgePublishAfterAssayInput,
): Promise<ForgePublishAfterAssayOutcome | null> {
  if (!input.storyId || !isTerminalAssayRole(input.finishedRole)) return null

  const runs = await listStoryRuns(input.storyId)
  const newestRun = runs[0] ?? null
  if (!newestRun) {
    return { kind: 'skipped', detail: 'Assay run not found.' }
  }

  const machineEvidence = await getForgeRunMachineEvidence(newestRun.id)
  const hasStructured = hasStructuredRunMachineEvidence(machineEvidence)
  const resultStatus = newestRun.resultStatus ?? input.resultStatus ?? null
  const candidateRun = runs.find((run) => Boolean(run.commitHash)) ?? null
  const candidateCommit = candidateRun?.commitHash ?? null
  const assayedCandidate = machineEvidence?.baseCommitHash ??
    verifiedShaFromWorkspaceEvidence(newestRun.notes)

  const persistedClean = hasStructured
    ? /^complete$/i.test((resultStatus ?? '').trim()) &&
      (machineEvidence?.commandsTotal ?? 0) > 0 &&
      isCleanRunMachineEvidence(machineEvidence)
    : isCleanAssayResult({
        resultStatus,
        testsSummary: newestRun.testsSummary ?? input.testsSummary ?? null,
      })

  const evidenceFailureCode = machineEvidence?.failureCode as ForgeFailureCode | null
  const acceptanceTransition = decideForgeTransition(
    persistedClean
      ? { type: 'assay-pass' }
      : {
          type: 'assay-fail',
          code: evidenceFailureCode ?? 'ASSAY_TEST_FAILED',
          detail: hasStructured
            ? `Assay Run evidence is not publishable (status=${resultStatus ?? '(none)'}, commands=${machineEvidence?.commandsPassed ?? '?'}/${machineEvidence?.commandsTotal ?? '?'}, tests=${machineEvidence?.testsPassed ?? '?'}/${machineEvidence?.testsTotal ?? '?'}, failed=${machineEvidence?.testsFailed ?? '?'}, policy=${machineEvidence?.policyViolationCount ?? '?'}, failure=${machineEvidence?.failureCode ?? '(none)'}).`
            : `Assay did not produce a publishable result (status=${resultStatus ?? '(none)'}).`,
        },
  )
  await appendTransitionDecision(newestRun, acceptanceTransition)

  if (acceptanceTransition.action !== 'publish') {
    await markForgeStoryHumanHold(input.storyId)
    const detail =
      acceptanceTransition.failure?.detail ?? 'Assay failed; human intervention required.'
    await updateStoryRunProgress(newestRun.id, {
      note: `${detail} No automatic Assay retry and no Smith restart.`,
    })
    return { kind: 'skipped', detail }
  }

  if (candidateCommit && assayedCandidate !== candidateCommit) {
    const detail =
      `Assay verified ${assayedCandidate ? assayedCandidate.slice(0, 12) : '(none)'}, ` +
      `but publish candidate is ${candidateCommit.slice(0, 12)}. Candidate preserved; human intervention required.`
    const conflict = decideForgeTransition({ type: 'publish-conflict', detail })
    await appendTransitionDecision(newestRun, conflict)
    await markForgeStoryHumanHold(input.storyId)
    await updateStoryRunProgress(newestRun.id, { note: detail })
    return { kind: 'publish-conflict', detail }
  }

  const publish = input.publish ?? publishAcceptedCandidateAfterAssay
  const report: AcceptedCandidatePublishReport = await publish({
    role: input.finishedRole,
    resultStatus,
    testsSummary: newestRun.testsSummary ?? input.testsSummary ?? null,
    machineEvidence: hasStructured ? machineEvidence : null,
    candidateCommit,
    assayedCandidate,
    repoRoot: input.repoRoot,
  })

  switch (report.action) {
    case 'not-eligible': {
      const decision = decideForgeTransition({
        type: 'publish-conflict',
        detail: report.reason,
      })
      await appendTransitionDecision(newestRun, decision)
      await markForgeStoryHumanHold(input.storyId)
      return { kind: 'skipped', detail: report.reason }
    }
    case 'no-candidate': {
      const decision = decideForgeTransition({
        type: 'smith-failed',
        code: 'NO_CANDIDATE',
        detail: report.reason,
      })
      await appendTransitionDecision(newestRun, decision)
      await markForgeStoryHumanHold(input.storyId)
      return { kind: 'no-candidate', detail: report.reason }
    }
    case 'published': {
      await appendForgeRunDetail(
        newestRun.id,
        `publish completed candidate=${report.candidateCommit} published_main=${report.publishedMainHash}`,
      )
      const complete = decideForgeTransition({ type: 'publish-complete' })
      await appendTransitionDecision(newestRun, complete)
      await markForgeStoryPublishedComplete(input.storyId)
      await updateStoryRunProgress(newestRun.id, {
        note: `Accepted candidate published: candidate ${report.candidateCommit} -> origin/main ${report.publishedMainHash}`,
      })
      return {
        kind: 'published',
        candidateCommit: report.candidateCommit,
        publishedMainHash: report.publishedMainHash,
      }
    }
    case 'publish-conflict': {
      const detail =
        `Accepted candidate ${report.candidateCommit ?? '(none)'} was NOT published to origin/main ` +
        `(remote main ${report.remoteMainHash ?? '(unreadable)'}): ${report.reason}`
      await appendForgeRunDetail(newestRun.id, `publish conflict: ${detail}`)
      const decision = decideForgeTransition({ type: 'publish-conflict', detail })
      await appendTransitionDecision(newestRun, decision)
      await markForgeStoryHumanHold(input.storyId)
      await updateStoryRunProgress(newestRun.id, {
        note: `Publish conflict — human intervention required: ${detail}`,
      })
      return { kind: 'publish-conflict', detail }
    }
    default:
      return null
  }
}
