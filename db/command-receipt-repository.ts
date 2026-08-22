import type {
  ApplicationTransaction,
  CommandReceipt,
  CommandReceiptRepository,
} from '../lib/commands/contracts'
import { commandReceiptStatus } from '../lib/commands/contracts'
import {
  claimReceipt,
  finalizeReceipt,
  readFinalReceipt,
} from './workflow-command-receipt'

// ---------------------------------------------------------------------------
// CRM-14J — Canonical CommandReceiptRepository over the existing
// `workflow_command_receipt` claim-first pattern (migration 018 / manual v4).
//
// This is the generalized receipt repository: the SAME table and the SAME
// winner/loser semantics the canonical deal services already use, now exposed
// through the canonical command-layer contract. Current workflow replay
// behavior is preserved exactly (claim-first INSERT ... ON CONFLICT DO
// NOTHING; 'pending' is the in-flight sentinel, never a terminal outcome; a
// losing caller replays the winner's committed result).
//
// The richer CommandReceipt fields (commandType, correlationId, causationId,
// aggregateType, resultPayload, errorCode, errorMessage) are compile-ready
// contract surface; the current table stores command_id / outcome /
// aggregate_id / message / created_at. A future additive migration can extend
// the row when a durable consumer needs those columns (CRM-14I defer).
// ---------------------------------------------------------------------------

export class PostgresCommandReceiptRepository
  implements CommandReceiptRepository
{
  async find(
    commandId: string,
    tx: ApplicationTransaction,
  ): Promise<CommandReceipt | null> {
    const stored = await readFinalReceipt(tx, commandId)
    if (!stored) return null
    return {
      commandId: stored.commandId,
      outcome: stored.outcome,
      status: commandReceiptStatus(stored.outcome),
      aggregateId: stored.aggregateId,
      message: stored.message,
      // created_at is not returned by readFinalReceipt; the canonical contract
      // field stays null for the current row shape.
      createdAt: null,
    }
  }

  async save(
    receipt: CommandReceipt,
    tx: ApplicationTransaction,
  ): Promise<void> {
    if (receipt.outcome === 'pending') {
      throw new Error(
        `Cannot finalize receipt ${receipt.commandId} with the 'pending' sentinel.`,
      )
    }
    await finalizeReceipt(
      tx,
      receipt.commandId,
      receipt.outcome,
      receipt.aggregateId,
      receipt.errorMessage ?? receipt.message,
    )
  }

  async claim(
    commandId: string,
    tx: ApplicationTransaction,
  ): Promise<boolean> {
    return claimReceipt(tx, commandId)
  }
}
