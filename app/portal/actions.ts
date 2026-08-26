'use server'

import { revalidatePath } from 'next/cache'

import { getPortalSessionAdapter } from '@/lib/auth/portal-session'
import { runAuthorized } from '@/lib/auth/require-authority'
import type { ActingUser, AuthorityCode } from '@/lib/auth/types'
import { PortalWriteError } from '@/lib/portal-write-error'
import { toPortalInstant } from '@/lib/portal-time'
import { refreshClientReadModels } from '@/db/client-read-models'
import { searchPeople } from '@/db/people'
import type { PersonSearchResult } from '@/db/people'
import {
  archiveClient,
  createClient,
  setClientIdentity,
  updateClientProfile,
} from '@/db/person-admin'
import type {
  ClientCreateInput,
  ClientIdentityKind,
  ClientProfileFields,
} from '@/lib/person-admin'
import { createTask } from '@/db/tasks'
import { applyFollowUpCommand, recordContactOutcome } from '@/db/follow-up'
import type { FollowUpCommandType, ContactOutcomeCode } from '@/db/follow-up'
import { NEXT_ACTION_PRESET_CODES, presetDefaultDue } from '@/lib/relationship-intel/next-action-presets'
import { dismissRecommendation } from '@/db/recommendations'
import type { RecommendationCode } from '@/lib/relationship-intel/recommendations'
import { emitDailyLoopTelemetry } from '@/db/telemetry'
import {
  cancelShowing,
  cancelTask,
  completeShowing,
  completeTask,
  createShowing,
  logManualInteraction,
  rejectOffer,
  scheduleShowing,
  submitOffer,
  updatePersonNotes,
  updatePersonStatus,
  updateTaskDue,
  withdrawOffer,
} from '@/db/portal-writes'
import {
  addOtherParticipant,
  endParticipant,
  updateParticipantRoleLabel,
} from '@/db/deal-participants'
import {
  createDeal,
  endStructuralParticipant,
  setStructuralParticipant,
} from '@/db/deal-admin-writes'
import type {
  DealCreateInput,
  StructuralParticipantInput,
} from '@/lib/deal-admin'
import { resolveIntake } from '@/db/needs-review-resolution'
import type {
  ResolveIntakeAction,
  ResolveIntakeInput,
  ResolveIntakeResult,
} from '@/db/needs-review-resolution'
import {
  setPropertyHero,
  setPropertyMediaOrder,
  setPropertyPublished,
  unlinkPropertyMedia,
  updateMediaMetadata,
  updatePropertyFacts,
  updatePropertyVisibility,
} from '@/db/portal-property'
import type { PropertyFactsInput } from '@/db/portal-property'
import {
  archiveProperty,
  createProperty,
  restoreProperty,
} from '@/db/property-admin-writes'
import type { PropertyCreateInput } from '@/lib/property-admin'

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

// AUTH-03 single enforcement seam for every authenticated Portal write.
// Resolves the acting user (session → app actor) and asserts the EXACT coarse
// authority BEFORE the business service runs. On denial the auth error
// propagates and the write never executes — client-side button hiding is
// cosmetic and never sufficient. Authorities are coarse ("may this actor
// attempt this class of command?"); business-state legality stays in the
// domain/workflow services. See docs/auth-command-map.md.
function portalWrite<T>(
  authority: AuthorityCode,
  handler: (actor: ActingUser) => Promise<T> | T,
): Promise<T> {
  return runAuthorized(getPortalSessionAdapter(), authority, handler)
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
  return portalWrite('crm.write', async () => {
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
  })
}

