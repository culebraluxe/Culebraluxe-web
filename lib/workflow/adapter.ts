// Pre-CRM-14 integration plumbing — the application-side interface the future
// workflow engine will consume, plus a narrow application-service facade.
//
// No old-engine implementation classes are referenced. These are pure contracts.

import type {
  CommandEnvelope,
  CommandResult,
  DomainEvent,
  WorkflowSubject,
} from './contracts'

// ---------------------------------------------------------------------------
// Application service facade (Story 46)
// UI/server action OR future workflow adapter both route through here.
// ---------------------------------------------------------------------------

export type ApplicationFacade = {
  executeCommand(envelope: CommandEnvelope): Promise<CommandResult>
  readSubject(subject: WorkflowSubject): Promise<Record<string, unknown>>
  readFacts(subject: WorkflowSubject): Promise<Record<string, unknown>>
}

// ---------------------------------------------------------------------------
// Workflow adapter interface (Story 56)
// ---------------------------------------------------------------------------

export type WorkflowAdapter = {
  // Request a canonical application command (authority + domain validation
  // remain inside the application).
  executeCommand(envelope: CommandEnvelope): Promise<CommandResult>

  // Deterministic subject facts for engine decisions (compact, typed).
  readSubject(subject: WorkflowSubject): Promise<Record<string, unknown>>
  readFacts(subject: WorkflowSubject): Promise<Record<string, unknown>>

  // Human-task correlation (CulebraLuxe task stays canonical user-facing item).
  materializeHumanTask(correlation: {
    workflowTaskId: string
    applicationTaskId: string
    subject: WorkflowSubject
  }): Promise<void>

  completeHumanTaskCorrelation(correlation: {
    workflowTaskId: string
  }): Promise<void>

  // Application-domain events for the engine's own event log. The application
  // does not persist engine runtime state.
  publishApplicationEvent(event: DomainEvent): Promise<void>
}

// ---------------------------------------------------------------------------
// Workflow fact projection (Story 57) — compact, deterministic, deal-focused.
// ---------------------------------------------------------------------------

export type DealWorkflowFacts = {
  dealId: string
  stage: string
  listPrice: number | null
  offerPrice: number | null
  property: {
    id: string
    name: string
    propertyType: string | null
    status: string
  } | null
  client: { id: string; name: string } | null
  offers: Array<{ id: string; amount: number; status: string; parentOfferId: string | null }>
  showings: Array<{ id: string; status: string }>
  openTasks: Array<{ id: string; title: string; dueAt: string | null }>
  participants: Array<{ id: string; roleCategory: string; roleLabel: string | null }>
}
