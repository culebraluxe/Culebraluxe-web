import { randomUUID } from 'node:crypto'
import { AGREEMENT_EXECUTION_CLAIM } from '../command-types'
import { commandReceiptStatus } from '../contracts'
import { createDomainEventFromCommand } from '../domain-events'
import { evaluateAgreementCompletion } from '../../agreements/completion'
import { AGREEMENT_FULLY_EXECUTED } from '../../agreements/execution'
import type {
  CommandEnvelope,
  CommandExecutionContext,
  CommandHandler,
  CommandResult,
} from '../contracts'

export { AGREEMENT_EXECUTION_CLAIM }

export class ClaimAgreementExecutionCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    const input = envelope.input as { transactionDocumentId?: unknown }
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
        message: 'agreement.execution.claim requires transactionDocumentId.',
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
        message: 'Agreement execution claim is in-flight; retry.',
        replayed: false,
      }
    }

    const eventId = randomUUID()
    const completion = await evaluateAgreementCompletion(
      transactionDocumentId,
      eventId,
      { execute: ctx.tx, run: ctx.run, now: ctx.now },
    )

    if (completion.shouldEmit) {
      const event = createDomainEventFromCommand(envelope, {
        eventType: AGREEMENT_FULLY_EXECUTED,
        eventId,
        aggregateType: 'transaction_document',
        aggregateId: transactionDocumentId,
        payload: {
          transactionDocumentId,
          issuedVersion: completion.document?.issuedVersion ?? null,
          templateId: completion.templateId,
          contractId: completion.contractId,
          dealId: completion.dealId,
          agreementVersion:
            completion.templateId && completion.document
              ? `${completion.templateId}-v${completion.document.issuedVersion}`
              : null,
        },
      })
      ctx.events.add(event)
    }

    await ctx.receipts.save(
      {
        commandId: envelope.commandId,
        outcome: completion.outcome,
        status: commandReceiptStatus(completion.outcome),
        aggregateId: transactionDocumentId,
        message: completion.error
          ? completion.error
          : completion.shouldEmit
            ? 'Agreement fully executed; AGREEMENT_FULLY_EXECUTED emitted.'
            : 'Agreement not fully executed yet (or already recorded).',
        actorAppUserId: envelope.actorAppUserId ?? null,
        createdAt: null,
      },
      ctx.tx,
    )

    return {
      commandId: envelope.commandId,
      outcome: completion.outcome,
      emittedEvents: [],
      aggregateId: transactionDocumentId,
      message: completion.error ?? null,
      replayed: false,
      value: {
        completion: {
          outcome: completion.outcome,
          error: completion.error,
          shouldEmit: completion.shouldEmit,
          verdict: completion.verdict,
          document: completion.document,
          templateId: completion.templateId,
          contractId: completion.contractId,
          dealId: completion.dealId,
          eventId: completion.eventId,
        },
      },
    }
  }
}
