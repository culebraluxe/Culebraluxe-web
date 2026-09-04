import type { ForgeFailure, ForgeFailureCode } from './forge-failure'
import { forgeFailure } from './forge-failure'
import type { LeadDecisionCode, LeadRunPhase } from './lead-decision'

export type ForgeTransitionAction =
  | 'enqueue-lead'
  | 'enqueue-smith'
  | 'enqueue-assay'
  | 'publish'
  | 'complete'
  | 'retry-same-lane'
  | 'hold-human'
  | 'stop'

export type ForgeTransitionDecision = {
  action: ForgeTransitionAction
  nextLane: 'lead' | 'smith' | 'assay' | null
  nextPhase: LeadRunPhase | null
  storyStatus: 'Ready' | 'In Progress' | 'Hold' | 'Complete' | null
  humanRequired: boolean
  failure: ForgeFailure | null
}

export type ForgeTransitionEvent =
  | { type: 'architect-complete' }
  | { type: 'lead-pre'; decision: LeadDecisionCode | null; splitCount?: number | null; detail?: string | null }
  | { type: 'lead-implement-complete'; candidateSha: string | null }
  | { type: 'smith-complete'; candidateSha: string | null }
  | { type: 'lead-post'; decision: LeadDecisionCode | null; candidateSha: string | null; detail?: string | null }
  | { type: 'smith-failed'; code?: ForgeFailureCode; detail: string }
  | { type: 'smith-runtime-interrupted'; attempts: number; maxAttempts: number; detail: string }
  | { type: 'assay-pass' }
  | { type: 'assay-fail'; code: ForgeFailureCode; detail: string }
  | { type: 'assay-runtime-interrupted'; detail: string }
  | { type: 'publish-complete' }
  | { type: 'publish-conflict'; detail: string }

function hold(code: ForgeFailureCode, detail: string): ForgeTransitionDecision {
  return {
    action: 'hold-human',
    nextLane: null,
    nextPhase: null,
    storyStatus: 'Hold',
    humanRequired: true,
    failure: forgeFailure(code, detail),
  }
}

/**
 * Forge V6 transition reducer. Pure machine routing only.
 *
 * Architect -> Lead PRE -> (Lead SOLO | Smith | Hold)
 * Smith -> Lead POST -> Assay
 * Lead SOLO implementation -> Assay
 * Assay PASS -> publish; Assay FAIL -> human Hold.
 */
export function decideForgeTransition(
  event: ForgeTransitionEvent,
): ForgeTransitionDecision {
  switch (event.type) {
    case 'architect-complete':
      return {
        action: 'enqueue-lead',
        nextLane: 'lead',
        nextPhase: 'pre',
        storyStatus: null,
        humanRequired: false,
        failure: null,
      }

    case 'lead-pre': {
      if (event.decision === 'HOLD') {
        return hold(
          'LEAD_ARCHITECTURE_CHALLENGE',
          event.detail?.trim() || 'Lead rejected the frozen Architect contract before implementation.',
        )
      }
      if (event.decision === 'SOLO') {
        return {
          action: 'enqueue-lead',
          nextLane: 'lead',
          nextPhase: 'implement',
          storyStatus: null,
          humanRequired: false,
          failure: null,
        }
      }
      if (event.decision === 'SMITH') {
        return {
          action: 'enqueue-smith',
          nextLane: 'smith',
          nextPhase: null,
          storyStatus: null,
          humanRequired: false,
          failure: null,
        }
      }
      if (event.decision === 'SPLIT') {
        const count = event.splitCount ?? 0
        if (count > 1) {
          return hold(
            'LEAD_SPLIT_REQUIRES_MULTIWORKER',
            `Lead selected SPLIT:${count}, but the current Forge queue supports only one active work item per story/system. Preserve the decomposition; do not fake parallelism or spend extra Smith tokens until multi-worker execution is enabled.`,
          )
        }
      }
      return hold(
        'LEAD_DECISION_MISSING',
        event.detail?.trim() || 'Lead PRE did not produce a valid structured SOLO, SMITH, SPLIT:n, or HOLD decision.',
      )
    }

    case 'lead-implement-complete':
      if (!event.candidateSha) {
        return hold(
          'NO_CANDIDATE',
          'Lead SOLO implementation finished without a candidate commit; Assay cannot verify a fallback base.',
        )
      }
      return {
        action: 'enqueue-assay',
        nextLane: 'assay',
        nextPhase: null,
        storyStatus: null,
        humanRequired: false,
        failure: null,
      }

    case 'smith-complete':
      if (!event.candidateSha) {
        return hold(
          'NO_CANDIDATE',
          'Smith finished without a candidate commit; Lead POST cannot integrate a fallback base.',
        )
      }
      return {
        action: 'enqueue-lead',
        nextLane: 'lead',
        nextPhase: 'post',
        storyStatus: null,
        humanRequired: false,
        failure: null,
      }

    case 'lead-post':
      if (event.decision === 'HOLD') {
        return hold(
          'LEAD_INTEGRATION_FAILED',
          event.detail?.trim() || 'Lead POST rejected the implementation/integration.',
        )
      }
      if (event.decision !== 'ASSAY') {
        return hold(
          'LEAD_DECISION_MISSING',
          event.detail?.trim() || 'Lead POST did not produce a valid structured ASSAY or HOLD decision.',
        )
      }
      if (!event.candidateSha) {
        return hold(
          'NO_CANDIDATE',
          'Lead POST approved Assay but no integrated candidate commit exists.',
        )
      }
      return {
        action: 'enqueue-assay',
        nextLane: 'assay',
        nextPhase: null,
        storyStatus: null,
        humanRequired: false,
        failure: null,
      }

    case 'smith-runtime-interrupted':
      if (event.attempts < event.maxAttempts) {
        return {
          action: 'retry-same-lane',
          nextLane: null,
          nextPhase: null,
          storyStatus: 'Ready',
          humanRequired: false,
          failure: forgeFailure('SMITH_RUNTIME_INTERRUPTED', event.detail, false),
        }
      }
      return hold('SMITH_RUNTIME_INTERRUPTED', event.detail)

    case 'smith-failed':
      return hold(event.code ?? 'SMITH_RESULT_FAILED', event.detail)

    case 'assay-pass':
      return {
        action: 'publish',
        nextLane: null,
        nextPhase: null,
        storyStatus: null,
        humanRequired: false,
        failure: null,
      }

    case 'assay-fail':
      return hold(event.code, event.detail)

    case 'assay-runtime-interrupted':
      return hold('ASSAY_RUNTIME_INTERRUPTED', event.detail)

    case 'publish-complete':
      return {
        action: 'complete',
        nextLane: null,
        nextPhase: null,
        storyStatus: 'Complete',
        humanRequired: false,
        failure: null,
      }

    case 'publish-conflict':
      return hold('PUBLISH_CONFLICT', event.detail)
  }
}
