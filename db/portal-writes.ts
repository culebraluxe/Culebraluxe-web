import { sql } from './client'
import { PortalWriteError } from '../lib/portal-write-error'

// Bounded Portal write service (V1.1). Each function is a canonical, typed
// write against existing tables. No identity resolution/creation, no person
// creation, no participant client/owner/seller writes, no offer accept, no
// deal-stage changes, no CRM-14. Multi-row invariants use explicit
// transactions or a single atomic statement.
//
// Needs Review resolution (attach/create/reject) lives behind the single
// CRM-09B seam db/needs-review-resolution.ts (resolveIntake); the former
// rejectIntake/attachIntakeToPerson functions were refactored into it.

// ---------------------------------------------------------------
// STORY 1 — Task operations
// ---------------------------------------------------------------

export async function completeTask(
  taskId: string,
): Promise<{ taskId: string; status: 'completed' }> {
  const rows = await sql`
    update task
    set status = 'completed',
        completed_at = now(),
        updated_at = now()
    where id = ${taskId}
      and status = 'open'
    returning id
  `
  if (rows.length === 0) {
    throw new PortalWriteError('conflict', 'Task not found or already resolved.')
  }
  return { taskId, status: 'completed' }
}

export async function cancelTask(
  taskId: string,
): Promise<{ taskId: string; status: 'cancelled' }> {
  const rows = await sql`
    update task
    set status = 'cancelled',
        completed_at = null,
        updated_at = now()
    where id = ${taskId}
      and status = 'open'
    returning id
  `
  if (rows.length === 0) {
    throw new PortalWriteError('conflict', 'Task not found or already resolved.')
  }
  return { taskId, status: 'cancelled' }
}

export async function updateTaskDue(taskId: string, dueAt: string | null) {
  const rows = await sql`
    update task
    set due_at = ${dueAt},
        updated_at = now()
    where id = ${taskId}
    returning id
  `
  if (rows.length === 0) {
    throw new PortalWriteError('not-found', 'Task not found.')
  }
  return { taskId, dueAt }
}

// Bounded Catch-Up task edit — title / detail / due / priority only. This is the
// everyday operator update; it never touches person/property/deal context, owner
// assignment rules, or lifecycle status. The action layer always supplies all
// four fields (detail/due may be null to clear them). Reuses the same canonical
// task write service as completeTask/updateTaskDue — no second task mutation
// system.
export async function updateTask(
  taskId: string,
  input: {
    title: string
    detail: string | null
    dueAt: string | null
    priority: number
    workstream?: string | null
    category?: string | null
  },
): Promise<{ taskId: string }> {
  if (!input.title.trim()) {
    throw new PortalWriteError('validation', 'Task title is required.')
  }
  const priority = input.priority
  if (!Number.isInteger(priority) || priority < 0 || priority > 32767) {
    throw new PortalWriteError(
      'validation',
      'Task priority must be an integer between 0 and 32767.',
    )
  }

  const rows = await sql`
    update task
    set title = ${input.title.trim()},
        detail = ${input.detail},
        due_at = ${input.dueAt},
        priority = ${priority},
        workstream = coalesce(${input.workstream ?? null}, workstream),
        category = coalesce(${input.category ?? null}, category),
        updated_at = now()
    where id = ${taskId}
    returning id
  `
  if (rows.length === 0) {
    throw new PortalWriteError('not-found', 'Task not found.')
  }
  return { taskId }
}

// ---------------------------------------------------------------
// STORY 2 — Manual interaction / note logging (append-only)
// ---------------------------------------------------------------

export async function logManualInteraction(input: {
  personId: string
  channel: 'manual' | 'note'
  occurredAt: string
  title?: string
  summary?: string
  propertyId?: string
  dealId?: string
}) {
  const rows = await sql`
    insert into interaction (
      person_id, property_id, deal_id, channel, event_type, direction,
      occurred_at, title, summary, source_metadata
    ) values (
      ${input.personId}, ${input.propertyId ?? null}, ${input.dealId ?? null},
      ${input.channel}, ${input.channel}, null,
      ${input.occurredAt}, ${input.title ?? null}, ${input.summary ?? null},
      '{}'::jsonb
    )
    returning id
  `
  const id = (rows[0] as { id: string } | undefined)?.id
  if (!id) throw new Error('Interaction could not be created.')
  return { interactionId: id }
}

// ---------------------------------------------------------------
// STORY 3 — Showing create / schedule / cancel
//
// request_source_interaction_id records the viewing-request interaction a
// showing originates from (schema column 013). It is OPTIONAL and only set
// when the showing is created from an existing source interaction; otherwise
// it stays null. The portal has no automatic viewing-request -> showing
// conversion flow, so this is populated by callers that have the source
// interaction at hand.
// ---------------------------------------------------------------

