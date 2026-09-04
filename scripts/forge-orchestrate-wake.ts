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
import { verifiedShaFromWorkspaceEvidence } from '../agent-runtime/candidate-assay-handoff'
import { decideForgeTransition, type ForgeTransitionDecision } from '../agent-runtime/forge-transition'
import type { ForgeFailureCode } from '../agent-runtime/forge-failure'
import type { LeadDecisionCode, LeadRunPhase } from '../agent-runtime/lead-decision'
import {
  hasStructuredRunMachineEvidence,
  isCleanRunMachineEvidence,
} from '../lib/forge-run-evidence'
import {
  enqueueForgeAgentWorkCommand,
  listForgeAgentWorkForStory,
  listForgeAgentWorkItems,
  type ForgeAgentWorkItem,
} from '../db/agent-work-v61'
import { sql } from '../db/client'
import {
  appendForgeRunDetail,
  getForgeLeadRunRecord,
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
    `transition action=${decision.action} next_lane=${decision.nextLane ?? '(none)'} next_phase=${decision.nextPhase ?? '(none)'} parallel=${decision.parallelCount ?? '(none)'} story_status=${decision.storyStatus ?? '(unchanged)'} human_required=${decision.humanRequired}${failure}`,
  )
}

export async function runForgeHydrate(): Promise<string[]> {
  return hydrateBareReadyItems({
    listItems: () => listForgeAgentWorkItems(sql),
    getStory: getStoryboardStory,
    enqueue: (input) => enqueueForgeAgentWorkCommand(input, sql),
  })
}

function runForWorkItem(
  runs: StoryRun[],
  item: ForgeAgentWorkItem,
): StoryRun | null {
  return item.storyRunId ? runs.find((run) => run.id === item.storyRunId) ?? null : null
}

function cleanComplete(run: StoryRun | null): boolean {
  return Boolean(run && /^complete$/i.test((run.resultStatus ?? '').trim()))
}

async function holdTransition(
  storyId: string,
  run: StoryRun | null,
  transition: ForgeTransitionDecision,
): Promise<null> {
  await appendTransitionDecision(run, transition)
  await markForgeStoryHumanHold(storyId)
  if (run && transition.failure) {
    await updateStoryRunProgress(run.id, {
      note: `${transition.failure.code}: ${transition.failure.detail} Human intervention required.`,
    })
  }
  return null
}

