import type {
  ApplicationTransaction,
  CommandReceipt,
  CommandReceiptRepository,
  ReceiptChainMetadata,
} from '../lib/commands/contracts'
import { commandReceiptStatus } from '../lib/commands/contracts'
import {
  claimReceipt,
  finalizeReceipt,
  readFinalReceipt,
  recordReceiptMetadata,
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
// CMD-01 (migration 051): the row additionally stores actor_app_user_id,
// command_type, correlation_id and causation_id so the durable receipt answers
// who issued it, what command type, and which correlation/causation chain it
// belonged to. The chain facts are recorded by the dispatcher (recordMetadata)
// in the SAME transaction as the mutation; outcome/aggregateId/message stay
// owned by the executing service.
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
      // CMD-01: the receipt's actor + envelope chain (actor_app_user_id,
      // command_type, correlation_id, causation_id — migration 051) are part of
      // the stored row; surface them on the canonical receipt contract.
      actorAppUserId: stored.actorAppUserId,
      commandType: stored.commandType ?? undefined,
      correlationId: stored.correlationId ?? null,
      causationId: stored.causationId ?? null,
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
    // CMD-01: persist the chain facts the caller carried on the receipt in the
    // SAME transaction (no-op for producers that carried none).
    await recordReceiptMetadata(tx, receipt.commandId, {
      actorAppUserId: receipt.actorAppUserId ?? null,
      commandType: receipt.commandType ?? null,
      correlationId: receipt.correlationId ?? null,
      causationId: receipt.causationId ?? null,
    })
  }

  async claim(
    commandId: string,
    tx: ApplicationTransaction,
  ): Promise<boolean> {
    return claimReceipt(tx, commandId)
  }

  async recordMetadata(
    commandId: string,
    metadata: ReceiptChainMetadata,
    tx: ApplicationTransaction,
  ): Promise<void> {
    await recordReceiptMetadata(tx, commandId, metadata)
  }
}
