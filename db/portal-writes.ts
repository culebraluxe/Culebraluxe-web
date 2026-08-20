import { randomUUID } from 'node:crypto'

import { sql } from './client'
import { PortalWriteError } from '../lib/portal-write-error'

// Bounded Portal write service (V1.1). Each function is a canonical, typed
// write against existing tables. No identity resolution/creation, no person
// creation, no participant client/owner/seller writes, no offer accept, no
// deal-stage changes, no CRM-14. Multi-row invariants use explicit
// transactions or a single atomic statement.

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
}) {
  const rows = await sql`
    insert into showing (person_id, property_id, deal_id, status, requested_at)
    values (
      ${input.personId}, ${input.propertyId ?? null}, ${input.dealId ?? null},
      'requested', ${input.requestedAt ?? null}
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
// STORY 5 — Offer operations (no accept)
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
// STORY 6 — Needs Review safe resolution (no identity writes)
// ---------------------------------------------------------------

export async function rejectIntake(submissionId: string) {
  const rows = await sql`
    update website_intake_submission
    set status = 'rejected', processing_started_at = null, updated_at = now()
    where id = ${submissionId}
      and status in ('received', 'resolution_required')
    returning id
  `
  if (rows.length === 0) {
    throw new PortalWriteError(
      'conflict',
      'Submission not found or not actionable.',
    )
  }
  return { submissionId, status: 'rejected' }
}

// Attach an intake submission to an explicitly selected existing person.
// Single atomic CTE: resolves the canonical interaction id (insert or existing
// via the partial unique source identity index), inserts exactly one human
// task tied to that interaction, and completes the receipt with the actual
// persisted interaction id. No person creation, no identity resolution, no
// property_interest side effects.
export async function attachIntakeToPerson(
  submissionId: string,
  personId: string,
) {
  const intakeRows = await sql`
    select
      id, request_type, property_id, display_name, email, message, status
    from website_intake_submission
    where id = ${submissionId}
      and status in ('received', 'resolution_required')
    limit 1
  `
  const intake = intakeRows[0] as
    | {
        id: string
        request_type: string
        property_id: string | null
        display_name: string
        email: string | null
        message: string | null
        status: string
      }
    | undefined
  if (!intake) {
    throw new PortalWriteError(
      'conflict',
      'Submission not found or not actionable.',
    )
  }

  const personRows = await sql`
    select id from person where id = ${personId} and archived_at is null limit 1
  `
  if (personRows.length === 0) {
    throw new PortalWriteError('not-found', 'Selected person does not exist.')
  }

  const candidateInteractionId = randomUUID()
  const eventType =
    intake.request_type === 'private_viewing'
      ? 'private_viewing_requested'
      : intake.request_type === 'property_information'
        ? 'property_inquiry_submitted'
        : 'general_enquiry_submitted'
  const title =
    intake.request_type === 'private_viewing'
      ? 'Private viewing request'
      : intake.request_type === 'property_information'
        ? 'Property information request'
        : 'General enquiry'
  const taskTitle = `Follow up on ${title.toLowerCase()} from ${intake.display_name}`

  // `resolved` always yields exactly one row: the newly inserted interaction
  // id or, on source-identity conflict, the existing canonical id.
  const resolvedRows = await sql`
    with resolved as (
      insert into interaction (
        id, person_id, property_id, channel, event_type, direction,
        occurred_at, title, summary, source_system, source_external_id,
        source_metadata
      ) values (
        ${candidateInteractionId}, ${personId}, ${intake.property_id ?? null},
        'website', ${eventType}, 'inbound', now(), ${title},
        ${intake.message ?? null}, 'website', ${intake.id},
        ${JSON.stringify({ requestType: intake.request_type })}::jsonb
      )
      on conflict (source_system, source_external_id)
        where source_system is not null and source_external_id is not null
      do update set id = interaction.id
      returning id
    ),
    task_ins as (
      insert into task (
        title, detail, person_id, property_id, source_interaction_id,
        task_kind, priority
      )
      select ${taskTitle}, ${intake.message ?? null}, ${personId},
        ${intake.property_id ?? null}, r.id, 'human', 0
      from resolved r
      where not exists (
        select 1 from task where source_interaction_id = r.id
      )
      returning id
    ),
    receipt as (
      update website_intake_submission
      set status = 'completed',
          processing_started_at = null,
          interaction_id = r.id,
          updated_at = now()
      from resolved r
      where website_intake_submission.id = ${submissionId}
        and website_intake_submission.status in ('received', 'resolution_required')
      returning website_intake_submission.id
    )
    select id from resolved
  `

  const resolved = resolvedRows[0] as { id: string } | undefined
  if (!resolved) {
    throw new Error('Canonical interaction could not be resolved.')
  }

  return { submissionId, interactionId: resolved.id, personId }
}

// ---------------------------------------------------------------
// STORY 7 — Participant role='other' writes
// ---------------------------------------------------------------

export async function addOtherParticipant(input: {
  dealId: string
  personId?: string
  userId?: string
  roleLabel: string
}) {
  if (!input.personId && !input.userId) {
    throw new PortalWriteError(
      'validation',
      'Exactly one of person or user is required.',
    )
  }
  if (input.personId && input.userId) {
    throw new PortalWriteError(
      'validation',
      'Exactly one of person or user is required.',
    )
  }
  const roleLabel = input.roleLabel.trim()
  if (!roleLabel) {
    throw new PortalWriteError(
      'validation',
      'A role label is required for other participants.',
    )
  }
  if (roleLabel.length > 120) {
    throw new PortalWriteError('validation', 'Role label is too long.')
  }

  const rows = await sql`
    insert into deal_participant (deal_id, person_id, user_id, role, role_label, active)
    values (
      ${input.dealId}, ${input.personId ?? null}, ${input.userId ?? null},
      'other', ${roleLabel}, true
    )
    returning id
  `
  const id = (rows[0] as { id: string } | undefined)?.id
  if (!id) throw new Error('Participant could not be created.')
  return { participantId: id }
}

export async function endParticipant(participantId: string) {
  const rows = await sql`
    update deal_participant
    set active = false, ended_at = now(), updated_at = now()
    where id = ${participantId}
      and active = true
    returning id
  `
  if (rows.length === 0) {
    throw new PortalWriteError(
      'conflict',
      'Participant not found or already ended.',
    )
  }
  return { participantId }
}

export async function updateParticipantRoleLabel(
  participantId: string,
  roleLabel: string,
) {
  const normalized = roleLabel.trim()
  if (!normalized) {
    throw new PortalWriteError('validation', 'Role label is required.')
  }
  if (normalized.length > 120) {
    throw new PortalWriteError('validation', 'Role label is too long.')
  }

  const rows = await sql`
    update deal_participant
    set role_label = ${normalized}, updated_at = now()
    where id = ${participantId}
      and role = 'other'
    returning id
  `
  if (rows.length === 0) {
    throw new PortalWriteError(
      'conflict',
      'Participant not found or not role=other.',
    )
  }
  return { participantId }
}

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
