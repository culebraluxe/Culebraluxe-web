import type { ForgeFailure, ForgeFailureCode } from './forge-failure'
import { forgeFailure } from './forge-failure'
import type { LeadDecisionCode, LeadRunPhase } from './lead-decision'

export type ForgeTransitionAction =
  | 'enqueue-architect'
  | 'enqueue-lead'
  | 'enqueue-smith'
  | 'enqueue-smith-split'
  | 'enqueue-inspector'
  | 'enqueue-assay'
  | 'publish'
  | 'complete'
  | 'retry-same-lane'
  | 'hold-human'
  | 'stop'

export type ForgeTransitionDecision = {
  action: ForgeTransitionAction
  nextLane: 'architect' | 'lead' | 'smith' | 'inspector' | 'assay' | null
  nextPhase: LeadRunPhase | null
  parallelCount: number | null
  storyStatus: 'Ready' | 'In Progress' | 'Hold' | 'Complete' | null
  humanRequired: boolean
  failure: ForgeFailure | null
}

export type ForgeTransitionEvent =
  | { type: 'scout-complete' }
  | { type: 'architect-complete' }
  | { type: 'lead-pre'; decision: LeadDecisionCode | null; splitCount?: number | null; detail?: string | null }
  | { type: 'lead-implement-complete'; candidateSha: string | null }
  | { type: 'smith-complete'; candidateSha: string | null }
  | { type: 'smith-split-complete'; candidateShas: string[] }
  | { type: 'lead-post'; decision: LeadDecisionCode | null; candidateSha: string | null; detail?: string | null }
  | { type: 'qa-pass'; candidateSha: string | null }
  | { type: 'qa-fail'; code?: ForgeFailureCode; detail: string }
  | { type: 'smith-failed'; code?: ForgeFailureCode; detail: string }
  | { type: 'smith-runtime-interrupted'; attempts: number; maxAttempts: number; detail: string }
  | { type: 'qa-runtime-interrupted'; detail: string }
  | { type: 'assay-pass' }
  | { type: 'assay-fail'; code: ForgeFailureCode; detail: string }
  | { type: 'assay-runtime-interrupted'; detail: string }
  | { type: 'publish-complete' }
  | { type: 'publish-conflict'; detail: string }

function decision(input: Omit<ForgeTransitionDecision, 'parallelCount'> & { parallelCount?: number | null }): ForgeTransitionDecision {
  return { parallelCount: input.parallelCount ?? null, ...input }
}

function hold(code: ForgeFailureCode, detail: string): ForgeTransitionDecision {
  return decision({
    action: 'hold-human',
    nextLane: null,
    nextPhase: null,
    storyStatus: 'Hold',
    humanRequired: true,
    failure: forgeFailure(code, detail),
  })
}

/**
 * Forge V6.1 transition reducer. Pure machine routing only.
 *
 * Scout -> Architect -> Lead PRE
 * Lead PRE -> Lead SOLO | 1 Smith | bounded 2-3 Smith split | Hold
 * Smith(s) -> Lead POST -> independent QA -> deterministic Assay
 * Assay PASS -> publish; any judgment/machine FAIL -> human Hold.
 */
