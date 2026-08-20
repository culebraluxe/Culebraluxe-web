import type { ProcessGraph } from '../../workflow_engine/lib/workflow/types'

// ---------------------------------------------------------------------------
// transaction-close-v1 — the first real CulebraLuxe brokerage process.
//
// Model: OFFER ACCEPTED → CONTRACT PREPARATION → CONTRACT EXECUTED
//        → UNDER CONTRACT → parallel transaction milestones → READY TO CLOSE
//        → CLOSING → CLOSED.
//
// Milestone applicability (determined from the current business model):
//   - inspection : REQUIRED (always)
//   - title      : REQUIRED (always)
//   - appraisal  : CONDITIONAL — required only when financed
//   - financing  : CONDITIONAL — required only when financed
//
// The engine's fork branches are static, so "conditional" is expressed with a
// decision node (`financed` fact) that routes to a financed fork (four required
// branches) or a cash fork (two required branches). Not every transaction is
// financed.
//
// Orchestration state (engine) stays distinct from canonical business state
// (deal.stage). The two `command` nodes request canonical stage changes through
// the application port; they never mutate business rows directly.
// ---------------------------------------------------------------------------

export const TRANSACTION_CLOSE_V1_KEY = 'transaction-close-v1'
export const TRANSACTION_CLOSE_V1_VERSION = 1

export const transactionCloseV1Graph: ProcessGraph = {
  startNodeId: 'start',
  nodes: {
    start: {
      id: 'start',
      type: 'start',
      transitions: [{ name: 'to_offer_accepted', to: 'offer_accepted' }],
    },

    offer_accepted: {
      id: 'offer_accepted',
      type: 'state',
      name: 'Offer Accepted',
      transitions: [{ name: 'to_contract_prep', to: 'contract_preparation' }],
    },

    contract_preparation: {
      id: 'contract_preparation',
      type: 'task',
      name: 'Prepare Contract',
      candidateGroups: ['brokerage'],
      transitions: [
        { name: 'ready', to: 'contract_executed' },
        { name: 'cancel', to: 'transaction_cancelled' },
      ],
    },

    contract_executed: {
      id: 'contract_executed',
      type: 'task',
      name: 'Execute Contract',
      candidateGroups: ['brokerage'],
      transitions: [
        { name: 'executed', to: 'mark_under_contract' },
        { name: 'cancel', to: 'transaction_cancelled' },
      ],
    },

    mark_under_contract: {
      id: 'mark_under_contract',
      type: 'command',
      name: 'Mark Under Contract',
      commandType: 'deal.set_stage_under_contract',
      transition: 'to_under_contract',
      transitions: [{ name: 'to_under_contract', to: 'under_contract' }],
    },

    under_contract: {
      id: 'under_contract',
      type: 'state',
      name: 'Under Contract',
      transitions: [{ name: 'to_financing', to: 'financing_applicable' }],
    },

    financing_applicable: {
      id: 'financing_applicable',
      type: 'decision',
      name: 'Financing Applicable',
      decisions: [
        { condition: 'financingApplicable == true', transition: 'financed' },
        { condition: 'financingApplicable == false', transition: 'cash' },
      ],
      // `unresolved` is the default (first) transition: a null/unknown
      // financingApplicable fact does NOT silently become cash — it waits for
      // a human to resolve it, then re-evaluates after a fact refresh.
      transitions: [
        { name: 'unresolved', to: 'resolve_financing' },
        { name: 'financed', to: 'fork_financed' },
        { name: 'cash', to: 'fork_cash' },
      ],
    },

    resolve_financing: {
      id: 'resolve_financing',
      type: 'task',
      name: 'Resolve Financing Applicability',
      candidateGroups: ['brokerage'],
      transitions: [{ name: 'resolved', to: 'set_financing_type' }],
    },

    set_financing_type: {
      id: 'set_financing_type',
      type: 'command',
      name: 'Set Financing Type',
      commandType: 'deal.set_financing_type',
      transition: 'to_financing',
      transitions: [{ name: 'to_financing', to: 'financing_applicable' }],
    },

    fork_financed: {
      id: 'fork_financed',
      type: 'fork',
      transitions: [
        { name: 'inspection', to: 'inspection' },
        { name: 'title', to: 'title' },
        // Appraisal is an OPTIONAL milestone in V1: banks normally require it,
        // but a cash transaction may also have an appraisal. Appraisal
        // applicability is intentionally NOT coupled to financing.
        { name: 'appraisal', to: 'appraisal', required: false },
        { name: 'financing', to: 'financing' },
      ],
    },

    fork_cash: {
      id: 'fork_cash',
      type: 'fork',
      transitions: [
        { name: 'inspection', to: 'inspection_cash' },
        { name: 'title', to: 'title_cash' },
      ],
    },

    inspection: {
      id: 'inspection',
      type: 'task',
      name: 'Inspection',
      candidateGroups: ['inspector'],
      transitions: [
        { name: 'done', to: 'join_financed' },
        { name: 'blocker', to: 'inspection_blocker' },
        { name: 'fail', to: 'inspection_failed' },
      ],
    },

    inspection_blocker: {
      id: 'inspection_blocker',
      type: 'task',
      name: 'Inspection Blocker',
      candidateGroups: ['inspector'],
      transitions: [
        { name: 'resolved', to: 'inspection' },
        { name: 'escalate', to: 'inspection_failed' },
      ],
    },

    title: {
      id: 'title',
      type: 'task',
      name: 'Title',
      candidateGroups: ['title'],
      transitions: [
        { name: 'done', to: 'join_financed' },
        { name: 'blocker', to: 'title_blocker' },
        { name: 'fail', to: 'title_failed' },
      ],
    },

    title_blocker: {
      id: 'title_blocker',
      type: 'task',
      name: 'Title Blocker',
      candidateGroups: ['title'],
      transitions: [
        { name: 'resolved', to: 'title' },
        { name: 'escalate', to: 'title_failed' },
      ],
    },

    appraisal: {
      id: 'appraisal',
      type: 'task',
      name: 'Appraisal',
      candidateGroups: ['appraiser'],
      transitions: [
        { name: 'done', to: 'join_financed' },
        { name: 'fail', to: 'appraisal_failed' },
      ],
    },

    financing: {
      id: 'financing',
      type: 'task',
      name: 'Financing',
      candidateGroups: ['lender'],
      transitions: [
        { name: 'done', to: 'join_financed' },
        { name: 'fail', to: 'financing_failed' },
      ],
    },

    inspection_cash: {
      id: 'inspection_cash',
      type: 'task',
      name: 'Inspection',
      candidateGroups: ['inspector'],
      transitions: [
        { name: 'done', to: 'join_cash' },
        { name: 'blocker', to: 'inspection_cash_blocker' },
        { name: 'fail', to: 'inspection_failed' },
      ],
    },

    inspection_cash_blocker: {
      id: 'inspection_cash_blocker',
      type: 'task',
      name: 'Inspection Blocker',
      candidateGroups: ['inspector'],
      transitions: [
        { name: 'resolved', to: 'inspection_cash' },
        { name: 'escalate', to: 'inspection_failed' },
      ],
    },

    title_cash: {
      id: 'title_cash',
      type: 'task',
      name: 'Title',
      candidateGroups: ['title'],
      transitions: [
        { name: 'done', to: 'join_cash' },
        { name: 'blocker', to: 'title_cash_blocker' },
        { name: 'fail', to: 'title_failed' },
      ],
    },

    title_cash_blocker: {
      id: 'title_cash_blocker',
      type: 'task',
      name: 'Title Blocker',
      candidateGroups: ['title'],
      transitions: [
        { name: 'resolved', to: 'title_cash' },
        { name: 'escalate', to: 'title_failed' },
      ],
    },

    join_financed: {
      id: 'join_financed',
      type: 'join',
      transitions: [{ name: 'to_ready', to: 'ready_to_close' }],
    },

    join_cash: {
      id: 'join_cash',
      type: 'join',
      transitions: [{ name: 'to_ready', to: 'ready_to_close' }],
    },

    ready_to_close: {
      id: 'ready_to_close',
      type: 'state',
      name: 'Ready to Close',
      transitions: [{ name: 'to_closing', to: 'closing' }],
    },

    closing: {
      id: 'closing',
      type: 'task',
      name: 'Closing',
      candidateGroups: ['attorney'],
      transitions: [{ name: 'closed', to: 'mark_closed' }],
    },

    mark_closed: {
      id: 'mark_closed',
      type: 'command',
      name: 'Mark Closed',
      commandType: 'deal.set_stage_closed',
      transition: 'to_closed',
      transitions: [{ name: 'to_closed', to: 'closed' }],
    },

    closed: { id: 'closed', type: 'end', name: 'Closed', outcome: 'completed' },

    transaction_cancelled: {
      id: 'transaction_cancelled',
      type: 'end',
      name: 'Transaction Cancelled',
      outcome: 'cancelled',
    },

    inspection_failed: {
      id: 'inspection_failed',
      type: 'end',
      name: 'Inspection Failed',
      outcome: 'failed',
    },

    title_failed: {
      id: 'title_failed',
      type: 'end',
      name: 'Title Failed',
      outcome: 'failed',
    },

    appraisal_failed: {
      id: 'appraisal_failed',
      type: 'end',
      name: 'Appraisal Failed',
      outcome: 'failed',
    },

    financing_failed: {
      id: 'financing_failed',
      type: 'end',
      name: 'Financing Failed',
      outcome: 'failed',
    },
  },
}

