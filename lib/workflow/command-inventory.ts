// Pre-CRM-14 integration plumbing — domain command inventory (Story 41),
// idempotency catalog (Story 48), and precondition boundary (Story 50).

import type { AggregateType } from './contracts'

export type IdempotencyClass =
  | 'A' // naturally idempotent
  | 'B' // guarded by unique source key
  | 'C' // safe retry with state check
  | 'D' // not currently safe to retry

export type DomainCommand = {
  name: string
  aggregate: AggregateType
  authority: string // coarse authority code
  changesBusinessState: boolean
  workflowAware: boolean // should eventually emit a domain event
  idempotency: IdempotencyClass
  idempotencyNote: string
  precondition: string | null
}

// Canonical inventory of existing consequential commands. No command behavior
// is redesigned; this is a descriptive catalog.
export const DOMAIN_COMMANDS: DomainCommand[] = [
  {
    name: 'submitOffer',
    aggregate: 'offer',
    authority: 'deal.write',
    changesBusinessState: true,
    workflowAware: true,
    idempotency: 'C',
    idempotencyNote: 'No unique source key; a retry would create a duplicate offer unless guarded by a client correlation id.',
    precondition: 'Deal must exist; offer amount must be positive; parent offer (counter) must be submitted.',
  },
  {
    name: 'withdrawOffer',
    aggregate: 'offer',
    authority: 'deal.write',
    changesBusinessState: true,
    workflowAware: true,
    idempotency: 'C',
    idempotencyNote: 'Withdrawing an already-withdrawn offer is a no-op only if a state check is performed first.',
    precondition: 'Offer must be in a withdrawable (submitted/countered) state.',
  },
  {
    name: 'rejectOffer',
    aggregate: 'offer',
    authority: 'deal.write',
    changesBusinessState: true,
    workflowAware: true,
    idempotency: 'C',
    idempotencyNote: 'State check on offer status before transition.',
    precondition: 'Offer must be actionable (submitted/countered).',
  },
  {
    name: 'createShowing',
    aggregate: 'showing',
    authority: 'deal.write',
    changesBusinessState: true,
    workflowAware: true,
    idempotency: 'C',
    idempotencyNote: 'No unique source key; retries duplicate unless correlated.',
    precondition: 'Person must exist; property/deal optional.',
  },
  {
    name: 'scheduleShowing',
    aggregate: 'showing',
    authority: 'deal.write',
    changesBusinessState: true,
    workflowAware: true,
    idempotency: 'C',
    idempotencyNote: 'Transition guarded by status = requested.',
    precondition: 'Showing must be in requested state.',
  },
  {
    name: 'completeShowing',
    aggregate: 'showing',
    authority: 'deal.write',
    changesBusinessState: true,
    workflowAware: true,
    idempotency: 'B',
    idempotencyNote: 'Emitted interaction is idempotent via (source_system=showing, source_external_id=showing.id); showing transition is guarded by status.',
    precondition: 'Showing must be requested or scheduled.',
  },
  {
    name: 'cancelShowing',
    aggregate: 'showing',
    authority: 'deal.write',
    changesBusinessState: true,
    workflowAware: true,
    idempotency: 'C',
    idempotencyNote: 'State check on status in (requested, scheduled).',
    precondition: 'Showing must be requested or scheduled.',
  },
  {
    name: 'addOtherParticipant',
    aggregate: 'deal_participant',
    authority: 'deal.write',
    changesBusinessState: true,
    workflowAware: true,
    idempotency: 'C',
    idempotencyNote: 'No unique constraint on (deal, person, role); retries can duplicate.',
    precondition: 'Deal must exist.',
  },
  {
    name: 'endParticipant',
    aggregate: 'deal_participant',
    authority: 'deal.write',
    changesBusinessState: true,
    workflowAware: true,
    idempotency: 'C',
    idempotencyNote: 'State check on active flag.',
    precondition: 'Participant must be active.',
  },
  {
    name: 'createTask',
    aggregate: 'task',
    authority: 'crm.write',
    changesBusinessState: true,
    workflowAware: true,
    idempotency: 'C',
    idempotencyNote: 'No unique source key; retries duplicate.',
    precondition: 'Title required; optional person/property/deal context.',
  },
  {
    name: 'completeTask',
    aggregate: 'task',
    authority: 'crm.write',
    changesBusinessState: true,
    workflowAware: true,
    idempotency: 'C',
    idempotencyNote: 'State check on status = open.',
    precondition: 'Task must be open.',
  },
  {
    name: 'cancelTask',
    aggregate: 'task',
    authority: 'crm.write',
    changesBusinessState: true,
    workflowAware: true,
    idempotency: 'C',
    idempotencyNote: 'State check on status = open.',
    precondition: 'Task must be open.',
  },
  {
    name: 'logManualInteraction',
    aggregate: 'interaction',
    authority: 'crm.write',
    changesBusinessState: true,
    workflowAware: true,
    idempotency: 'B',
    idempotencyNote: 'Guarded by unique (source_system, source_external_id) when both are provided.',
    precondition: 'Person required; title or summary required.',
  },
  {
    name: 'updatePropertyFacts',
    aggregate: 'property',
    authority: 'listing.write',
    changesBusinessState: true,
    workflowAware: true,
    idempotency: 'A',
    idempotencyNote: 'Absolute full-save update; retrying with the same input is naturally idempotent.',
    precondition: 'Property must exist; name required; numeric ranges validated.',
  },
  {
    name: 'updatePropertyVisibility',
    aggregate: 'property',
    authority: 'listing.write',
    changesBusinessState: true,
    workflowAware: true,
    idempotency: 'A',
    idempotencyNote: 'Absolute set of featured + safe status.',
    precondition: 'Status must be a safe editorial status; transaction statuses rejected.',
  },
  {
    name: 'rejectIntake',
    aggregate: 'interaction',
    authority: 'crm.write',
    changesBusinessState: true,
    workflowAware: true,
    idempotency: 'C',
    idempotencyNote: 'State check on submission status.',
    precondition: 'Submission must be received or resolution_required.',
  },
  {
    name: 'attachIntakeToPerson',
    aggregate: 'person',
    authority: 'crm.write',
    changesBusinessState: true,
    workflowAware: true,
    idempotency: 'C',
    idempotencyNote: 'Identity-sensitive; guarded by explicit person selection, not automatic.',
    precondition: 'Submission unresolved; target person must exist.',
  },
  {
    name: 'updatePersonStatus',
    aggregate: 'person',
    authority: 'crm.write',
    changesBusinessState: true,
    workflowAware: false,
    idempotency: 'A',
    idempotencyNote: 'Absolute status set.',
    precondition: 'Status must be one of new/warm/active/referral.',
  },
]
