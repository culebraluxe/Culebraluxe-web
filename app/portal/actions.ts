'use server'

import { revalidatePath } from 'next/cache'

import { PortalWriteError } from '@/lib/portal-write-error'
import { toPortalInstant } from '@/lib/portal-time'
import { searchPeople } from '@/db/people'
import type { PersonSearchResult } from '@/db/people'
import { createTask } from '@/db/tasks'
import {
  addOtherParticipant,
  cancelShowing,
  cancelTask,
  completeShowing,
  completeTask,
  createShowing,
  endParticipant,
  logManualInteraction,
  rejectOffer,
  scheduleShowing,
  submitOffer,
  updateParticipantRoleLabel,
  updatePersonNotes,
  updatePersonStatus,
  updateTaskDue,
  withdrawOffer,
} from '@/db/portal-writes'
import { resolveIntake } from '@/db/needs-review-resolution'
import type {
  ResolveIntakeAction,
  ResolveIntakeInput,
  ResolveIntakeResult,
} from '@/db/needs-review-resolution'
import {
  setPropertyHero,
  setPropertyMediaOrder,
  unlinkPropertyMedia,
  updateMediaMetadata,
  updatePropertyFacts,
  updatePropertyVisibility,
} from '@/db/portal-property'
import type { PropertyFactsInput } from '@/db/portal-property'

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
  if (error instanceof PortalWriteError) {
    return { ok: false, code: error.code, message: error.message }
  }
  // Never surface raw database/internal messages to the operator.
  console.error(
    'Portal write failed.',
    error instanceof Error ? error.message : error,
  )
  return {
    ok: false,
    code: 'unknown',
    message: 'Something went wrong. Please try again.',
  }
}

export async function searchPeopleAction(
  query: string,
): Promise<PersonSearchResult[]> {
  if (typeof query !== 'string' || query.trim().length < 2) return []
  return searchPeople(query.trim())
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
  if (
    input.dueAt != null &&
    Number.isNaN(new Date(input.dueAt).getTime())
  ) {
    return {
      ok: false,
      code: 'validation',
      message: 'Due date must be a valid date.',
    }
  }
  try {
    const task = await createTask({
      title,
      detail: input.detail?.trim() || undefined,
      personId: input.personId ?? undefined,
      propertyId: input.propertyId ?? undefined,
      dealId: input.dealId ?? undefined,
      dueAt: input.dueAt != null ? toPortalInstant(input.dueAt) : undefined,
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
    const result = await updateTaskDue(
      taskId,
      dueAt !== null ? toPortalInstant(dueAt) : null,
    )
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
  if (
    input.occurredAt !== undefined &&
    Number.isNaN(new Date(input.occurredAt).getTime())
  ) {
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
      occurredAt:
        input.occurredAt !== undefined
          ? toPortalInstant(input.occurredAt)
          : new Date().toISOString(),
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
      requestedAt:
        input.requestedAt !== undefined
          ? toPortalInstant(input.requestedAt)
          : undefined,
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
    const result = await scheduleShowing(showingId, toPortalInstant(scheduledAt))
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
    const result = await completeShowing(
      showingId,
      completedAt !== undefined ? toPortalInstant(completedAt) : undefined,
    )
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
// STORY 6 — Needs Review resolution (single seam)
// ---------------------------------------------------------------

export async function resolveIntakeAction(input: {
  submissionId: string
  action: ResolveIntakeAction
  actorAppUserId?: string | null
}): Promise<PortalWriteResult<ResolveIntakeResult>> {
  if (!isUuid(input.submissionId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid submission identifier.',
    }
  }
  if (input.action.kind === 'attach' && !isUuid(input.action.personId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid person identifier.',
    }
  }
  if (input.actorAppUserId != null && !isUuid(input.actorAppUserId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid acting user identifier.',
    }
  }
  try {
    const result = await resolveIntake({
      submissionId: input.submissionId,
      action: input.action,
      actorAppUserId: input.actorAppUserId ?? undefined,
    } satisfies ResolveIntakeInput)
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

// ---------------------------------------------------------------
// LISTING OPERATIONS — Property Administration edits (LOPS V1)
// ---------------------------------------------------------------

// Revalidate the public surfaces that read canonical property data so admin
// edits propagate immediately to the homepage, buyers inventory, and listing
// detail pages.
function revalidatePropertyPublic() {
  revalidatePath('/', 'layout')
  revalidatePath('/buyers', 'page')
  revalidatePath('/properties', 'layout')
}

export async function updatePropertyFactsAction(
  propertyId: string,
  input: PropertyFactsInput,
): Promise<PortalWriteResult<{ id: string; slug: string | null }>> {
  if (!isUuid(propertyId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid property identifier.',
    }
  }
  try {
    const result = await updatePropertyFacts(propertyId, input)
    revalidatePortal()
    revalidatePropertyPublic()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

export async function updatePropertyVisibilityAction(
  propertyId: string,
  input: { featured: boolean; status: string },
): Promise<PortalWriteResult<{ id: string; slug: string | null }>> {
  if (!isUuid(propertyId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid property identifier.',
    }
  }
  try {
    const result = await updatePropertyVisibility(propertyId, input)
    revalidatePortal()
    revalidatePropertyPublic()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

export async function setPropertyMediaOrderAction(
  propertyId: string,
  orderedMediaIds: string[],
): Promise<PortalWriteResult<{ id: string }>> {
  if (!isUuid(propertyId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid property identifier.',
    }
  }
  try {
    const result = await setPropertyMediaOrder(propertyId, orderedMediaIds)
    revalidatePortal()
    revalidatePropertyPublic()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

export async function setPropertyHeroAction(
  propertyId: string,
  mediaId: string,
): Promise<PortalWriteResult<{ id: string; mediaId: string }>> {
  if (!isUuid(propertyId) || !isUuid(mediaId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid property or media identifier.',
    }
  }
  try {
    const result = await setPropertyHero(propertyId, mediaId)
    revalidatePortal()
    revalidatePropertyPublic()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

export async function unlinkPropertyMediaAction(
  propertyId: string,
  mediaId: string,
): Promise<PortalWriteResult<{ id: string; mediaId: string }>> {
  if (!isUuid(propertyId) || !isUuid(mediaId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid property or media identifier.',
    }
  }
  try {
    const result = await unlinkPropertyMedia(propertyId, mediaId)
    revalidatePortal()
    revalidatePropertyPublic()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}

export async function updateMediaMetadataAction(
  mediaId: string,
  input: { altText: string | null; caption: string | null },
): Promise<PortalWriteResult<{ id: string }>> {
  if (!isUuid(mediaId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid media identifier.',
    }
  }
  try {
    const result = await updateMediaMetadata(mediaId, input)
    revalidatePortal()
    revalidatePropertyPublic()
    return { ok: true, data: result }
  } catch (error) {
    return failure(error)
  }
}