type ShowingRow = {
  id: string
  person_id: string
  property_id: string | null
  deal_id: string | null
  status: string
  requested_at: string
  scheduled_at: string | null
  completed_at: string | null
  cancelled_at: string | null
  feedback: string | null
}

function mapShowing(row: ShowingRow) {
  return {
    id: row.id,
    personId: row.person_id,
    propertyId: row.property_id ?? null,
    dealId: row.deal_id ?? null,
    status: row.status,
    requestedAt: row.requested_at,
    scheduledAt: row.scheduled_at ?? null,
    completedAt: row.completed_at ?? null,
    cancelledAt: row.cancelled_at ?? null,
    feedback: row.feedback ?? null,
  }
}

export async function createShowing(input: {
  personId: string
  propertyId?: string
  dealId?: string
  requestedAt?: string
  requestSourceInteractionId?: string
}) {
  // Defect fix (CRM-11): requested_at is NOT NULL with DEFAULT now().
  // Passing an explicit NULL for the omitted optional requestedAt would
  // override the column default and violate the constraint — coalesce to
  // now() so the documented default semantics hold for the portal path
  // (which never supplies requestedAt).
  const rows = await sql`
    insert into showing (
      person_id, property_id, deal_id, status, requested_at,
      request_source_interaction_id
    )
    values (
      ${input.personId}, ${input.propertyId ?? null}, ${input.dealId ?? null},
      'requested', coalesce(${input.requestedAt ?? null}::timestamptz, now()),
      ${input.requestSourceInteractionId ?? null}
    )
    returning id, person_id, property_id, deal_id, status,
      requested_at, scheduled_at, completed_at, cancelled_at, feedback
  `
  const row = rows[0] as ShowingRow
  if (!row) throw new Error('Showing could not be created.')
  return mapShowing(row)
}

export async function scheduleShowing(showingId: string, scheduledAt: string) {
  const rows = await sql`
    update showing
    set status = 'scheduled',
        scheduled_at = ${scheduledAt}::timestamptz,
        updated_at = now()
    where id = ${showingId}
      and status = 'requested'
    returning id, person_id, property_id, deal_id, status,
      requested_at, scheduled_at, completed_at, cancelled_at, feedback
  `
  const row = rows[0] as ShowingRow | undefined
  if (!row) {
    throw new PortalWriteError(
      'conflict',
      'Showing not found or not in requested state.',
    )
  }
  return mapShowing(row)
}

export async function cancelShowing(showingId: string) {
  const rows = await sql`
    update showing
    set status = 'cancelled',
        cancelled_at = now(),
        updated_at = now()
    where id = ${showingId}
      and status in ('requested', 'scheduled')
    returning id, person_id, property_id, deal_id, status,
      requested_at, scheduled_at, completed_at, cancelled_at, feedback
  `
  const row = rows[0] as ShowingRow | undefined
  if (!row) {
    throw new PortalWriteError('conflict', 'Showing not found or already resolved.')
  }
  return mapShowing(row)
}

// ---------------------------------------------------------------
// STORY 4 — Showing complete (atomic dual-write, idempotent)
//
// Canonical completion rule (migration 013, documented): on `completed` emit
// exactly ONE interaction with channel = 'showing', occurred_at =
// completed_at ?? scheduled_at, person/property/deal copied from the showing
// row, idempotent via source_system='showing' / source_external_id=showing.id
// (partial unique index interaction_source_identity_unique). requested /
// scheduled / cancelled transitions emit no timeline interaction.
// ---------------------------------------------------------------

export async function completeShowing(showingId: string, completedAt?: string) {
  const completedTs = completedAt ?? new Date().toISOString()

  // Single atomic statement: transition the showing to completed and emit
  // exactly one immutable 'showing' interaction keyed by showing.id. Repeated
  // invocation cannot duplicate the interaction (unique source identity) and
  // is a no-op once the showing is already completed.
  await sql`
    with updated as (
      update showing
      set status = 'completed',
          completed_at = ${completedTs}::timestamptz,
          updated_at = now()
      where id = ${showingId}
        and status in ('requested', 'scheduled')
      returning id, person_id, property_id, deal_id, scheduled_at
    )
    insert into interaction (
      person_id, property_id, deal_id, channel, event_type,
      occurred_at, title, source_system, source_external_id, source_metadata
    )
    select
      u.person_id, u.property_id, u.deal_id, 'showing', 'showing_completed',
      coalesce(${completedTs}::timestamptz, u.scheduled_at),
      'Showing completed', 'showing', ${showingId}, '{}'::jsonb
    from updated u
    on conflict (source_system, source_external_id)
      where source_system is not null and source_external_id is not null
    do nothing
  `

  const rows = await sql`
    select id, person_id, property_id, deal_id, status,
      requested_at, scheduled_at, completed_at, cancelled_at, feedback
    from showing
    where id = ${showingId}
    limit 1
  `
  const row = rows[0] as ShowingRow | undefined
  if (!row) throw new PortalWriteError('not-found', 'Showing not found.')
  return mapShowing(row)
}

