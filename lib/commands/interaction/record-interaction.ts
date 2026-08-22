// ---------------------------------------------------------------------------
// CRM-14J / CRM-23 — Canonical command wrapper: interaction.record.
//
// Thin adapter over the existing canonical service db/interactions.ts
// (createInteraction). The service owns validation, the idempotent
// (source_system, source_external_id) insert and the canonical interaction
// row; this handler translates the envelope into the service call and emits
// the INTERACTION_RECORDED domain event (the committed fact downstream
// alerting/integrations consume via the future CRM-14J outbox).
//
// CRM-23 uses this command as the canonical Business Command seam for
// integration-inbox interaction persistence (criterion 7): the inbox
// processor never writes the interaction row directly when wired for
// production — it executes interaction.record through the canonical
// dispatcher, so every canonical CRM change runs through the command layer.
//
// Receipt-backed (new-style handler): claim-first on the UNIQUE commandId so
// concurrent/duplicate dispatches serialize; the winner commits the mutation
// + receipt in ONE transaction; a replaying caller reads the winner's receipt.
// No business rules live here.
// ---------------------------------------------------------------------------

import { createInteraction } from '../../../db/interactions'
import type { CreateInteractionInput } from '../../crm-types'
import type {
  CommandEnvelope,
  CommandExecutionContext,
  CommandHandler,
  CommandResult,
} from '../contracts'
import { INTERACTION_RECORD } from '../command-types'
import { createDomainEventFromCommand } from '../domain-events'

export { INTERACTION_RECORD }

export class RecordInteractionCommand
  implements CommandHandler<CommandEnvelope, CommandResult>
{
  async handle(
    envelope: CommandEnvelope,
    ctx: CommandExecutionContext,
  ): Promise<CommandResult> {
    const input = envelope.input as unknown as CreateInteractionInput
    if (
      !input?.personId ||
      !input?.channel ||
      !input?.eventType ||
      !input?.occurredAt
    ) {
      return {
        commandId: envelope.commandId,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: null,
        message:
          'interaction.record requires personId, channel, eventType and occurredAt.',
        replayed: false,
      }
    }

    // Claim-first: the single winner of a commandId executes; losers report
    // an in-flight conflict (the dispatcher replays the winner's committed
    // receipt once it commits).
    const claimed = await ctx.receipts.claim(envelope.commandId, ctx.tx)
    if (!claimed) {
      return {
        commandId: envelope.commandId,
        outcome: 'conflict',
        emittedEvents: [],
        aggregateId: null,
        message: 'interaction.record is already in flight for this commandId.',
        replayed: false,
      }
    }

    try {
      const { interaction, created } = await createInteraction(input, ctx.tx)

      // EVENT = FACT: the committed interaction is a domain event consumed by
      // downstream subscribers (future CRM-14J outbox). Added to the
      // collector so it commits atomically with the mutation + receipt.
      ctx.events.add(
        createDomainEventFromCommand(envelope, {
          eventType: 'INTERACTION_RECORDED',
          aggregateType: 'interaction',
          aggregateId: interaction.id,
          payload: {
            interactionId: interaction.id,
            personId: interaction.personId,
            channel: interaction.channel,
            eventType: interaction.eventType,
            direction: interaction.direction ?? null,
            sourceSystem: interaction.sourceSystem ?? null,
            sourceExternalId: interaction.sourceExternalId ?? null,
            created,
          },
        }),
      )

      await ctx.receipts.save(
        {
          commandId: envelope.commandId,
          outcome: 'success',
          status: 'Succeeded',
          aggregateId: interaction.id,
          message: null,
          createdAt: new Date().toISOString(),
          actorAppUserId: envelope.actorAppUserId ?? null,
        },
        ctx.tx,
      )

      return {
        commandId: envelope.commandId,
        outcome: 'success',
        emittedEvents: [],
        aggregateId: interaction.id,
        message: null,
        replayed: false,
        value: { interactionId: interaction.id, created },
      }
    } catch (error) {
      // Deterministic rejection (validation) — no receipt is finalized; the
      // transaction rolls back (the claim row disappears), so a corrected
      // resubmission can re-execute.
      return {
        commandId: envelope.commandId,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: null,
        message: error instanceof Error ? error.message : 'interaction.record failed',
        replayed: false,
      }
    }
  }
}