export async function runForgeFollow(input: {
  storyId: string
  finishedRole: string | null
  resultStatus?: string | null
  testsSummary?: string | null
}): Promise<string | null> {
  if (!input.finishedRole || isTerminalAssayRole(input.finishedRole)) return null

  const runs = await listStoryRuns(input.storyId)
  const newestRun = runs[0] ?? null
  if (!newestRun) return null
  const items = await listForgeAgentWorkForStory(input.storyId, sql)
  const currentItem = items.find((item) => item.storyRunId === newestRun.id) ?? null
  const resultStatus = newestRun.resultStatus ?? input.resultStatus ?? null

  let transition: ForgeTransitionDecision | null = null
  let candidateShas: string[] = []
  let splitComplete = false
  let leadPhase: LeadRunPhase | null = null
  let leadDecision: LeadDecisionCode | null = null
  let leadSplitCount: number | null = null
  let leadAssignments: string[] = []
  let leadReason: string | null = null
  let qaApproved: boolean | null = null
  let qaReason: string | null = null

  if (input.finishedRole === 'scout') {
    if (!/^complete$/i.test((resultStatus ?? '').trim())) return null
    transition = decideForgeTransition({ type: 'scout-complete' })
  } else if (input.finishedRole === 'architect') {
    if (!/^complete$/i.test((resultStatus ?? '').trim())) return null
    transition = decideForgeTransition({ type: 'architect-complete' })
  } else if (input.finishedRole === 'builder') {
    if (!currentItem) return null
    if (currentItem.parallelGroupId) {
      const siblings = items
        .filter((item) => item.parallelGroupId === currentItem.parallelGroupId)
        .sort((left, right) => (left.parallelSlot ?? 0) - (right.parallelSlot ?? 0))
      const expected = currentItem.parallelSize ?? 0
      if (siblings.length !== expected) {
        return holdTransition(
          input.storyId,
          newestRun,
          decideForgeTransition({
            type: 'smith-failed',
            code: 'SMITH_SPLIT_FAILED',
            detail: `Smith split ${currentItem.parallelGroupId} expected ${expected} siblings but found ${siblings.length}.`,
          }),
        )
      }
      if (siblings.some((item) => ['Ready', 'Claimed', 'Running', 'Paused'].includes(item.state))) {
        return null
      }
      const bad = siblings.find((item) => !cleanComplete(runForWorkItem(runs, item)))
      if (bad) {
        return holdTransition(
          input.storyId,
          newestRun,
          decideForgeTransition({
            type: 'smith-failed',
            code: 'SMITH_SPLIT_FAILED',
            detail: `Smith split slot ${bad.parallelSlot ?? '?'} did not complete cleanly; preserve sibling candidates for human review.`,
          }),
        )
      }
      candidateShas = siblings.map((item) => runForWorkItem(runs, item)?.commitHash ?? '')
      if (candidateShas.some((sha) => !sha)) {
        return holdTransition(
          input.storyId,
          newestRun,
          decideForgeTransition({
            type: 'smith-failed',
            code: 'SMITH_SPLIT_FAILED',
            detail: 'Every Smith split assignment completed but one or more candidate commits are missing.',
          }),
        )
      }
      splitComplete = true
      transition = decideForgeTransition({ type: 'smith-split-complete', candidateShas })
    } else {
      const candidate = newestRun.commitHash ?? null
      candidateShas = candidate ? [candidate] : []
      transition = decideForgeTransition({ type: 'smith-complete', candidateSha: candidate })
    }
  } else if (input.finishedRole === 'lead') {
    if (!currentItem) return null
    const record = await getForgeLeadRunRecord(newestRun.id)
    leadPhase = (record?.phase as LeadRunPhase | null) ?? currentItem.runPhase ?? null
    leadDecision = (record?.decision as LeadDecisionCode | null) ?? null
    leadSplitCount = record?.splitCount ?? null
    leadAssignments = record?.assignments ?? []
    const evidence = await getForgeRunMachineEvidence(newestRun.id)
    leadReason = evidence?.evidenceDetail ?? null

    if (leadPhase === 'pre') {
      transition = decideForgeTransition({
        type: 'lead-pre',
        decision: leadDecision,
        splitCount: leadSplitCount,
        detail: leadReason,
      })
    } else if (leadPhase === 'implement') {
      candidateShas = newestRun.commitHash ? [newestRun.commitHash] : []
      transition = decideForgeTransition({
        type: 'lead-implement-complete',
        candidateSha: candidateShas[0] ?? null,
      })
    } else if (leadPhase === 'post') {
      if (newestRun.commitHash) {
        candidateShas = [newestRun.commitHash]
      } else if (currentItem.candidateShas.length === 1) {
        candidateShas = [...currentItem.candidateShas]
      } else {
        return holdTransition(
          input.storyId,
          newestRun,
          decideForgeTransition({
            type: 'lead-post',
            decision: 'HOLD',
            candidateSha: null,
            detail: 'Lead POST received multiple Smith candidates but did not create one integrated candidate commit.',
          }),
        )
      }
      transition = decideForgeTransition({
        type: 'lead-post',
        decision: leadDecision,
        candidateSha: candidateShas[0] ?? null,
        detail: leadReason,
      })
    } else {
      transition = decideForgeTransition({
        type: 'lead-pre',
        decision: null,
        detail: 'Lead Run has no valid typed run_phase.',
      })
    }
  } else if (input.finishedRole === 'reviewer') {
    if (!currentItem) return null
    candidateShas = [...currentItem.candidateShas]
    qaApproved = /^complete$/i.test((resultStatus ?? '').trim())
    const evidence = await getForgeRunMachineEvidence(newestRun.id)
    qaReason = evidence?.evidenceDetail ?? newestRun.notes ?? null
    transition = qaApproved
      ? decideForgeTransition({ type: 'qa-pass', candidateSha: candidateShas[0] ?? null })
      : decideForgeTransition({
          type: 'qa-fail',
          code: (evidence?.failureCode as ForgeFailureCode | null) ?? 'QA_REVIEW_FAILED',
          detail: qaReason || 'Independent QA rejected the candidate.',
        })
  }

  if (!transition) return null
  await appendTransitionDecision(newestRun, transition)

  if (transition.action === 'hold-human') {
    await markForgeStoryHumanHold(input.storyId)
    if (transition.failure) {
      await updateStoryRunProgress(newestRun.id, {
        note: `${transition.failure.code}: ${transition.failure.detail} Human intervention required.`,
      })
    }
    return null
  }

  const followed = await followFinishedLane({
    storyId: input.storyId,
    finishedRole: input.finishedRole,
    resultStatus,
    testsSummary: input.testsSummary ?? null,
    candidateShas,
    splitComplete,
    parallelGroupId: leadPhase === 'pre' ? newestRun.id : currentItem?.parallelGroupId ?? null,
    leadPhase,
    leadDecision,
    leadSplitCount,
    leadAssignments,
    leadReason,
    qaApproved,
    qaReason,
    getStory: getStoryboardStory,
    enqueue: (work) => enqueueForgeAgentWorkCommand(work, sql),
  })
  if (followed) return followed

  if (transition.nextLane) {
    const story = await getStoryboardStory(input.storyId)
    const merged = story ? storyFieldsFromBoardAndGit(story, input.storyId) : null
    const missingPlan =
      transition.nextLane === 'assay' &&
      (!merged || parseAssayCommands(merged.assayCommands).length === 0)
    const detail = missingPlan
      ? `Candidate ${candidateShas[0] ?? '(none)'} was preserved, but the handoff has no Assay plan. Human intervention required.`
      : `Forge transition expected ${transition.nextLane}${transition.nextPhase ? `/${transition.nextPhase}` : ''} but no work item was enqueued. Human intervention required.`
    const holdDecision = decideForgeTransition({
      type: 'smith-failed',
      code: missingPlan ? 'MISSING_ASSAY_PLAN' : 'HUMAN_DECISION_REQUIRED',
      detail,
    })
    await appendTransitionDecision(newestRun, holdDecision)
    await markForgeStoryHumanHold(input.storyId)
    await updateStoryRunProgress(newestRun.id, { note: detail })
  }
  return null
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

export async function runForgePublishAfterAssay(
  input: ForgePublishAfterAssayInput,
): Promise<ForgePublishAfterAssayOutcome | null> {
  if (!input.storyId || !isTerminalAssayRole(input.finishedRole)) return null

  const runs = await listStoryRuns(input.storyId)
  const newestRun = runs[0] ?? null
  if (!newestRun) return { kind: 'skipped', detail: 'Assay run not found.' }
  const items = await listForgeAgentWorkForStory(input.storyId, sql)
  const assayItem = items.find((item) => item.storyRunId === newestRun.id) ?? null

  const machineEvidence = await getForgeRunMachineEvidence(newestRun.id)
  const hasStructured = hasStructuredRunMachineEvidence(machineEvidence)
  const resultStatus = newestRun.resultStatus ?? input.resultStatus ?? null
  const candidateCommit = assayItem?.candidateShas[0] ??
    runs.find((run) => Boolean(run.commitHash))?.commitHash ?? null
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
    const detail = acceptanceTransition.failure?.detail ?? 'Assay failed; human intervention required.'
    await updateStoryRunProgress(newestRun.id, {
      note: `${detail} No automatic Assay retry and no Smith restart.`,
    })
    return { kind: 'skipped', detail }
  }

  if (candidateCommit && assayedCandidate !== candidateCommit) {
    const detail =
      `Assay verified ${assayedCandidate ? assayedCandidate.slice(0, 12) : '(none)'}, ` +
      `but typed publish candidate is ${candidateCommit.slice(0, 12)}. Candidate preserved; human intervention required.`
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
      const decision = decideForgeTransition({ type: 'publish-conflict', detail: report.reason })
      await appendTransitionDecision(newestRun, decision)
      await markForgeStoryHumanHold(input.storyId)
      return { kind: 'skipped', detail: report.reason }
    }
    case 'no-candidate': {
      const decision = decideForgeTransition({ type: 'smith-failed', code: 'NO_CANDIDATE', detail: report.reason })
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
