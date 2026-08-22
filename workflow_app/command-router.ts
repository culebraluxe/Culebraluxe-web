import type {
  CommandEnvelope,
  CommandResult,
} from '../lib/workflow/contracts'
import { ROUTED_COMMAND_TYPES } from './command-types'
import { commandDispatcher } from '../lib/commands'

// ---------------------------------------------------------------------------
// CRM-14J — Command router: a translation seam, not a rule holder.
//
// workflow_app maps an engine command request -> canonical CommandEnvelope ->
// canonical CommandDispatcher. All business legality, invariant enforcement,
// receipt/replay behavior and canonical mutation live in the canonical domain
// services (db/*), reached through the thin command handlers registered in
// lib/commands/register.ts. The workflow engine never knows command internals;
// this file no longer hosts business rules or per-command switch cases.
//
// The ROUTED_COMMAND_TYPES guard below is defense-in-depth for inventory
// members lacking a registered handler (should not happen): command types
// outside the inventory are rejected before dispatch, preserving the CRM-14G
// inventory contract enforced by tests/command-inventory.test.ts and
// parseReSupermodel().
// ---------------------------------------------------------------------------

export type GapContract = {
  commandType: string
  title: string
  invariants: string[]
  schemaChange?: string
}

/** Retained for documentation; the offer/stage gaps are now implemented. */
export const ACCEPT_OFFER_CONTRACT: GapContract = {
  commandType: 'offer.accept',
  title: 'Accept Offer (canonical)',
  invariants: [
    'offer exists and belongs to the deal',
    'offer.status is actionable (submitted)',
    'one accepted/primary offer per deal',
    'competing offers preserved unchanged',
  ],
  schemaChange: 'none required — offer.status already supports `accepted`.',
}

export const SET_DEAL_STAGE_CONTRACT: GapContract = {
  commandType: 'deal.set_stage',
  title: 'Set Deal Stage (canonical)',
  invariants: [
    'compare-and-set on current stage',
    'only offer -> under_contract and under_contract -> closed',
  ],
  schemaChange: 'none required — deal.stage already exists.',
}

/**
 * Translate a workflow command envelope through the canonical dispatcher.
 * Returns the same CommandResult contract the router always returned (same
 * outcomes, messages, replayed flags) — no business behavior change.
 */
export async function routeCommand(
  envelope: CommandEnvelope,
): Promise<CommandResult> {
  if (!ROUTED_COMMAND_TYPES.has(envelope.commandType)) {
    return {
      commandId: envelope.commandId,
      outcome: 'not_found',
      emittedEvents: [],
      aggregateId: null,
      message: `Unknown command type: ${envelope.commandType}`,
      replayed: false,
    }
  }
  return commandDispatcher.execute(envelope)
}
