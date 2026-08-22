// ---------------------------------------------------------------------------
// CRM-23 — Integration Inbox processor (the durable intake path).
//
// INBOX RESPONSIBILITY (architect brief): durable receipt of inbound external
// facts; stable external-source id; dedupe; replay; retry; status; bounded
// failure handling; poison/dead-letter or HumanRequired escalation;
// correlation; source provenance.
//
//   MacIntegrationObserver.acquire()
//     -> ExternalActivityEvent[] (neutral facts)
//     -> processExternalActivityEvent (this module), per event:
//         1. capability guard  (unsupported/unproven sources are SKIPPED
//            honestly — defense in depth below the observer filter)
//         2. insert-or-read receipt on UNIQUE (source, sourceAccount,
//            externalEventId) — the replay dedupe key
//         3. claim (received -> processing; stale re-claim)
//         4. mapper -> existing channel intake stub (identity/contact
//            resolution happens INSIDE the stub, before any mutation)
//         5. persist the canonical interaction ONLY when ready (or, for
//            contacts, converge onto the canonical person — no interaction)
//         6. transition the receipt to completed / rejected /
//            resolution_required / duplicate
//         7. bounded retry: a thrown failure re-queues (received, attempt+1)
//            up to maxAttempts, then POISONS the receipt (dead-letter /
//            HumanRequired escalation) — other events are never blocked.
//
// The ONLY canonical CRM writes are the interaction row (via the command
// seam, see lib/integration-inbox/wiring.ts) and — for contacts — identity
// resolution (read-only in V1). No task, deal, workflow or alert is ever
// created by this processor.
// ---------------------------------------------------------------------------

import { normalizeInboundEvent } from '../crm-intake-normalization'
import { prepareCalendarIntake } from '../crm-calendar-intake'
import type { CalendarAdapterConfiguration } from '../crm-calendar-types'
import { prepareEmailIntake } from '../crm-email-intake'
import type { EmailAdapterConfiguration } from '../crm-email-types'
import { prepareCommunicationsIntake } from '../crm-communications-intake'
import type { CommunicationsAdapterConfiguration } from '../crm-communications-types'
import { prepareWhatsAppIntake } from '../crm-whatsapp-intake'
import type { WhatsAppAdapterConfiguration } from '../crm-whatsapp-types'
import { resolveOrCreateInboundPerson } from '../crm-person-creation'
import type { IntakeRepositories } from '../crm-intake-types'
import type { PersonCreationRepositories } from '../crm-person-types'
import type { ExternalActivityEvent } from '../mac-observer/contracts'
import type {
  InsertIntegrationInboxInput,
  IntegrationInboxConfiguration,
  IntegrationInboxDurability,
  IntegrationInboxProcessingOutcome,
  IntegrationInboxRecord,
  IntegrationInboxTerminalStatus,
} from './contracts'
import {
  mapCalendarEvent,
  mapContactsEvent,
  mapMailEvent,
  mapMessagesEvent,
  mapWhatsAppEvent,
} from './mapper'
// INTAKE-01 — the realtime lane emits the canonical intake message and the
// durable inbox insert is the single projection both lanes share (batch and
// realtime differ only at the edge).
import { toInboxInsert } from '../intake/inbox'
import { lowerExternalActivityEventToIntakeMessage } from '../intake/realtime'

export type MacIntakeRepositories = IntakeRepositories & PersonCreationRepositories

/** Per-channel adapter configurations the existing intake stubs require. */
export type MacChannelConfigurations = {
  calendar: CalendarAdapterConfiguration
  mail: EmailAdapterConfiguration
  messages: CommunicationsAdapterConfiguration
  whatsapp: WhatsAppAdapterConfiguration
}

export const DEFAULT_INTEGRATION_INBOX_CONFIGURATION: IntegrationInboxConfiguration = {
  maxAttempts: 3,
  capabilities: {},
}

export type ProcessExternalActivityInput = {
  event: ExternalActivityEvent
  configuration: IntegrationInboxConfiguration
  repositories: MacIntakeRepositories
  durability: IntegrationInboxDurability
  channels: MacChannelConfigurations
  now?: () => string
}

