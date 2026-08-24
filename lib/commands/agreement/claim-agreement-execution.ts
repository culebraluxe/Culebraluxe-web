// ---------------------------------------------------------------------------
// CRM-27 — Canonical command: agreement.execution.claim.
//
// The DURABILITY REPAIR over the earlier predicate/marker-only front half. This
// thin handler evaluates whether a specific immutable issued agreement/document
// version is FULLY EXECUTED and, when it becomes fully executed, atomically
// commits in the dispatcher's ONE transaction:
//
//   command receipt (claimed + finalized by this handler)
//   agreement_execution marker (evaluateAgreementCompletion, same tx)
//   AGREEMENT_FULLY_EXECUTED DomainEvent (ctx.events)
//   outbox_message row (dispatcher eventSink append, same tx)
//
// A single commit covers all of them; a rollback leaves none. External side
// effects (Phase 2 MQ consumer) react to the committed outbox event AFTER
// commit. No business rules live here — the completion evaluator owns evidence
// assembly and the required-role policy seam (lib/agreements).
// ---------------------------------------------------------------------------

import { AGREEMENT_EXECUTION_CLAIM } from '../command-types'
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

    // Claim-first receipt in the dispatcher transaction. The dispatcher already
    // served the committed-receipt replay fast-path, so a false claim here means
    // a concurrent in-flight claim for the same commandId → retryable conflict.
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

    // Evaluate within the SAME transaction: reads + marker write share ctx.tx,
    // so the marker commits atomically with the receipt and the outbox row.
    const completion = await evaluateAgreementCompletion(
      transactionDocumentId,
      envelope.commandId,
      { execute: ctx.tx, run: ctx.run, now: ctx.now },
    )

    if (completion.shouldEmit) {
      const event = createDomainEventFromCommand(envelope, {
        eventType: AGREEMENT_FULLY_EXECUTED,
        aggregateType: 'transaction_document',
        aggregateId: transactionDocumentId,
        payload: {
          transactionDocumentId,
          issuedVersion: completion.document?.issuedVersion ?? null,
          templateId: completion.templateId,
          dealId: completion.dealId,
          agreementVersion:
            completion.templateId && completion.document
              ? `${completion.templateId}-v${completion.document.issuedVersion}`
              : null,
        },
      })
      // The dispatcher drains ctx.events and appends them to the outbox in the
      // SAME transaction (step 8); it also merges the drained set into the
      // returned result, so do NOT return it here (would duplicate).
      ctx.events.add(event)
    }

    await ctx.receipts.save(
      {
        commandId: envelope.commandId,
        outcome: 'success',
        status: 'Succeeded',
        aggregateId: transactionDocumentId,
        message: completion.shouldEmit
          ? 'Agreement fully executed; AGREEMENT_FULLY_EXECUTED emitted.'
          : 'Agreement not fully executed yet (or already recorded).',
        actorAppUserId: envelope.actorAppUserId ?? null,
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
          shouldEmit: completion.shouldEmit,
          verdict: completion.verdict,
          document: completion.document,
          templateId: completion.templateId,
          dealId: completion.dealId,
        },
      },
    }
  }
}
