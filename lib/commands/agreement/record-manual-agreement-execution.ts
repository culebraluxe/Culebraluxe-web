import { randomUUID } from 'node:crypto'
import { AGREEMENT_EXECUTION_MANUAL } from '../command-types'
import { commandReceiptStatus } from '../contracts'
import { createDomainEventFromCommand } from '../domain-events'
import { resolveAgreementDocument } from '../../agreements/completion'
import { recordManualAgreementExecution } from '../../../db/agreement-execution'
import { AGREEMENT_FULLY_EXECUTED } from '../../agreements/execution'
import type {
  CommandEnvelope,
  CommandExecutionContext,
  CommandHandler,
  CommandResult,
} from '../contracts'

export { AGREEMENT_EXECUTION_MANUAL }

const MAX_NOTE_LENGTH = 500

/** Provider-neutral audited manual/external execution command. */
export class RecordManualAgreementExecutionCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    const input = envelope.input as { transactionDocumentId?: unknown; note?: unknown }
    const transactionDocumentId =
      typeof input?.transactionDocumentId === 'string' &&
      input.transactionDocumentId !== ''
        ? input.transactionDocumentId
        : null
    if (!transactionDocumentId) {
      return {
        commandId: envelope.commandId,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: envelope.aggregateId,
        message: 'agreement.execution.manual requires transactionDocumentId.',
        replayed: false,
      }
    }

    const actor = envelope.actorAppUserId ?? null
    if (!actor) {
      return {
        commandId: envelope.commandId,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: transactionDocumentId,
        message: 'agreement.execution.manual requires an authenticated actorAppUserId.',
        replayed: false,
      }
    }
    const note = typeof input?.note === 'string' ? input.note : null
    if (note !== null && note.length > MAX_NOTE_LENGTH) {
      return {
        commandId: envelope.commandId,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: transactionDocumentId,
        message: `Manual execution note must be ${MAX_NOTE_LENGTH} characters or fewer.`,
        replayed: false,
      }
    }

    const claimed = await ctx.receipts.claim(envelope.commandId, ctx.tx)
    if (!claimed) {
      return {
        commandId: envelope.commandId,
        outcome: 'conflict',
        emittedEvents: [],
        aggregateId: transactionDocumentId,
        message: 'Agreement execution manual claim is in-flight; retry.',
        replayed: false,
      }
    }

    const eventId = randomUUID()
    const docCtx = await resolveAgreementDocument(transactionDocumentId, ctx.tx)
    if (docCtx.outcome !== 'success') {
      await ctx.receipts.save(
        {
          commandId: envelope.commandId,
          outcome: docCtx.outcome,
          status: commandReceiptStatus(docCtx.outcome),
          aggregateId: transactionDocumentId,
          message: docCtx.error ?? docCtx.outcome,
          actorAppUserId: actor,
          createdAt: null,
        },
        ctx.tx,
      )
      return {
        commandId: envelope.commandId,
        outcome: docCtx.outcome,
        emittedEvents: [],
        aggregateId: transactionDocumentId,
        message: docCtx.error ?? null,
        replayed: false,
      }
    }

    const document = docCtx.document!
    const { recorded } = await recordManualAgreementExecution(ctx.tx, {
      documentId: transactionDocumentId,
      issuedVersion: document.issuedVersion,
      eventId,
      emittedAt: ctx.now(),
      actorAppUserId: actor,
      note,
    })

    if (recorded) {
      const event = createDomainEventFromCommand(envelope, {
        eventType: AGREEMENT_FULLY_EXECUTED,
        eventId,
        aggregateType: 'transaction_document',
        aggregateId: transactionDocumentId,
        payload: {
          transactionDocumentId,
          issuedVersion: document.issuedVersion,
          templateId: docCtx.templateId,
          contractId: docCtx.contractId,
          dealId: docCtx.dealId,
          agreementVersion:
            docCtx.templateId ? `${docCtx.templateId}-v${document.issuedVersion}` : null,
          executionKind: 'manual',
          actorAppUserId: actor,
        },
      })
      ctx.events.add(event)
    }

    await ctx.receipts.save(
      {
        commandId: envelope.commandId,
        outcome: 'success',
        status: 'Succeeded',
        aggregateId: transactionDocumentId,
        message: recorded
          ? 'Manual agreement execution recorded.'
          : 'Agreement already executed; manual execution is a no-op.',
        actorAppUserId: actor,
        createdAt: null,
      },
      ctx.tx,
    )

    return {
      commandId: envelope.commandId,
      outcome: 'success',
      emittedEvents: [],
      aggregateId: transactionDocumentId,
      message: null,
      replayed: false,
      value: {
        completion: {
          outcome: 'success',
          error: null,
          shouldEmit: recorded,
          verdict: {
            fullyExecuted: true,
            missingRoles: [],
            missingSlotIds: [],
            reason: 'manual_execution',
          },
          document,
          templateId: docCtx.templateId,
          contractId: docCtx.contractId,
          dealId: docCtx.dealId,
          eventId: recorded ? eventId : null,
        },
      },
    }
  }
}