export function decideForgeTransition(event: ForgeTransitionEvent): ForgeTransitionDecision {
  switch (event.type) {
    case 'scout-complete':
      return decision({
        action: 'enqueue-architect',
        nextLane: 'architect',
        nextPhase: null,
        storyStatus: null,
        humanRequired: false,
        failure: null,
      })

    case 'architect-complete':
      return decision({
        action: 'enqueue-lead',
        nextLane: 'lead',
        nextPhase: 'pre',
        storyStatus: null,
        humanRequired: false,
        failure: null,
      })

    case 'lead-pre': {
      if (event.decision === 'HOLD') {
        return hold(
          'LEAD_ARCHITECTURE_CHALLENGE',
          event.detail?.trim() || 'Lead rejected the frozen Architect contract before implementation.',
        )
      }
      if (event.decision === 'SOLO') {
        return decision({
          action: 'enqueue-lead',
          nextLane: 'lead',
          nextPhase: 'implement',
          storyStatus: null,
          humanRequired: false,
          failure: null,
        })
      }
      if (event.decision === 'SMITH') {
        return decision({
          action: 'enqueue-smith',
          nextLane: 'smith',
          nextPhase: null,
          storyStatus: null,
          humanRequired: false,
          failure: null,
        })
      }
      if (event.decision === 'SPLIT') {
        const count = event.splitCount ?? 0
        if (count === 2 || count === 3) {
          return decision({
            action: 'enqueue-smith-split',
            nextLane: 'smith',
            nextPhase: null,
            parallelCount: count,
            storyStatus: null,
            humanRequired: false,
            failure: null,
          })
        }
        return hold('LEAD_SPLIT_INVALID', `Lead SPLIT count must be 2 or 3; received ${count}.`)
      }
      return hold(
        'LEAD_DECISION_MISSING',
        event.detail?.trim() || 'Lead PRE did not produce SOLO, SMITH, SPLIT:2, SPLIT:3, or HOLD.',
      )
    }

    case 'lead-implement-complete':
      if (!event.candidateSha) return hold('NO_CANDIDATE', 'Lead SOLO finished without a candidate commit.')
      return decision({
        action: 'enqueue-inspector',
        nextLane: 'inspector',
        nextPhase: null,
        storyStatus: null,
        humanRequired: false,
        failure: null,
      })

    case 'smith-complete':
      if (!event.candidateSha) return hold('NO_CANDIDATE', 'Smith finished without a candidate commit; Lead POST cannot integrate.')
      return decision({
        action: 'enqueue-lead',
        nextLane: 'lead',
        nextPhase: 'post',
        storyStatus: null,
        humanRequired: false,
        failure: null,
      })

    case 'smith-split-complete':
      if (event.candidateShas.length < 2 || event.candidateShas.some((sha) => !sha)) {
        return hold('SMITH_SPLIT_FAILED', 'Bounded Smith split completed without one candidate per assignment.')
      }
      return decision({
        action: 'enqueue-lead',
        nextLane: 'lead',
        nextPhase: 'post',
        storyStatus: null,
        humanRequired: false,
        failure: null,
      })

    case 'lead-post':
      if (event.decision === 'HOLD') {
        return hold('LEAD_INTEGRATION_FAILED', event.detail?.trim() || 'Lead POST rejected the implementation/integration.')
      }
      if (event.decision !== 'ASSAY') {
        return hold('LEAD_DECISION_MISSING', event.detail?.trim() || 'Lead POST did not produce ASSAY or HOLD.')
      }
      if (!event.candidateSha) return hold('NO_CANDIDATE', 'Lead POST approved QA but no integrated candidate commit exists.')
      return decision({
        action: 'enqueue-inspector',
        nextLane: 'inspector',
        nextPhase: null,
        storyStatus: null,
        humanRequired: false,
        failure: null,
      })

    case 'qa-pass':
      if (!event.candidateSha) return hold('NO_CANDIDATE', 'QA approved Assay but no reviewed candidate commit exists.')
      return decision({
        action: 'enqueue-assay',
        nextLane: 'assay',
        nextPhase: null,
        storyStatus: null,
        humanRequired: false,
        failure: null,
      })

    case 'qa-fail':
      return hold(event.code ?? 'QA_REVIEW_FAILED', event.detail)

    case 'smith-runtime-interrupted':
      if (event.attempts < event.maxAttempts) {
        return decision({
          action: 'retry-same-lane',
          nextLane: null,
          nextPhase: null,
          storyStatus: 'Ready',
          humanRequired: false,
          failure: forgeFailure('SMITH_RUNTIME_INTERRUPTED', event.detail, false),
        })
      }
      return hold('SMITH_RUNTIME_INTERRUPTED', event.detail)

    case 'smith-failed':
      return hold(event.code ?? 'SMITH_RESULT_FAILED', event.detail)

    case 'qa-runtime-interrupted':
      return hold('QA_RUNTIME_INTERRUPTED', event.detail)

    case 'assay-pass':
      return decision({ action: 'publish', nextLane: null, nextPhase: null, storyStatus: null, humanRequired: false, failure: null })

    case 'assay-fail':
      return hold(event.code, event.detail)

    case 'assay-runtime-interrupted':
      return hold('ASSAY_RUNTIME_INTERRUPTED', event.detail)

    case 'publish-complete':
      return decision({ action: 'complete', nextLane: null, nextPhase: null, storyStatus: 'Complete', humanRequired: false, failure: null })

    case 'publish-conflict':
      return hold('PUBLISH_CONFLICT', event.detail)
  }
}
