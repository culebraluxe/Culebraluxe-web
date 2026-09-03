import {
  followFinishedLane,
  hydrateBareReadyItems,
  isCleanAssayResult,
} from '../agent-runtime/orchestrate-apply'
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

  // ENG-FORGE-V4-10C: a code-changing Smith run that finished without a
  // candidate commit must NOT stand Complete or launch Assay-as-though-
  // verification-were-possible. Hold the story with factual evidence; the
  // candidate branch/worktree (when one exists) is always preserved.
  const okResult =
    !input.resultStatus || /complete|success|pass/i.test(input.resultStatus)
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
 *
 * Callers invoke this once a terminal Assay/verification run has finished:
 *   - clean Assay + real candidate + compatible origin/main -> candidate is
 *     pushed to `origin/main` (no force), evidence note records both hashes,
 *     and the story stays on its clean-Completion path
 *   - publish conflict (remote main advanced / push rejected) -> story is
 *     failed closed into Hold with the factual conflict in run evidence; the
 *     candidate commit is preserved for repair/retry
 *   - no candidate / not eligible -> nothing is published and no story state
 *     is touched
 *
 * The candidate is the newest story run that recorded a commit (the Smith
 * candidate; Assay runs never keep a commit). No schema change is involved:
 * evidence lands in the existing run narrative.
 */
export async function runForgePublishAfterAssay(
  input: ForgePublishAfterAssayInput,
): Promise<ForgePublishAfterAssayOutcome | null> {
  // Fast path: publication only ever follows a terminal Assay/verification
  // lane; other finished lanes keep today's exact follow behavior.
  if (!input.storyId || !isTerminalAssayRole(input.finishedRole)) return null

  const runs = await listStoryRuns(input.storyId)
  const newestRun = runs[0] ?? null
  const candidateRun = runs.find((run) => Boolean(run.commitHash)) ?? null
  const candidateCommit = candidateRun?.commitHash ?? null
  // ENG-FORGE-V4-10C: the clean-Assay facts are read from the PERSISTED Assay
  // run (result status, summary, and the workspace base its evidence records)
  // — never from raw model output that already bypassed terminal
  // normalization. verifiedSha == candidate is the acceptance proof.
  const assayedCandidate = newestRun
    ? verifiedShaFromWorkspaceEvidence(newestRun.notes)
    : null
  const persistedClean = isCleanAssayResult({
    resultStatus: newestRun?.resultStatus ?? input.resultStatus ?? null,
    testsSummary: newestRun?.testsSummary ?? input.testsSummary ?? null,
  })

  // Fail-closed belt: a persisted clean Assay whose workspace base is not the
  // publish candidate means the Assay verified the wrong commit (e.g. main).
  // It must never publish, and the story must not stand Complete.
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
      // Fail closed: the story must NOT stand Complete while its accepted
      // code was not published. Hold keeps it visible for repair/retry and
      // the candidate commit is preserved (never discarded, never rewritten).
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
