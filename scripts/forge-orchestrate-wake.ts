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
  enqueueAgentWorkCommand,
  listAgentWorkItems,
} from '../db/agent-work'
import {
  getStoryboardStory,
  listStoryRuns,
  setStoryboardStatus,
  updateStoryRunProgress,
} from '../db/storyboard'

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
  // ENG-FORGE-V4-10C: resolve the exact Smith candidate this cycle produced
  // from existing run evidence. For a JUST-finished code run the candidate is
  // that run's OWN commit (never an older cycle's candidate); follow may only
  // hand a builder finish to Assay when a candidate exists.
  const runs = await listStoryRuns(input.storyId)
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

  // V5 repairability: Assay planning belongs at the Smith→Assay boundary, not
  // the Smith launch boundary. A clean Smith candidate is valuable evidence.
  // If verification commands are missing, preserve that candidate and Hold the
  // story for an Assay-plan repair instead of turning completed Smith work into
  // a launch Error or allowing the story to stand Complete without verification.
  if (!followed && input.finishedRole === 'builder' && okResult && candidateSha) {
    const story = await getStoryboardStory(input.storyId)
    const merged = story
      ? storyFieldsFromBoardAndGit(story, input.storyId)
      : null
    if (!merged || parseAssayCommands(merged.assayCommands).length === 0) {
      const newestRun = runs[0] ?? null
      const detail =
        `Smith candidate ${candidateSha} completed and was preserved, but Assay was not launched because the story packet has no ## Assay commands. ` +
        'Story held for verification planning; add the Assay plan and resume from the existing candidate rather than rebuilding Smith work.'
      await setStoryboardStatus(input.storyId, 'Hold')
      if (newestRun) {
        await updateStoryRunProgress(newestRun.id, { note: detail })
      }
      console.log('follow hold', input.storyId, 'builder', 'missing-assay-plan', detail)
      return null
    }
  }

  // ENG-FORGE-V4-10C: a code-changing Smith run that finished without a
  // candidate commit must NOT stand Complete or launch Assay-as-though-
  // verification-were-possible. Hold the story with factual evidence; the
  // candidate branch/worktree (when one exists) is always preserved.
  if (!followed && input.finishedRole === 'builder' && okResult && !candidateSha) {
    const newestRun = runs[0] ?? null
    const detail =
      'Smith produced no candidate commit for this code-changing run; Assay was not launched because verification requires the exact candidate commit (never a fallback base such as main). Story held for repair/retry.'
    await setStoryboardStatus(input.storyId, 'Hold')
    if (newestRun) {
      await updateStoryRunProgress(newestRun.id, { note: detail })
    }
    console.log('follow hold', input.storyId, 'builder', 'no-candidate', detail)
    return null
  }
  return followed
}

export type ForgePublishAfterAssayInput = {
  storyId: string
  finishedRole: string | null
  resultStatus?: string | null
  testsSummary?: string | null
  /** Primary checkout root owning `origin` (defaults to the scheduler cwd). */
  repoRoot?: string
  publish?: typeof publishAcceptedCandidateAfterAssay
}

export type ForgePublishAfterAssayOutcome =
  | { kind: 'published'; candidateCommit: string; publishedMainHash: string }
  | { kind: 'publish-conflict'; detail: string }
  | { kind: 'no-candidate'; detail: string }
  | { kind: 'skipped'; detail: string }

/**
 * ENG-FORGE-V4-10B — outer-Forge publication of an accepted candidate after a
 * clean Assay result. Runs ONLY in the outer Forge process (the scheduler /
 * worker host that owns the git checkout and `origin`), never in the model
 * sandbox.
 */
export async function runForgePublishAfterAssay(
  input: ForgePublishAfterAssayInput,
): Promise<ForgePublishAfterAssayOutcome | null> {
  if (!input.storyId || !isTerminalAssayRole(input.finishedRole)) return null

  const runs = await listStoryRuns(input.storyId)
  const newestRun = runs[0] ?? null
  const candidateRun = runs.find((run) => Boolean(run.commitHash)) ?? null
  const candidateCommit = candidateRun?.commitHash ?? null
  const assayedCandidate = newestRun
    ? verifiedShaFromWorkspaceEvidence(newestRun.notes)
    : null
  const persistedClean = isCleanAssayResult({
    resultStatus: newestRun?.resultStatus ?? input.resultStatus ?? null,
    testsSummary: newestRun?.testsSummary ?? input.testsSummary ?? null,
  })

  if (persistedClean && candidateCommit && assayedCandidate !== candidateCommit) {
    const detail =
      `Clean Assay verified candidate ${assayedCandidate ? assayedCandidate.slice(0, 12) : '(none)'} (workspace base), ` +
      `but the publish candidate is ${candidateCommit.slice(0, 12)}. ` +
      `Publication requires the clean Assay to have verified the exact candidate being published (ENG-FORGE-V4-10C). ` +
      `Candidate commit preserved; story held for repair.`
    await setStoryboardStatus(input.storyId, 'Hold')
    if (newestRun) {
      await updateStoryRunProgress(newestRun.id, { note: detail })
    }
    return { kind: 'publish-conflict', detail }
  }

  const publish = input.publish ?? publishAcceptedCandidateAfterAssay
  const report: AcceptedCandidatePublishReport = await publish({
    role: input.finishedRole,
    resultStatus: newestRun?.resultStatus ?? input.resultStatus ?? null,
    testsSummary: newestRun?.testsSummary ?? input.testsSummary ?? null,
    candidateCommit,
    assayedCandidate,
    repoRoot: input.repoRoot,
  })

  switch (report.action) {
    case 'not-eligible':
      return { kind: 'skipped', detail: report.reason }
    case 'no-candidate':
      return { kind: 'no-candidate', detail: report.reason }
    case 'published': {
      if (newestRun) {
        await updateStoryRunProgress(newestRun.id, {
          note: `Accepted candidate published: candidate ${report.candidateCommit} -> origin/main ${report.publishedMainHash}`,
        })
      }
      return {
        kind: 'published',
        candidateCommit: report.candidateCommit,
        publishedMainHash: report.publishedMainHash,
      }
    }
    case 'publish-conflict': {
      await setStoryboardStatus(input.storyId, 'Hold')
      const detail =
        `Accepted candidate ${report.candidateCommit ?? '(none)'} was NOT published to origin/main ` +
        `(remote main ${report.remoteMainHash ?? '(unreadable)'}): ${report.reason}`
      if (newestRun) {
        await updateStoryRunProgress(newestRun.id, {
          note: `Publish conflict — story held for repair: ${detail}`,
        })
      }
      return { kind: 'publish-conflict', detail }
    }
    default:
      return null
  }
}