function insertInputFromEvent(
  event: ExternalActivityEvent,
  maxAttempts: number,
): InsertIntegrationInboxInput {
  // INTAKE-01 — lower the neutral realtime fact into the canonical intake
  // message, then project through the SAME inbox bridge the batch lane uses.
  // The durable receipt therefore always derives from the canonical envelope:
  // one normalized intake contract, one transformation stack.
  return toInboxInsert(
    lowerExternalActivityEventToIntakeMessage(event),
    maxAttempts,
  )
}

function isTerminal(status: IntegrationInboxRecord['status']): boolean {
  return (
    status === 'completed' ||
    status === 'rejected' ||
    status === 'resolution_required' ||
    status === 'duplicate' ||
    status === 'poisoned'
  )
}

function replayOutcome(
  record: IntegrationInboxRecord,
): IntegrationInboxProcessingOutcome {
  switch (record.status) {
    case 'completed':
      return {
        outcome: 'completed',
        record,
        interactionId: record.interactionId ?? undefined,
        resolvedPersonId: record.resolvedPersonId ?? undefined,
        created: false,
      }
    case 'duplicate':
      return { outcome: 'duplicate', record }
    case 'rejected':
      return { outcome: 'rejected', record, reason: 'previously_rejected' }
    case 'resolution_required':
      return {
        outcome: 'resolution_required',
        record,
        reason: 'previously_requiring_resolution',
      }
    case 'poisoned':
      return {
        outcome: 'poisoned',
        record,
        error: record.lastError ?? 'previously_poisoned',
        attempts: record.attemptCount,
        escalated: true,
      }
    default:
      return { outcome: 'in_flight', record }
  }
}

/**
 * Process ONE external activity event through the durable inbox. Idempotent:
 * a replayed event with a terminal receipt returns the recorded outcome and
 * writes nothing. Thrown failures are bounded (retry -> received, then ->
 * poisoned) and never block other events.
 */
export async function processExternalActivityEvent(
  input: ProcessExternalActivityInput,
): Promise<IntegrationInboxProcessingOutcome> {
  const { event, configuration, durability } = input

  // 1. Honest capability gate (defense in depth below the observer filter).
  const capability = configuration.capabilities[event.source]
  if (!capability || capability.status !== 'available') {
    return {
      outcome: 'skipped_unsupported',
      reason: capability
        ? `source '${event.source}' capability is '${capability.status}'`
        : `source '${event.source}' has no capability declaration`,
    }
  }

  // 2. Insert-or-read receipt (the replay dedupe key).
  const inserted = await durability.insertOrReadReceipt(
    insertInputFromEvent(event, configuration.maxAttempts),
  )
  let record = inserted.record

  // Replay: a replayed event with a terminal status returns the recorded
  // outcome and writes nothing.
  if (!inserted.created && isTerminal(record.status)) {
    return replayOutcome(record)
  }

  // 3. Claim (received -> processing; stale re-claim; fresh in-flight = busy).
  const claimed = await durability.claimReceipt(record.id)
  if (!claimed) return { outcome: 'in_flight', record }
  record = claimed

  // 4-5. Map + intake through the existing channel stubs (identity/contact
  // resolution happens inside them, before any mutation).
  try {
    switch (event.source) {
      case 'contacts':
        return await processContacts(input, record)
      case 'calendar':
        return await processChannelEvent(
          input,
          record,
          () =>
            prepareCalendarIntake(
              mapCalendarEvent(event),
              input.channels.calendar,
              input.repositories,
            ),
        )
      case 'mail':
        return await processChannelEvent(
          input,
          record,
          () =>
            prepareEmailIntake(
              mapMailEvent(event),
              input.channels.mail,
              input.repositories,
            ),
        )
      case 'messages':
        return await processChannelEvent(
          input,
          record,
          () =>
            prepareCommunicationsIntake(
              mapMessagesEvent(event),
              input.channels.messages,
              input.repositories,
            ),
        )
      case 'whatsapp':
        return await processChannelEvent(
          input,
          record,
          () =>
            prepareWhatsAppIntake(
              mapWhatsAppEvent(event),
              input.channels.whatsapp,
              input.repositories,
            ),
        )
      default:
        throw new Error(`No intake path for source '${event.source}'.`)
    }
  } catch (error) {
    return failBounded(input, record, error)
  }
}