export async function completeTaskAction(
  taskId: string,
): Promise<PortalWriteResult<{ taskId: string; status: 'completed' }>> {
  if (!isUuid(taskId)) {
    return { ok: false, code: 'validation', message: 'Invalid task identifier.' }
  }
  return portalWrite('crm.write', async () => {
    try {
      const result = await completeTask(taskId)
      revalidatePortal()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
}

export async function cancelTaskAction(
  taskId: string,
): Promise<PortalWriteResult<{ taskId: string; status: 'cancelled' }>> {
  if (!isUuid(taskId)) {
    return { ok: false, code: 'validation', message: 'Invalid task identifier.' }
  }
  return portalWrite('crm.write', async () => {
    try {
      const result = await cancelTask(taskId)
      revalidatePortal()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
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
  return portalWrite('crm.write', async () => {
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
  })
}

// ---------------------------------------------------------------
// CORE-DAILY-01 — relationship follow-up lifecycle command.
// Every call carries a caller-supplied commandId (idempotency key). A
// replayed/duplicate commandId never re-applies a side effect (receipt unique).
// ---------------------------------------------------------------
export async function applyFollowUpCommandAction(input: {
  commandId: string
  commandType: FollowUpCommandType
  payload: {
    followUpId?: string | null
    personId?: string | null
    propertyId?: string | null
    dealId?: string | null
    title?: string | null
    detail?: string | null
    dueAt?: string | null
    snoozeUntil?: string | null
    outcome?: string | null
    nextTouchAt?: string | null
    nextTouchTitle?: string | null
    source?: string | null
    reason?: string | null
    recommendationKey?: string | null
  }
}): Promise<
  PortalWriteResult<{
    duplicate: boolean
    followUpId: string | null
    nextFollowUpId: string | null
  }>
> {
  const commandTypes: FollowUpCommandType[] = [
    'create', 'snooze', 'complete', 'dismiss', 'cancel',
  ]
  if (!isUuid(input.commandId)) {
    return { ok: false, code: 'validation', message: 'Invalid command identifier.' }
  }
  if (!commandTypes.includes(input.commandType)) {
    return { ok: false, code: 'validation', message: 'Unknown command type.' }
  }
  const payloadIds = [input.payload.followUpId, input.payload.personId, input.payload.propertyId, input.payload.dealId].filter(Boolean) as string[]
  if (payloadIds.some((id) => !isUuid(id))) {
    return { ok: false, code: 'validation', message: 'Follow-up context identifiers are invalid.' }
  }
  return portalWrite('crm.write', async (actor) => {
    try {
      const result = await applyFollowUpCommand({
        commandId: input.commandId,
        commandType: input.commandType,
        payload: {
          ...input.payload,
          dueAt: input.payload.dueAt != null ? toPortalInstant(input.payload.dueAt) : null,
          snoozeUntil: input.payload.snoozeUntil != null ? toPortalInstant(input.payload.snoozeUntil) : null,
          nextTouchAt: input.payload.nextTouchAt != null ? toPortalInstant(input.payload.nextTouchAt) : null,
        },
        actorUserId: actor.appUserId,
      })
      revalidatePortal()
      if (!result.duplicate && input.commandType === 'snooze') {
        void emitDailyLoopTelemetry({ eventType: 'followup_snoozed', entityKind: 'follow_up', entityId: result.followUp?.id ?? null })
      }
      if (!result.duplicate && input.commandType === 'complete') {
        void emitDailyLoopTelemetry({ eventType: 'followup_completed', entityKind: 'follow_up', entityId: result.followUp?.id ?? null })
        if (result.nextFollowUp?.id) void emitDailyLoopTelemetry({ eventType: 'next_touch_created', entityKind: 'follow_up', entityId: result.nextFollowUp.id })
      }
      return {
        ok: true,
        data: {
          duplicate: result.duplicate,
          followUpId: result.followUp?.id ?? null,
          nextFollowUpId: result.nextFollowUp?.id ?? null,
        },
      }
    } catch (error) {
      return failure(error)
    }
  })
}

// ---------------------------------------------------------------
// CORE-DAILY-03 — record contact outcome (canonical Interaction + follow-up).
// ---------------------------------------------------------------
export async function recordOutcomeAction(input: {
  commandId: string
  personId: string
  channel: string
  outcome: string
  occurredAt?: string | null
  title?: string | null
  summary?: string | null
  propertyId?: string | null
  dealId?: string | null
  followUpId?: string | null
  nextTouchAt?: string | null
  nextTouchTitle?: string | null
  source?: string | null
}): Promise<
  PortalWriteResult<{
    duplicate: boolean
    interactionId: string | null
    followUpId: string | null
    nextFollowUpId: string | null
  }>
> {
  const outcomeCodes: ContactOutcomeCode[] = [
    'connected', 'no_answer', 'left_message', 'sent_information', 'scheduled_follow_up',
    'showing_discussed', 'offer_discussed', 'waiting_on_client', 'waiting_on_third_party',
    'completed', 'custom',
  ]
  if (!isUuid(input.commandId) || !isUuid(input.personId)) {
    return { ok: false, code: 'validation', message: 'Invalid command or person identifier.' }
  }
  if (!outcomeCodes.includes(input.outcome as ContactOutcomeCode)) {
    return { ok: false, code: 'validation', message: 'Unknown outcome.' }
  }
  const ids = [input.propertyId, input.dealId, input.followUpId].filter(Boolean) as string[]
  if (ids.some((id) => !isUuid(id))) {
    return { ok: false, code: 'validation', message: 'Outcome context identifiers are invalid.' }
  }
  return portalWrite('crm.write', async (actor) => {
    try {
      const result = await recordContactOutcome({
        commandId: input.commandId,
        personId: input.personId,
        channel: input.channel,
        outcome: input.outcome as ContactOutcomeCode,
        occurredAt: input.occurredAt ?? undefined,
        title: input.title,
        summary: input.summary,
        propertyId: input.propertyId,
        dealId: input.dealId,
        followUpId: input.followUpId,
        nextTouchAt: input.nextTouchAt != null ? toPortalInstant(input.nextTouchAt) : null,
        nextTouchTitle: input.nextTouchTitle,
        source: input.source,
        actorUserId: actor.appUserId,
      })
      revalidatePortal()
      if (!result.duplicate) {
        void emitDailyLoopTelemetry({ eventType: 'outcome_recorded', entityKind: 'person', entityId: input.personId })
        if (result.followUpId) void emitDailyLoopTelemetry({ eventType: 'followup_completed', entityKind: 'follow_up', entityId: result.followUpId })
        if (result.nextFollowUpId) void emitDailyLoopTelemetry({ eventType: 'next_touch_created', entityKind: 'follow_up', entityId: result.nextFollowUpId })
      }
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
}

// ---------------------------------------------------------------
// CORE-DAILY-04 — quick next action from a preset (canonical follow-up create).
// ---------------------------------------------------------------
export async function createQuickNextActionAction(input: {
  commandId: string
  personId?: string | null
  propertyId?: string | null
  dealId?: string | null
  preset: string
  dueAt?: string | null
  title?: string | null
  detail?: string | null
  source?: string | null
}): Promise<
  PortalWriteResult<{ duplicate: boolean; followUpId: string | null }>
> {
  if (!isUuid(input.commandId)) {
    return { ok: false, code: 'validation', message: 'Invalid command identifier.' }
  }
  if (!NEXT_ACTION_PRESET_CODES.includes(input.preset as never)) {
    return { ok: false, code: 'validation', message: 'Unknown next-action preset.' }
  }
  const ids = [input.personId, input.propertyId, input.dealId].filter(Boolean) as string[]
  if (ids.some((id) => !isUuid(id))) {
    return { ok: false, code: 'validation', message: 'Next-action context identifiers are invalid.' }
  }
  if (!input.personId && !input.propertyId && !input.dealId) {
    return { ok: false, code: 'validation', message: 'Next action requires person, property, or deal context.' }
  }
  return portalWrite('crm.write', async () => {
    try {
      const presetTitle = input.title?.trim() || presetTitleFor(input.preset)
      const dueAt = input.dueAt ?? presetDefaultDue(input.preset as never)
      const result = await applyFollowUpCommand({
        commandId: input.commandId,
        commandType: 'create',
        payload: {
          personId: input.personId,
          propertyId: input.propertyId,
          dealId: input.dealId,
          title: presetTitle,
          detail: input.detail ?? null,
          dueAt,
          source: input.source ?? 'quick_next_action',
        },
      })
      revalidatePortal()
      return { ok: true, data: { duplicate: result.duplicate, followUpId: result.followUp?.id ?? null } }
    } catch (error) {
      return failure(error)
    }
  })
}

function presetTitleFor(code: string): string {
  switch (code) {
    case 'call_back': return 'Call back'
    case 'send_information': return 'Send information'
    case 'schedule_showing': return 'Schedule showing'
    case 'prepare_offer': return 'Prepare offer'
    case 'check_financing': return 'Check financing'
    case 'follow_up_lawyer': return 'Follow up with lawyer'
    case 'check_appraisal': return 'Check appraisal'
    case 'check_inspection': return 'Check inspection'
    case 'check_closing_readiness': return 'Check closing readiness'
    default: return 'Custom reminder'
  }
}

// ---------------------------------------------------------------
// CORE-DAILY-08 — dismiss a recommendation (idempotent suppression).
// ---------------------------------------------------------------
export async function dismissRecommendationAction(input: {
  personId: string
  code: string
}): Promise<PortalWriteResult<{ dismissed: boolean }>> {
  const codes: RecommendationCode[] = [
    'overdue_relationship_commitment', 'due_soon_relationship_commitment',
    'unanswered_inbound', 'two_way_without_next_step', 'quiet_past_client',
  ]
  if (!isUuid(input.personId)) {
    return { ok: false, code: 'validation', message: 'Invalid person identifier.' }
  }
  if (!codes.includes(input.code as RecommendationCode)) {
    return { ok: false, code: 'validation', message: 'Unknown recommendation.' }
  }
  return portalWrite('crm.write', async (actor) => {
    try {
      await dismissRecommendation(input.personId, input.code as RecommendationCode, actor.appUserId)
      revalidatePortal()
      return { ok: true, data: { dismissed: true } }
    } catch (error) {
      return failure(error)
    }
  })
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
  return portalWrite('crm.write', async () => {
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
      await refreshClientReadModels()
      revalidatePortal()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
}

// ---------------------------------------------------------------
// STORY 3 — Showing create / schedule / cancel
// ---------------------------------------------------------------

export async function createShowingAction(input: {
  personId: string
  propertyId?: string
  dealId?: string
  requestedAt?: string
  requestSourceInteractionId?: string
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
    input.requestSourceInteractionId &&
    !isUuid(input.requestSourceInteractionId)
  ) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid source interaction identifier.',
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
  return portalWrite('deal.write', async () => {
    try {
      const result = await createShowing({
        personId: input.personId,
        propertyId: input.propertyId ?? undefined,
        dealId: input.dealId ?? undefined,
        requestedAt:
          input.requestedAt !== undefined
            ? toPortalInstant(input.requestedAt)
            : undefined,
        requestSourceInteractionId:
          input.requestSourceInteractionId ?? undefined,
      })
      revalidatePortal()
      return { ok: true, data: { id: result.id } }
    } catch (error) {
      return failure(error)
    }
  })
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
  return portalWrite('deal.write', async () => {
    try {
      const result = await scheduleShowing(showingId, toPortalInstant(scheduledAt))
      revalidatePortal()
      return { ok: true, data: { id: result.id } }
    } catch (error) {
      return failure(error)
    }
  })
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
  return portalWrite('deal.write', async () => {
    try {
      const result = await cancelShowing(showingId)
      revalidatePortal()
      return { ok: true, data: { id: result.id } }
    } catch (error) {
      return failure(error)
    }
  })
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
  return portalWrite('deal.write', async () => {
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
  })
}

// ---------------------------------------------------------------
// STORY 5 — Offer operations (submit / withdraw / reject)
//
// Recorded asymmetry (CRM-11): there is intentionally NO portal accept action.
// Offer accept is the canonical application command offer.accept, reachable
// only through the command seam (workflow command-router -> canonical
// dispatcher -> db/offer-acceptance.ts) with claim-first receipts, idempotency
// and the one-accepted/primary-offer-per-deal invariant. reject/withdraw stay
// portal actions here. Accepting an offer NEVER auto-advances the deal stage —
// deal stage changes only via explicit deal.set_stage commands.
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
  return portalWrite('deal.write', async () => {
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
  })
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
  return portalWrite('deal.write', async () => {
    try {
      const result = await withdrawOffer(offerId)
      revalidatePortal()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
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
  return portalWrite('deal.write', async () => {
    try {
      const result = await rejectOffer(offerId)
      revalidatePortal()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
}

// ---------------------------------------------------------------
// STORY 6 — Needs Review resolution (single seam)
//
// AUTH-05 actor capture: the acting app_user is derived from the session by
// portalWrite and threaded into resolveIntake, so resolved_by_user_id on the
// durable receipt records WHO resolved the item. The action intentionally does
// NOT accept a client-supplied actor id — audit attribution must come from the
// authenticated session, never from the caller.
// ---------------------------------------------------------------

export async function resolveIntakeAction(input: {
  submissionId: string
  action: ResolveIntakeAction
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
  return portalWrite('crm.write', async (actor) => {
    try {
      const result = await resolveIntake({
        submissionId: input.submissionId,
        action: input.action,
        actorAppUserId: actor.appUserId,
      } satisfies ResolveIntakeInput)
      revalidatePortal()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
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
  return portalWrite('deal.write', async () => {
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
  })
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
  return portalWrite('deal.write', async () => {
    try {
      const result = await endParticipant(participantId)
      revalidatePortal()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
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
  return portalWrite('deal.write', async () => {
    try {
      const result = await updateParticipantRoleLabel(participantId, roleLabel)
      revalidatePortal()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
}

// ---------------------------------------------------------------
// OPS-05 — Deal / Participant Administration (create + structural
// participant lifecycle). deal_participant is THE canonical participant model
// (migration 034); these writes operate on it and keep the per-deal legacy FK
// mirrors (deal.client_person_id / deal.owner_user_id) consistent in the same
// transaction. Validation is the shared pure contract in lib/deal-admin.ts;
// the action layer re-checks identifiers for a friendly result before any
// authority-bound write runs.
// ---------------------------------------------------------------

export async function createDealAction(
  input: DealCreateInput,
): Promise<PortalWriteResult<{ id: string }>> {
  return portalWrite('deal.write', async () => {
    try {
      const result = await createDeal(input)
      revalidatePortal()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
}

export async function setStructuralParticipantAction(
  input: StructuralParticipantInput,
): Promise<PortalWriteResult<{ participantId: string }>> {
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
  return portalWrite('deal.write', async () => {
    try {
      const result = await setStructuralParticipant(input)
      revalidatePortal()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
}

export async function endStructuralParticipantAction(
  participantId: string,
): Promise<PortalWriteResult<{ participantId: string }>> {
  if (!isUuid(participantId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid participant identifier.',
    }
  }
  return portalWrite('deal.write', async () => {
    try {
      const result = await endStructuralParticipant(participantId)
      revalidatePortal()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
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
  return portalWrite('crm.write', async () => {
    try {
      const result = await updatePersonNotes(personId, notes)
      revalidatePortal()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
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
  return portalWrite('crm.write', async () => {
    try {
      const result = await updatePersonStatus(personId, status)
      revalidatePortal()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
}

// ---------------------------------------------------------------
// OPS-02 — Client Administration CRUD (create / update / archive /
// contact identity). Validation is the shared pure contract in
// lib/person-admin.ts (canonical email / E.164 phone, closed role/status
// vocabularies); identity ownership conflicts surface as 'conflict'.
// ---------------------------------------------------------------

export async function createClientAction(
  input: ClientCreateInput,
): Promise<PortalWriteResult<{ personId: string }>> {
  return portalWrite('crm.write', async () => {
    try {
      const result = await createClient(input)
      await refreshClientReadModels()
      revalidatePortal()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
}

export async function updateClientProfileAction(
  personId: string,
  input: ClientProfileFields,
): Promise<PortalWriteResult<{ personId: string }>> {
  if (!isUuid(personId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid person identifier.',
    }
  }
  return portalWrite('crm.write', async () => {
    try {
      const result = await updateClientProfile(personId, input)
      await refreshClientReadModels()
      revalidatePortal()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
}

export async function archiveClientAction(
  personId: string,
): Promise<PortalWriteResult<{ personId: string }>> {
  if (!isUuid(personId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid person identifier.',
    }
  }
  return portalWrite('crm.write', async () => {
    try {
      const result = await archiveClient(personId)
      await refreshClientReadModels()
      revalidatePortal()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
}

export async function updateClientIdentityAction(
  personId: string,
  kind: ClientIdentityKind,
  value: string | null,
): Promise<PortalWriteResult<{ personId: string; kind: ClientIdentityKind }>> {
  if (!isUuid(personId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid person identifier.',
    }
  }
  if (kind !== 'email' && kind !== 'phone') {
    return {
      ok: false,
      code: 'validation',
      message: 'Contact kind must be email or phone.',
    }
  }
  return portalWrite('crm.write', async () => {
    try {
      const result = await setClientIdentity(personId, kind, value)
      await refreshClientReadModels()
      revalidatePortal()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
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
  return portalWrite('listing.write', async () => {
    try {
      const result = await updatePropertyFacts(propertyId, input)
      revalidatePortal()
      revalidatePropertyPublic()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
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
  return portalWrite('listing.write', async () => {
    try {
      const result = await updatePropertyVisibility(propertyId, input)
      revalidatePortal()
      revalidatePropertyPublic()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
}

export async function setPropertyPublishedAction(
  propertyId: string,
  isPublished: boolean,
): Promise<PortalWriteResult<{ id: string; slug: string | null }>> {
  if (!isUuid(propertyId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid property identifier.',
    }
  }
  if (typeof isPublished !== 'boolean') {
    return {
      ok: false,
      code: 'validation',
      message: 'isPublished must be a boolean.',
    }
  }
  return portalWrite('listing.write', async () => {
    try {
      const result = await setPropertyPublished(propertyId, isPublished)
      revalidatePortal()
      revalidatePropertyPublic()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
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
  return portalWrite('listing.write', async () => {
    try {
      const result = await setPropertyMediaOrder(propertyId, orderedMediaIds)
      revalidatePortal()
      revalidatePropertyPublic()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
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
  return portalWrite('listing.write', async () => {
    try {
      const result = await setPropertyHero(propertyId, mediaId)
      revalidatePortal()
      revalidatePropertyPublic()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
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
  return portalWrite('listing.write', async () => {
    try {
      const result = await unlinkPropertyMedia(propertyId, mediaId)
      revalidatePortal()
      revalidatePropertyPublic()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
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
  return portalWrite('listing.write', async () => {
    try {
      const result = await updateMediaMetadata(mediaId, input)
      revalidatePortal()
      revalidatePropertyPublic()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
}

// ---------------------------------------------------------------
// OPS-03 — Property Administration lifecycle (create / archive /
// restore). Completes the property CRUD surface: the admin projection,
// facts editor, visibility, and media writers already exist; these add
// record creation and soft archive. Validation is the shared pure
// contract in lib/property-admin.ts; archive/restore only flip
// archived_at, which every existing read projection already filters.
// ---------------------------------------------------------------

export async function createPropertyAction(
  input: PropertyCreateInput,
): Promise<PortalWriteResult<{ id: string; slug: string | null }>> {
  return portalWrite('listing.write', async () => {
    try {
      const result = await createProperty(input)
      revalidatePortal()
      revalidatePropertyPublic()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
}

export async function archivePropertyAction(
  propertyId: string,
): Promise<PortalWriteResult<{ id: string }>> {
  if (!isUuid(propertyId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid property identifier.',
    }
  }
  return portalWrite('listing.write', async () => {
    try {
      const result = await archiveProperty(propertyId)
      revalidatePortal()
      revalidatePropertyPublic()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
}

export async function restorePropertyAction(
  propertyId: string,
): Promise<PortalWriteResult<{ id: string }>> {
  if (!isUuid(propertyId)) {
    return {
      ok: false,
      code: 'validation',
      message: 'Invalid property identifier.',
    }
  }
  return portalWrite('listing.write', async () => {
    try {
      const result = await restoreProperty(propertyId)
      revalidatePortal()
      revalidatePropertyPublic()
      return { ok: true, data: result }
    } catch (error) {
      return failure(error)
    }
  })
}
