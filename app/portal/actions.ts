'use server'

import { revalidatePath } from 'next/cache'

import { PortalWriteError } from '@/lib/portal-write-error'
import { createTask } from '@/db/tasks'
import {
  addOtherParticipant,
  attachIntakeToPerson,
  cancelShowing,
  cancelTask,
  completeShowing,
  completeTask,
  createShowing,
  endParticipant,
  logManualInteraction,
  rejectIntake,
  rejectOffer,
  scheduleShowing,
  submitOffer,
  updateParticipantRoleLabel,
  updatePersonNotes,
  updatePersonStatus,
  updateTaskDue,
  withdrawOffer,
} from '@/db/portal-writes'

// Bounded Portal V1.1 write actions. Server-side validation happens here; the
// SQL stays in the db layer. Every action returns a discriminated result that
// distinguishes validation errors, conflicts, not-found, and unknown failures.

export type PortalWriteErrorCode =
  | 'validation'
  | 'conflict'
  | 'not-found'
  | 'unknown'

export type PortalWriteResult<T> =
  | { ok: true; data: T }
  | { ok: false; code: PortalWriteErrorCode; message: string }

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

function isUuid(value: string): boolean {
  return UUID_PATTERN.test(value)
}

function failure(
  error: unknown,
): { ok: false; code: PortalWriteErrorCode; message: string } {
  const message = error instanceof Error ? error.message : 'Unknown error.'
  const code: PortalWriteErrorCode =
    error instanceof PortalWriteError ? error.code : 'unknown'
  return { ok: false, code, message }
}

function revalidatePortal() {
  // Revalidate the entire portal subtree so deal workspaces, client dossiers,
  // showings, and the needs-review queue reflect the write immediately.
  revalidatePath('/portal', 'layout')
}

// ---------------------------------------------------------------
// STORY 1 — Task operations
// ---------------------------------------------------------------

export async function createTaskAction(input: {
  title: string
  detail?: string | null
  personId?: string | null
  propertyId?: string | null
  dealId?: string | null
  dueAt?: string | null
  priority?: number | null
}): Promise<PortalWriteResult<{ id: string }>> {
  const title = input.title?.trim()
  if (!title) {
    return {
      ok: false,
      code: 'validation',
      message: 'Task title is required.',
    }
  }
  const contextIds = [
    input.personId,
    input.propertyId,
    input.dealId,
  ].filter(Boolean) as string[]
  if (contextIds.some((id) => !isUuid(id))) {
    return {
      ok: false,
      code: 'validation',
      message: 'Task context identifiers are invalid.',
    }
  }
  try {
    const task = await createTask({
      title,
      detail: input.detail?.trim() || undefined,
      personId: input.personId ?? undefined,
      propertyId: input.propertyId ?? undefined,
      dealId: input.dealId ?? undefined,
      dueAt: input.dueAt ?? undefined,
      priority: input.priority ?? undefined,
      taskKind: 'human',
    })
    revalidatePortal()
    return { ok: true, data: { id: task.id } }
  } catch (error) {
    return failure(error)
  }
}