// ---------------------------------------------------------------------------
// Contacts: identity/contact convergence (the spine), no interaction in V1.
// ---------------------------------------------------------------------------

async function processContacts(
  input: ProcessExternalActivityInput,
  record: IntegrationInboxRecord,
): Promise<IntegrationInboxProcessingOutcome> {
  const { event, repositories } = input
  const normalizedEvent = normalizeInboundEvent(mapContactsEvent(event))

  // Identity/contact resolution BEFORE any mutation; an observed address-book
  // record never authorizes person auto-creation (CRM-03, least privilege).
  const personResult = await resolveOrCreateInboundPerson(
    normalizedEvent,
    { allowCreation: false, role: 'buyer' },
    repositories,
  )

  switch (personResult.status) {
    case 'resolved_existing': {
      // Converged onto the canonical person: no interaction row for a pure
      // address-book fact (identity convergence writes are a future, reviewed
      // capability). The receipt records the resolved person.
      const transitioned = await transition(input, record, 'completed', {
        resolvedPersonId: personResult.personId,
      })
      if (!transitioned) return { outcome: 'in_flight', record }
      return {
        outcome: 'completed',
        record: transitioned,
        resolvedPersonId: personResult.personId,
        created: false,
      }
    }
    case 'duplicate':
      return finishDuplicate(input, record, personResult.existingInteractionId)
    case 'resolution_required':
      return finishResolutionRequired(input, record, personResult.reason ?? 'identity_unresolved')
    case 'conflicting':
    case 'rejected':
      return finishRejected(input, record, personResult.reason ?? personResult.status)
    case 'created':
      // Unreachable: allowCreation is false. Fail closed instead of silently
      // accepting a fabricated person.
      return finishRejected(input, record, 'unexpected_person_creation')
  }
}

// ---------------------------------------------------------------------------
// Channel events: calendar / mail / messages / whatsapp via the existing
// intake coordinators (which already own normalization, assurance, exclusion
// and person resolution).
// ---------------------------------------------------------------------------

type ChannelIntakeResult =
  | Awaited<ReturnType<typeof prepareCalendarIntake>>
  | Awaited<ReturnType<typeof prepareEmailIntake>>
  | Awaited<ReturnType<typeof prepareCommunicationsIntake>>
  | Awaited<ReturnType<typeof prepareWhatsAppIntake>>

async function processChannelEvent(
  input: ProcessExternalActivityInput,
  record: IntegrationInboxRecord,
  prepare: () => Promise<ChannelIntakeResult>,
): Promise<IntegrationInboxProcessingOutcome> {
  const result = await prepare()

  switch (result.status) {
    case 'ready': {
      const interactionInput = result.intakeResult.interactionInput
      if (!interactionInput) {
        // Unreachable for a 'ready' intake result; fail closed without a write.
        return finishRejected(input, record, 'ready_without_interaction_input')
      }
      const persisted = await input.durability.persistInteraction(interactionInput)
      const transitioned = await transition(input, record, 'completed', {
        interactionId: persisted.interactionId,
        resolvedPersonId: result.personResult?.personId,
      })
      if (!transitioned) return { outcome: 'in_flight', record }
      return {
        outcome: 'completed',
        record: transitioned,
        interactionId: persisted.interactionId,
        resolvedPersonId: result.personResult?.personId,
        created: persisted.created,
      }
    }
    case 'duplicate':
      return finishDuplicate(input, record, result.existingInteractionId)
    case 'resolution_required':
      return finishResolutionRequired(input, record, result.reason ?? 'identity_unresolved')
    case 'excluded':
    case 'rejected':
      return finishRejected(input, record, result.reason)
  }
}

// ---------------------------------------------------------------------------
// Terminal transitions + bounded failure handling
// ---------------------------------------------------------------------------

