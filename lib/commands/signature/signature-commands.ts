// ---------------------------------------------------------------------------
// DOC-03 — Canonical command wrappers: signature.request.send / .status /
// .cancel / .decline.
//
// Thin adapters over the canonical signature-request service
// (db/signature-request.ts). The service owns legality, the claim-first
// receipt and the canonical mutation; these handlers translate the envelope
// into the service call, validate the neutral transport shape, and emit the
// neutral DomainEvents (SIGNATURE_REQUEST_SENT / _COMPLETED / _DECLINED /
// _VOIDED) with the envelope's correlation/causation when a transition
// actually happens. NO provider call happens here — provider dispatch belongs
// to the application router (lib/signature/application.ts), strictly AFTER a
// command has committed (rejected design: synchronous provider calls inside a
// domain service or a transaction).
// ---------------------------------------------------------------------------

import type { CommandEnvelope, CommandExecutionContext, CommandHandler, CommandResult } from '../contracts'
import type { DomainEvent } from '../../workflow/contracts'
import {
  SIGNATURE_REQUEST_CANCEL,
  SIGNATURE_REQUEST_DECLINE,
  SIGNATURE_REQUEST_SEND,
  SIGNATURE_REQUEST_STATUS,
} from '../command-types'
import { createDomainEventFromCommand } from '../domain-events'
import {
  applySignatureRequestStatus,
  cancelSignatureRequest,
  declineSignatureRequest,
  sendSignatureRequest,
  type ApplySignatureRequestStatusResult,
  type SendSignatureRequestResult,
} from '../../../db/signature-request'
import type {
  CancelSignatureRequestCommandInput,
  DeclineSignatureRequestCommandInput,
  SendSignatureRequestCommandInput,
  StatusSignatureRequestCommandInput,
} from '../../signature/contracts'
import {
  SIGNATURE_EVENT_TYPE_BY_STATUS,
  validateSignatureRecipients,
} from '../../signature/contracts'
import { normalizeEmail } from '../../agreements/participants'

export {
  SIGNATURE_REQUEST_CANCEL,
  SIGNATURE_REQUEST_DECLINE,
  SIGNATURE_REQUEST_SEND,
  SIGNATURE_REQUEST_STATUS,
}

/**
 * Emit the neutral DomainEvent for a status transition (sent/completed/
 * declined/voided) when the service actually changed the status — never on
 * replay, no-op or read-only applications.
 */
function emitTransitionEvent(
  envelope: CommandEnvelope,
  ctx: CommandExecutionContext,
  result: CommandResult,
): void {
  if (result.replayed || result.outcome !== 'success') return
  const value = result.value as ApplySignatureRequestStatusResult | undefined
  if (!value?.transitioned) return
  const eventType = SIGNATURE_EVENT_TYPE_BY_STATUS[value.signatureRequest.status]
  if (!eventType) return
  const event: DomainEvent = createDomainEventFromCommand(envelope, {
    eventType,
    payload: {
      signatureRequestId: value.signatureRequest.id,
      transactionDocumentId: value.signatureRequest.transactionDocumentId,
      status: value.signatureRequest.status,
    },
    aggregateType: 'signature_request',
    aggregateId: value.signatureRequest.id,
  })
  ctx.events.add(event)
}

export class SendSignatureRequestCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    const input = envelope.input as SendSignatureRequestCommandInput
    // Transport-shape validation (application owns authority/validation). The
    // recipients never reach the canonical record; they flow to the provider
    // through the seam after this command commits.
    const recipientErrors = validateSignatureRecipients(input.recipients ?? [])
    if (recipientErrors.length > 0) {
      return {
        commandId: envelope.commandId,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: null,
        message: recipientErrors.join(' '),
        replayed: false,
      }
    }
    // CRM-27: a slot-bound request must contain exactly one recipient (one request
    // per issued execution slot in V1).
    if (input.executionSlotId && (input.recipients?.length ?? 0) !== 1) {
      return {
        commandId: envelope.commandId,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: null,
        message: 'A slot-bound signature request must have exactly one recipient.',
        replayed: false,
      }
    }
    // CRM-27 (bind the ACTUAL provider recipient to the immutable slot): the
    // supplied executionSlotId / executionRole / slotRecipientEmail and the actual
    // provider recipient must all describe the same slot. The client must not be
    // able to reach the provider with a recipient that differs from the slot even
    // while supplying a "correct" auxiliary slotRecipientEmail.
    if (input.executionSlotId) {
      const actualEmail = (input.recipients ?? [])[0]?.email ?? null
      if (!input.slotRecipientEmail) {
        return {
          commandId: envelope.commandId,
          outcome: 'validation_failure',
          emittedEvents: [],
          aggregateId: null,
          message: 'A slot-bound signature request requires slotRecipientEmail.',
          replayed: false,
        }
      }
      if (normalizeEmail(actualEmail) !== normalizeEmail(input.slotRecipientEmail)) {
        return {
          commandId: envelope.commandId,
          outcome: 'validation_failure',
          emittedEvents: [],
          aggregateId: null,
          message: 'The actual recipient email must match the immutable execution slot recipient.',
          replayed: false,
        }
      }
    }
    return sendSignatureRequest(
      {
        commandId: envelope.commandId,
        transactionDocumentId: input.transactionDocumentId,
        message: input.message ?? null,
        createdByUserId: input.createdByUserId ?? null,
        executionRole: input.executionRole ?? null,
        executionSlotId: input.executionSlotId ?? null,
        slotRecipientEmail: input.slotRecipientEmail ?? null,
      },
      ctx.run,
    )
  }
}

export class StatusSignatureRequestCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    const input = envelope.input as StatusSignatureRequestCommandInput
    const result = await applySignatureRequestStatus(
      {
        commandId: envelope.commandId,
        signatureRequestId: input.signatureRequestId,
        targetStatus: input.targetStatus ?? null,
      },
      ctx.run,
    )
    emitTransitionEvent(envelope, ctx, result)
    return result
  }
}

export class CancelSignatureRequestCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    const input = envelope.input as CancelSignatureRequestCommandInput
    const result = await cancelSignatureRequest(
      {
        commandId: envelope.commandId,
        signatureRequestId: input.signatureRequestId,
      },
      ctx.run,
    )
    emitTransitionEvent(envelope, ctx, result)
    return result
  }
}

export class DeclineSignatureRequestCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    const input = envelope.input as DeclineSignatureRequestCommandInput
    const result = await declineSignatureRequest(
      {
        commandId: envelope.commandId,
        signatureRequestId: input.signatureRequestId,
      },
      ctx.run,
    )
    emitTransitionEvent(envelope, ctx, result)
    return result
  }
}

// Re-exported for the send result shape (tests / router).
export type { SendSignatureRequestResult }