export async function completeTaskAction(
  taskId: string,
): Promise<PortalWriteResult<{ taskId: string; status: 'completed' }>> {
  if (!isUuid(taskId)) {
    return { ok: false, code: 'validation', message: 'Invalid task identifier.' }
  }
  try {
    const result = await completeTask(taskId)
    revalidatePortal()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

export async function cancelTaskAction(
  taskId: string,
): Promise<PortalWriteResult<{ taskId: string; status: 'cancelled' }>> {
  if (!isUuid(taskId)) {
    return { ok: false, code: 'validation', message: 'Invalid task identifier.' }
  }
  try {
    const result = await cancelTask(taskId)
    revalidatePortal()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

export async function updateTaskDueAction(
  taskId: string,
  dueAt: string | null,
): Promise<PortalWriteResult<{ taskId: string; dueAt: string | null }>> {
  if (!isUuid(taskId)) {
    return { ok: false, code: 'validation', message: 'Invalid task identifier.' }
  }
  if (dueAt !== null && Number.isNaN(new Date(dueAt).getTime())) {
    return {
      ok: false,
      code: 'validation',
      message: 'Due date must be a valid date or null.',
    }
  }
  try {
    const result = await updateTaskDue(taskId, dueAt)
    revalidatePortal()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

// ---------------------------------------------------------------
// STORY 2 — Manual interaction / note logging
// ---------------------------------------------------------------

export async function logManualInteractionAction(input: {
  personId: string
  channel: 'manual' | 'note'
  occurredAt?: string
  title?: string
  summary?: string
  propertyId?: string
  dealId?: string
}): Promise<PortalWriteResult<{ interactionId: string }>> {
  if (!isUuid(input.personId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid person identifier.',
    }
  }
  if (input.propertyId && !isUuid(input.propertyId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid property identifier.',
    }
  }
  if (input.dealId && !isUuid(input.dealId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid deal identifier.',
    }
  }
  if (input.channel !== 'manual' && input.channel !== 'note') {
    return {
      ok: false,
      code: 'validation',
      message: 'Channel must be manual or note.',
    }
  }
  const title = input.title?.trim()
  const summary = input.summary?.trim()
  if (!title && !summary) {
    return {
      ok: false,
      code: 'validation',
      message: 'A title or summary is required.',
    }
  }
  const occurredAt = input.occurredAt ?? new Date().toISOString()
  if (Number.isNaN(new Date(occurredAt).getTime())) {
    return {
      ok: false,
      code: 'validation',
      message: 'Occurred at must be a valid date.',
    }
  }
  try {
    const result = await logManualInteraction({
      personId: input.personId,
      channel: input.channel,
      occurredAt,
      title: title || undefined,
      summary: summary || undefined,
      propertyId: input.propertyId ?? undefined,
      dealId: input.dealId ?? undefined,
    })
    revalidatePortal()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

// ---------------------------------------------------------------
// STORY 3 — Showing create / schedule / cancel
// ---------------------------------------------------------------

export async function createShowingAction(input: {
  personId: string
  propertyId?: string
  dealId?: string
  requestedAt?: string
}): Promise<PortalWriteResult<{ id: string }>> {
  if (!isUuid(input.personId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid person identifier.',
    }
  }
  if (input.propertyId && !isUuid(input.propertyId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid property identifier.',
    }
  }
  if (input.dealId && !isUuid(input.dealId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid deal identifier.',
    }
  }
  if (
    input.requestedAt &&
    Number.isNaN(new Date(input.requestedAt).getTime())
  ) {
    return {
      ok: false,
      code: 'validation',
      message: 'Requested at must be a valid date.',
    }
  }
  try {
    const result = await createShowing({
      personId: input.personId,
      propertyId: input.propertyId ?? undefined,
      dealId: input.dealId ?? undefined,
      requestedAt: input.requestedAt ?? undefined,
    })
    revalidatePortal()
    return { ok: true, data: { id: result.id } }
  } catch (error) {
    return failure(error)
  }
}

export async function scheduleShowingAction(
  showingId: string,
  scheduledAt: string,
): Promise<PortalWriteResult<{ id: string }>> {
  if (!isUuid(showingId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid showing identifier.',
    }
  }
  if (Number.isNaN(new Date(scheduledAt).getTime())) {
    return {
      ok: false,
      code: 'validation',
      message: 'Scheduled at must be a valid date.',
    }
  }
  try {
    const result = await scheduleShowing(showingId, scheduledAt)
    revalidatePortal()
    return { ok: true, data: { id: result.id } }
  } catch (error) {
    return failure(error)
  }
}

export async function cancelShowingAction(
  showingId: string,
): Promise<PortalWriteResult<{ id: string }>> {
  if (!isUuid(showingId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid showing identifier.',
    }
  }
  try {
    const result = await cancelShowing(showingId)
    revalidatePortal()
    return { ok: true, data: { id: result.id } }
  } catch (error) {
    return failure(error)
  }
}

// ---------------------------------------------------------------
// STORY 4 — Showing complete (atomic dual-write)
// ---------------------------------------------------------------

export async function completeShowingAction(
  showingId: string,
  completedAt?: string,
): Promise<PortalWriteResult<{ id: string }>> {
  if (!isUuid(showingId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid showing identifier.',
    }
  }
  if (completedAt && Number.isNaN(new Date(completedAt).getTime())) {
    return {
      ok: false,
      code: 'validation',
      message: 'Completed at must be a valid date.',
    }
  }
  try {
    const result = await completeShowing(showingId, completedAt ?? undefined)
    revalidatePortal()
    return { ok: true, data: { id: result.id } }
  } catch (error) {
    return failure(error)
  }
}

// ---------------------------------------------------------------
// STORY 5 — Offer operations (no accept)
// ---------------------------------------------------------------

export async function submitOfferAction(input: {
  dealId: string
  personId: string
  amount: number
  parentOfferId?: string
}): Promise<PortalWriteResult<{ offerId: string }>> {
  if (!isUuid(input.dealId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid deal identifier.',
    }
  }
  if (!isUuid(input.personId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid person identifier.',
    }
  }
  if (input.parentOfferId && !isUuid(input.parentOfferId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid parent offer identifier.',
    }
  }
  try {
    const result = await submitOffer({
      dealId: input.dealId,
      personId: input.personId,
      amount: input.amount,
      parentOfferId: input.parentOfferId ?? undefined,
    })
    revalidatePortal()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

export async function withdrawOfferAction(
  offerId: string,
): Promise<PortalWriteResult<{ offerId: string; status: 'withdrawn' }>> {
  if (!isUuid(offerId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid offer identifier.',
    }
  }
  try {
    const result = await withdrawOffer(offerId)
    revalidatePortal()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

export async function rejectOfferAction(
  offerId: string,
): Promise<PortalWriteResult<{ offerId: string; status: 'rejected' }>> {
  if (!isUuid(offerId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid offer identifier.',
    }
  }
  try {
    const result = await rejectOffer(offerId)
    revalidatePortal()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

// ---------------------------------------------------------------
// STORY 6 — Needs Review safe resolution
// ---------------------------------------------------------------

export async function rejectIntakeAction(
  submissionId: string,
): Promise<PortalWriteResult<{ submissionId: string }>> {
  if (!isUuid(submissionId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid submission identifier.',
    }
  }
  try {
    const result = await rejectIntake(submissionId)
    revalidatePortal()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

export async function attachIntakeToPersonAction(
  submissionId: string,
  personId: string,
): Promise<
  PortalWriteResult<{ submissionId: string; interactionId: string; personId: string }>
> {
  if (!isUuid(submissionId) || !isUuid(personId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid submission or person identifier.',
    }
  }
  try {
    const result = await attachIntakeToPerson(submissionId, personId)
    revalidatePortal()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

// ---------------------------------------------------------------
// STORY 7 — Participant role='other' writes
// ---------------------------------------------------------------

export async function addOtherParticipantAction(input: {
  dealId: string
  personId?: string
  userId?: string
  roleLabel: string
}): Promise<PortalWriteResult<{ participantId: string }>> {
  if (!isUuid(input.dealId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid deal identifier.',
    }
  }
  if (input.personId && !isUuid(input.personId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid person identifier.',
    }
  }
  if (input.userId && !isUuid(input.userId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid user identifier.',
    }
  }
  try {
    const result = await addOtherParticipant({
      dealId: input.dealId,
      personId: input.personId ?? undefined,
      userId: input.userId ?? undefined,
      roleLabel: input.roleLabel,
    })
    revalidatePortal()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

export async function endParticipantAction(
  participantId: string,
): Promise<PortalWriteResult<{ participantId: string }>> {
  if (!isUuid(participantId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid participant identifier.',
    }
  }
  try {
    const result = await endParticipant(participantId)
    revalidatePortal()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

export async function updateParticipantRoleLabelAction(
  participantId: string,
  roleLabel: string,
): Promise<PortalWriteResult<{ participantId: string }>> {
  if (!isUuid(participantId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid participant identifier.',
    }
  }
  try {
    const result = await updateParticipantRoleLabel(participantId, roleLabel)
    revalidatePortal()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

// ---------------------------------------------------------------
// STORY 8 — Client note / relationship status
// ---------------------------------------------------------------

export async function updatePersonNotesAction(
  personId: string,
  notes: string | null,
): Promise<PortalWriteResult<{ personId: string }>> {
  if (!isUuid(personId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid person identifier.',
    }
  }
  try {
    const result = await updatePersonNotes(personId, notes)
    revalidatePortal()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

const VALID_RELATIONSHIP_STATUSES = ['new', 'warm', 'active', 'referral']

export async function updatePersonStatusAction(
  personId: string,
  status: string,
): Promise<PortalWriteResult<{ personId: string; status: string }>> {
  if (!isUuid(personId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid person identifier.',
    }
  }
  if (!VALID_RELATIONSHIP_STATUSES.includes(status)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Relationship status is invalid.',
    }
  }
  try {
    const result = await updatePersonStatus(personId, status)
    revalidatePortal()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}