// ---------------------------------------------------------------
// STORY 5 — Offer operations (submit / withdraw / reject)
//
// Recorded asymmetry (CRM-11): offer ACCEPT is intentionally NOT a portal
// server action here. Accept is the canonical application command
// offer.accept (db/offer-acceptance.ts, thin handler lib/commands/offer/
// accept-offer.ts) reachable through the command seam (workflow_app command
// router -> canonical dispatcher) with claim-first receipts, idempotency and
// the one-accepted/primary-offer-per-deal invariant. submit/withdraw/reject
// are plain portal writes (this file). Deal stage is NEVER auto-advanced by
// any of these writes — it changes only via explicit deal.set_stage commands.
// ---------------------------------------------------------------

export async function submitOffer(input: {
  dealId: string
  personId: string
  amount: number
  parentOfferId?: string
}) {
  if (!Number.isFinite(input.amount) || input.amount <= 0) {
    throw new PortalWriteError('validation', 'Offer amount must be a positive number.')
  }

  if (input.parentOfferId) {
    const parentRows = await sql`
      select deal_id from offer where id = ${input.parentOfferId} limit 1
    `
    const parent = parentRows[0] as { deal_id: string } | undefined
    if (!parent) {
      throw new PortalWriteError('not-found', 'Parent offer not found.')
    }
    if (parent.deal_id !== input.dealId) {
      throw new PortalWriteError(
        'validation',
        'Parent offer must belong to the same deal.',
      )
    }
  }

  const rows = await sql`
    insert into offer (deal_id, person_id, parent_offer_id, amount, status)
    values (
      ${input.dealId}, ${input.personId}, ${input.parentOfferId ?? null},
      ${input.amount}, 'submitted'
    )
    returning id
  `
  const id = (rows[0] as { id: string } | undefined)?.id
  if (!id) throw new Error('Offer could not be created.')
  return { offerId: id }
}

export async function withdrawOffer(
  offerId: string,
): Promise<{ offerId: string; status: 'withdrawn' }> {
  const rows = await sql`
    update offer
    set status = 'withdrawn', responded_at = now(), updated_at = now()
    where id = ${offerId}
      and status = 'submitted'
    returning id
  `
  if (rows.length === 0) {
    throw new PortalWriteError(
      'conflict',
      'Offer not found or not in submitted state.',
    )
  }
  return { offerId, status: 'withdrawn' }
}

export async function rejectOffer(
  offerId: string,
): Promise<{ offerId: string; status: 'rejected' }> {
  const rows = await sql`
    update offer
    set status = 'rejected', responded_at = now(), updated_at = now()
    where id = ${offerId}
      and status = 'submitted'
    returning id
  `
  if (rows.length === 0) {
    throw new PortalWriteError(
      'conflict',
      'Offer not found or not in submitted state.',
    )
  }
  return { offerId, status: 'rejected' }
}

// ---------------------------------------------------------------
// STORY 6 — Needs Review safe resolution
// Refactored into the single CRM-09B seam db/needs-review-resolution.ts
// (resolveIntake with action attach | create | reject). See that module.
// ---------------------------------------------------------------

// ---------------------------------------------------------------
// STORY 7 — Participant role='other' writes
// Refactored into the canonical CRM-13 seam db/deal-participants.ts
// (addOtherParticipant / endParticipant / updateParticipantRoleLabel),
// which enforces the one-active-structural-role and one-active-role_label
// invariants. See that module.
// ---------------------------------------------------------------

// ---------------------------------------------------------------
// STORY 8 — Client note / relationship status
// ---------------------------------------------------------------

const VALID_STATUSES = new Set(['new', 'warm', 'active', 'referral'])

export async function updatePersonNotes(personId: string, notes: string | null) {
  const rows = await sql`
    update person
    set notes = ${notes === null ? null : notes.trim() || null},
        updated_at = now()
    where id = ${personId} and archived_at is null
    returning id
  `
  if (rows.length === 0) {
    throw new PortalWriteError('not-found', 'Person not found.')
  }
  return { personId }
}

export async function updatePersonStatus(personId: string, status: string) {
  if (!VALID_STATUSES.has(status)) {
    throw new PortalWriteError('validation', 'Relationship status is invalid.')
  }
  const rows = await sql`
    update person
    set status = ${status}, updated_at = now()
    where id = ${personId} and archived_at is null
    returning id
  `
  if (rows.length === 0) {
    throw new PortalWriteError('not-found', 'Person not found.')
  }
  return { personId, status }
}
