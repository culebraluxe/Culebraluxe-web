import type { CommandResult, CommandOutcome } from '../lib/workflow/contracts'
import { claimReceipt, finalizeReceipt, readFinalReceipt } from './workflow-command-receipt'
import { neonTx, type TxRunner } from './tx'

// ---------------------------------------------------------------------------
// Canonical ACCEPT OFFER (CRM-14).
//
// V1 competing-offer policy:
//   1. Exactly one offer may be accepted/primary for a deal at a time.
//   2. Accepting does NOT reject/delete/withdraw/mutate competing offers.
//   3. Competing offers are preserved unchanged (historical/potential backup).
//   4. No new "backup" status is invented.
//   5. A second offer cannot become accepted while another is accepted —
//      explicit conflict/precondition result.
//
// The only canonical mutation performed is the target offer's status; there is
// no established application semantics that require a deal mutation on accept,
// so none is invented.
//
// Idempotency uses the claim-first receipt: the same commandId executes its
// business effect at most once, and every concurrent/repeated caller observes
// the winner's final outcome.
// ---------------------------------------------------------------------------

export type AcceptOfferInput = {
  dealId: string
  offerId: string
  commandId: string
  actorAppUserId?: string | null
}

export async function acceptOffer(
  input: AcceptOfferInput,
  run: TxRunner = neonTx,
): Promise<CommandResult> {
  return run(async (tx) => {
    // Claim (serializes on command_id). A loser replays the winner's result.
    const claimed = await claimReceipt(tx, input.commandId)
    if (!claimed) {
      const receipt = await readFinalReceipt(tx, input.commandId)
      return {
        commandId: input.commandId,
        outcome: (receipt?.outcome ?? 'success') as CommandOutcome,
        emittedEvents: [],
        aggregateId: receipt?.aggregateId ?? null,
        message: receipt?.message ?? null,
        replayed: true,
      }
    }

    // Winner path — validate then mutate, tracking the final outcome.
    let outcome: CommandOutcome = 'success'
    let aggregateId: string | null = input.offerId
    let message: string | null = null

    const dealRows = await tx`
      select id from deal where id = ${input.dealId} limit 1 for update
    `
    if (!dealRows[0]) {
      outcome = 'not_found'
      aggregateId = null
      message = 'Deal not found.'
    } else {
      const offerRows = await tx`
        select id, deal_id, status from offer where id = ${input.offerId} limit 1 for update
      `
      const offer = offerRows[0] as
        | { id: string; deal_id: string; status: string }
        | undefined
      if (!offer) {
        outcome = 'not_found'
        aggregateId = null
        message = 'Offer not found.'
      } else if (offer.deal_id !== input.dealId) {
        outcome = 'validation_failure'
        aggregateId = null
        message = 'Offer does not belong to this deal.'
      } else if (offer.status !== 'submitted') {
        outcome = 'precondition_failure'
        aggregateId = null
        message = 'Offer is not in an actionable state.'
      } else {
        const acceptedRows = await tx`
          select id from offer where deal_id = ${input.dealId} and status = 'accepted' limit 1
        `
        if (acceptedRows[0]) {
          outcome = 'conflict'
          aggregateId = null
          message = 'A different offer is already accepted for this deal.'
        } else {
          await tx`
            update offer
            set status = 'accepted', responded_at = now(), updated_at = now()
            where id = ${input.offerId} and status = 'submitted'
          `
        }
      }
    }

    await finalizeReceipt(tx, input.commandId, outcome, aggregateId, message)

    return {
      commandId: input.commandId,
      outcome,
      emittedEvents: [],
      aggregateId,
      message,
      replayed: false,
    }
  })
}
