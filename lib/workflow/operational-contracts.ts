// Pre-CRM-14 integration plumbing — operational seams.
// Task/timer/alert/SME/audit boundaries. No delivery, no worker, no engine.

// ---------------------------------------------------------------------------
// Task integration contract (Story 51)
// ---------------------------------------------------------------------------
// CulebraLuxe `task` remains the canonical user-facing operational work item.
// The engine keeps its own runtime task state. Correlation, not dual truth.

export type TaskCorrelation = {
  workflowTaskId: string
  applicationTaskId: string
  subjectType: string
  subjectId: string
}

// ---------------------------------------------------------------------------
// Timer / deadline contract (Story 52)
// ---------------------------------------------------------------------------
// Application owns business dates/facts; the engine may schedule orchestration
// timers that reference those facts. No worker implemented here.

export type TimerSpec = {
  timerId: string
  subjectType: string
  subjectId: string
  deadlineAt: string // ISO UTC, derived from application business date
  correlationId: string
}

export type TimerExpiration = {
  timerId: string
  firedAt: string // ISO UTC
  expired: boolean
  rescheduleTo: string | null
}

// ---------------------------------------------------------------------------
// Alert / notification event seam (Story 53)
// ---------------------------------------------------------------------------

export type TechnicalAlertKind =
  | 'ENGINE_FAILURE'
  | 'STUCK_TOKEN'
  | 'EXHAUSTED_RETRIES'
  | 'INVARIANT_VIOLATION'
  | 'DEAD_LETTER'

export type BusinessNotificationKind =
  | 'MILESTONE_DUE'
  | 'OVERDUE_ACTION'
  | 'CLIENT_ACTION_REQUIRED'
  | 'SME_WAITING'
  | 'BUSINESS_BLOCKER'

export type AlertEvent = {
  eventId: string
  occurredAt: string
  subjectType: string | null
  subjectId: string | null
  kind: TechnicalAlertKind | BusinessNotificationKind
  correlationId: string | null
  payload: Record<string, unknown>
}

// Alert/notification policy is a pure mapping from AlertEvent → future channel
// adapter calls. No delivery built.

// ---------------------------------------------------------------------------
// External SME participant contract (Story 54)
// ---------------------------------------------------------------------------
// Business participant identity is domain data (person/deal_participant), not
// authentication identity. An SME only needs an app_user/auth identity if they
// actually log in.

export type SmeRole = 'inspector' | 'appraiser' | 'title' | 'attorney' | 'lender' | 'contractor'

export type SmeParticipant = {
  subjectType: string
  subjectId: string
  smeRole: SmeRole
  personId: string | null // existing CRM person, when known
  displayName: string
  contactEmail: string | null
  contactPhone: string | null
}

// ---------------------------------------------------------------------------
// Application event audit strategy (Story 58)
// ---------------------------------------------------------------------------
// Prefer least new persistence:
//   - Existing immutable domain records (interaction, showing lifecycle,
//     task history, offer) already provide the business audit trail.
//   - `security_audit_event` covers security-significant auth events only.
//   - DomainEvents are initially transient (returned on CommandResult); the
//     workflow engine maintains its own event log keyed by eventId/correlation.
//   - No new application_event table is created unless a durable application-
//     side event log is later required (engine-archaeology decision).