/** Canonical milestone ids (financed fork uses the unsuffixed ids). */
export const TRANSACTION_MILESTONE_IDS = [
  'inspection',
  'title',
  'appraisal',
  'financing',
] as const

/** Which milestones are required vs conditional (business policy). */
export const TRANSACTION_MILESTONE_REQUIRED: Record<string, boolean> = {
  inspection: true,
  title: true,
  appraisal: false, // conditional on financing
  financing: false, // conditional on financing
}

/** Maps any node id back to its canonical milestone id. */
export const TRANSACTION_MILESTONE_BASE: Record<string, string> = {
  inspection: 'inspection',
  inspection_cash: 'inspection',
  inspection_blocker: 'inspection',
  inspection_cash_blocker: 'inspection',
  title: 'title',
  title_cash: 'title',
  title_blocker: 'title',
  title_cash_blocker: 'title',
  appraisal: 'appraisal',
  financing: 'financing',
}

/** Linear, human-readable progression for timeline rendering. */
export const TRANSACTION_CLOSE_V1_ORDER = [
  'offer_accepted',
  'contract_preparation',
  'contract_executed',
  'mark_under_contract',
  'under_contract',
  'financing_applicable',
  'inspection',
  'title',
  'appraisal',
  'financing',
  'ready_to_close',
  'closing',
  'closed',
] as const