async function transition(
  input: ProcessExternalActivityInput,
  record: IntegrationInboxRecord,
  to: IntegrationInboxTerminalStatus,
  extra: { interactionId?: string; resolvedPersonId?: string } = {},
): Promise<IntegrationInboxRecord | null> {
  const { durability } = input
  const ok = await durability.transitionReceipt({
    receiptId: record.id,
    claimToken: record.processingStartedAt as string,
    from: 'processing',
    to,
    interactionId: extra.interactionId,
    resolvedPersonId: extra.resolvedPersonId,
  })
  if (!ok) return null
  return {
    ...record,
    status: to,
    interactionId: extra.interactionId ?? null,
    resolvedPersonId: extra.resolvedPersonId ?? null,
    processingStartedAt: null,
    processingCompletedAt: (input.now ?? (() => new Date().toISOString()))(),
  }
}

async function finishDuplicate(
  input: ProcessExternalActivityInput,
  record: IntegrationInboxRecord,
  existingInteractionId?: string,
): Promise<IntegrationInboxProcessingOutcome> {
  const transitioned = await transition(input, record, 'duplicate')
  if (!transitioned) return { outcome: 'in_flight', record }
  return {
    outcome: 'duplicate',
    record: transitioned,
    existingInteractionId,
  }
}

async function finishRejected(
  input: ProcessExternalActivityInput,
  record: IntegrationInboxRecord,
  reason: string,
): Promise<IntegrationInboxProcessingOutcome> {
  const transitioned = await transition(input, record, 'rejected')
  if (!transitioned) return { outcome: 'in_flight', record }
  return { outcome: 'rejected', record: transitioned, reason }
}

async function finishResolutionRequired(
  input: ProcessExternalActivityInput,
  record: IntegrationInboxRecord,
  reason: string,
): Promise<IntegrationInboxProcessingOutcome> {
  const transitioned = await transition(input,
    record,
    'resolution_required',
  )
  if (!transitioned) return { outcome: 'in_flight', record }
  return {
    outcome: 'resolution_required',
    record: transitioned,
    reason,
  }
}

/**
 * Bounded failure handling: re-queue (received, attempt+1) up to maxAttempts,
 * then POISON the receipt (dead-letter / HumanRequired escalation). The
 * failure is isolated to this receipt — other events keep processing.
 */
async function failBounded(
  input: ProcessExternalActivityInput,
  record: IntegrationInboxRecord,
  error: unknown,
): Promise<IntegrationInboxProcessingOutcome> {
  const message = error instanceof Error ? error.message : String(error)
  const attempts = record.attemptCount + 1
  const failed = await input.durability.failReceipt({
    receiptId: record.id,
    claimToken: record.processingStartedAt as string,
    error: message,
    attempts,
    maxAttempts: input.configuration.maxAttempts,
  })
  if (!failed) return { outcome: 'in_flight', record }
  if (failed.status === 'poisoned') {
    return {
      outcome: 'poisoned',
      record: failed,
      error: message,
      attempts: failed.attemptCount,
      escalated: true,
    }
  }
  return {
    outcome: 'failed_retryable',
    record: failed,
    error: message,
    attempts: failed.attemptCount,
  }
}

// ---------------------------------------------------------------------------
// Sync orchestration — acquire + lower + process (the observer entry point)
// ---------------------------------------------------------------------------

export type SyncMacObservationsInput = Omit<
  ProcessExternalActivityInput,
  'event'
> & {
  /** Acquire + lower observations (e.g. a MacIntegrationObserver). */
  acquire: () => Promise<ExternalActivityEvent[]>
}

export type SyncMacObservationsResult = {
  events: ExternalActivityEvent[]
  outcomes: IntegrationInboxProcessingOutcome[]
}

/**
 * The full CRM-23 intake loop: acquire facts from the Mac observer(s), lower
 * them into neutral ExternalActivityEvents, and process each through the
 * durable inbox. Acquisition is separated from processing (the observer never
 * decides business outcomes).
 */
export async function syncMacObservations(
  input: SyncMacObservationsInput,
): Promise<SyncMacObservationsResult> {
  const events = await input.acquire()
  const outcomes: IntegrationInboxProcessingOutcome[] = []
  for (const event of events) {
    // eslint-disable-next-line no-await-in-loop
    outcomes.push(await processExternalActivityEvent({ ...input, event }))
  }
  return { events, outcomes }
}
