import type { QueryExecutor } from './query-executor'
import type { CommandOutcome } from '../lib/workflow/contracts'

// ---------------------------------------------------------------------------
// Application-side command idempotency receipt (migration 018).
//
// Claim-first pattern: UNIQUE(command_id) is the serialization boundary.
//
//   winner: claimReceipt -> INSERT ... ON CONFLICT DO NOTHING returns a row
//           (the INSERT itself blocks on any in-flight conflicting claim),
//           then the business effect runs, then finalizeReceipt writes the
//           final outcome — all in ONE transaction, so the 'pending' sentinel
//           never persists (rollback removes it; commit finalizes first).
//   loser:  claimReceipt returns no row (the winner's INSERT won), then
//           readFinalReceipt reads the winner's committed outcome and replays
//           it. The losing INSERT blocks until the winner commits/rolls back,
//           so a losing caller always observes the winner's final result.
//
// A persisted outcome='pending' is the claim sentinel, NOT a terminal
// CommandOutcome. `replayOutcome` maps it (and any missing receipt) to an
// explicit retryable 'conflict' so it can never be cast to CommandOutcome and
// never reaches workflow_engine as an unknown outcome.
//
// AUTH-05: the receipt also records the acting app_user (actor_app_user_id,
// migration 038) for the allow-listed consequential commands — the receipt is
// the durable actor/action/resource/time/outcome record for those commands.
// The actor is OPTIONAL and written in the same transaction as the mutation +
// receipt; it never gates the mutation and reads never depend on it.
// ---------------------------------------------------------------------------

/** Stored receipt outcome: a terminal CommandOutcome or the 'pending' sentinel. */
export type ReceiptOutcome = CommandOutcome | 'pending'

export type CommandReceipt = {
  commandId: string
  outcome: ReceiptOutcome
  aggregateId: string | null
  message: string | null
  /** AUTH-05: acting app_user recorded when the caller supplied one. */
  actorAppUserId?: string | null
}

export type ReplayDecision = {
  outcome: CommandOutcome
  message: string | null
}

/**
 * Convert a stored receipt into a valid terminal CommandOutcome for replay.
 * A missing receipt or a still-'pending' receipt is NOT a successful replay and
 * NOT a failed terminal result — it is an in-flight condition reported as a
 * retryable 'conflict'.
 */
export function replayOutcome(receipt: CommandReceipt | null): ReplayDecision {
  if (!receipt) {
    return { outcome: 'conflict', message: 'Command has no receipt; treat as in-flight.' }
  }
  if (receipt.outcome === 'pending') {
    return {
      outcome: 'conflict',
      message: 'Command claim is in-flight (pending receipt); retry later.',
    }
  }
  return { outcome: receipt.outcome, message: receipt.message }
}

/** Claim a commandId. Returns true only for the single winner. */
export async function claimReceipt(
  tx: QueryExecutor,
  commandId: string,
  actorAppUserId: string | null = null,
): Promise<boolean> {
  const rows = await tx`
    insert into workflow_command_receipt (
      command_id, outcome, aggregate_id, message, actor_app_user_id
    ) values (
      ${commandId}, 'pending', null, null, ${actorAppUserId}
    )
    on conflict (command_id) do nothing
    returning command_id
  `
  return rows.length > 0
}

/** Record the winner's final outcome (same transaction as the effect). */
export async function finalizeReceipt(
  tx: QueryExecutor,
  commandId: string,
  outcome: CommandOutcome,
  aggregateId: string | null,
  message: string | null,
  actorAppUserId: string | null = null,
): Promise<void> {
  await tx`
    update workflow_command_receipt
    set outcome = ${outcome}, aggregate_id = ${aggregateId}, message = ${message},
        actor_app_user_id = ${actorAppUserId}
    where command_id = ${commandId}
  `
}

/** Read the winner's committed final outcome (may be 'pending' if half-written). */
export async function readFinalReceipt(
  tx: QueryExecutor,
  commandId: string,
): Promise<CommandReceipt | null> {
  const rows = await tx`
    select command_id, outcome, aggregate_id, message, actor_app_user_id
    from workflow_command_receipt
    where command_id = ${commandId}
    limit 1
  `
  const r = rows[0]
  if (!r) return null
  return {
    commandId: r.command_id as string,
    outcome: r.outcome as ReceiptOutcome,
    aggregateId: (r.aggregate_id as string | null) ?? null,
    message: (r.message as string | null) ?? null,
    actorAppUserId: (r.actor_app_user_id as string | null) ?? null,
  }
}
