import type { ForgeFailure, ForgeFailureCode } from './forge-failure'
import { forgeFailure } from './forge-failure'

export type ForgeTransitionAction =
  | 'enqueue-assay'
  | 'publish'
  | 'complete'
  | 'retry-same-lane'
  | 'hold-human'
  | 'stop'

export type ForgeTransitionDecision = {
  action: ForgeTransitionAction
  nextLane: 'assay' | null
  storyStatus: 'Ready' | 'In Progress' | 'Hold' | 'Complete' | null
  humanRequired: boolean
  failure: ForgeFailure | null
}

export type ForgeTransitionEvent =
  | { type: 'smith-complete'; candidateSha: string | null }
  | { type: 'smith-failed'; code?: ForgeFailureCode; detail: string }
  | { type: 'smith-runtime-interrupted'; attempts: number; maxAttempts: number; detail: string }
  | { type: 'assay-pass' }
  | { type: 'assay-fail'; code: ForgeFailureCode; detail: string }
  | { type: 'assay-runtime-interrupted'; detail: string }
  | { type: 'publish-complete' }
  | { type: 'publish-conflict'; detail: string }

/**
 * Forge V6 transition reducer.
 *
 * This is intentionally pure. It decides WHAT happens next; callers persist
 * the result. No scheduler, repository, model, test output, or prose parsing is
 * allowed inside this function.
 *
 * Critical invariant:
 *   Smith -> Assay -> PASS -> publish
 *                    FAIL -> HUMAN HOLD
 * Assay never restarts Smith and never auto-retries itself.
 */
export function decideForgeTransition(
  event: ForgeTransitionEvent,
): ForgeTransitionDecision {
  switch (event.type) {
    case 'smith-complete':
      if (!event.candidateSha) {
        return {
          action: 'hold-human',
          nextLane: null,
          storyStatus: 'Hold',
          humanRequired: true,
          failure: forgeFailure(
            'NO_CANDIDATE',
            'Smith finished without a candidate commit; Assay cannot verify a fallback base.',
          ),
        }
      }
      return {
        action: 'enqueue-assay',
        nextLane: 'assay',
        storyStatus: null,
        humanRequired: false,
        failure: null,
      }

    case 'smith-runtime-interrupted':
      if (event.attempts < event.maxAttempts) {
        return {
          action: 'retry-same-lane',
          nextLane: null,
          storyStatus: 'Ready',
          humanRequired: false,
          failure: forgeFailure('SMITH_RUNTIME_INTERRUPTED', event.detail, false),
        }
      }
      return {
        action: 'hold-human',
        nextLane: null,
        storyStatus: 'Hold',
        humanRequired: true,
        failure: forgeFailure('SMITH_RUNTIME_INTERRUPTED', event.detail),
      }

    case 'smith-failed':
      return {
        action: 'hold-human',
        nextLane: null,
        storyStatus: 'Hold',
        humanRequired: true,
        failure: forgeFailure(event.code ?? 'SMITH_RESULT_FAILED', event.detail),
      }

    case 'assay-pass':
      return {
        action: 'publish',
        nextLane: null,
        storyStatus: null,
        humanRequired: false,
        failure: null,
      }

    case 'assay-fail':
      return {
        action: 'hold-human',
        nextLane: null,
        storyStatus: 'Hold',
        humanRequired: true,
        failure: forgeFailure(event.code, event.detail),
      }

    case 'assay-runtime-interrupted':
      return {
        action: 'hold-human',
        nextLane: null,
        storyStatus: 'Hold',
        humanRequired: true,
        failure: forgeFailure('ASSAY_RUNTIME_INTERRUPTED', event.detail),
      }

    case 'publish-complete':
      return {
        action: 'complete',
        nextLane: null,
        storyStatus: 'Complete',
        humanRequired: false,
        failure: null,
      }

    case 'publish-conflict':
      return {
        action: 'hold-human',
        nextLane: null,
        storyStatus: 'Hold',
        humanRequired: true,
        failure: forgeFailure('PUBLISH_CONFLICT', event.detail),
      }
  }
}
