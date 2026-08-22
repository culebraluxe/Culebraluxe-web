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
// ---------------------------------------------------------------------------

/** Stored receipt outcome: a terminal CommandOutcome or the 'pending' sentinel. */
export type ReceiptOutcome = CommandOutcome | 'pending'

export type CommandReceipt = {
  commandId: string
  outcome: ReceiptOutcome
  aggregateId: string | null
  message: string | null
  // CMD-01 (migration 051): envelope chain metadata persisted on the row.
  actorAppUserId?: string | null
  commandType?: string | null
  correlationId?: string | null
  causationId?: string | null
}

/** CMD-01 — envelope chain facts recorded on an existing receipt row. */
export type ReceiptMetadata = {
  actorAppUserId?: string | null
  commandType?: string | null
  correlationId?: string | null
  causationId?: string | null
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
): Promise<boolean> {
  const rows = await tx`
    insert into workflow_command_receipt (
      command_id, outcome, aggregate_id, message
    ) values (
      ${commandId}, 'pending', null, null
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
): Promise<void> {
  await tx`
    update workflow_command_receipt
    set outcome = ${outcome}, aggregate_id = ${aggregateId}, message = ${message}
    where command_id = ${commandId}
  `
}

/** Read the winner's committed final outcome (may be 'pending' if half-written). */
export async function readFinalReceipt(
  tx: QueryExecutor,
  commandId: string,
): Promise<CommandReceipt | null> {
  const rows = await tx`
    select command_id, outcome, aggregate_id, message, actor_app_user_id,
      command_type, correlation_id, causation_id
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
    commandType: (r.command_type as string | null) ?? null,
    correlationId: (r.correlation_id as string | null) ?? null,
    causationId: (r.causation_id as string | null) ?? null,
  }
}

/**
 * CMD-01 — record envelope chain metadata on an existing receipt row in the
 * SAME transaction as the mutation + receipt outcome. Outcome/aggregateId/
 * message stay untouched (owned by the executing service). No-op when the
 * commandId has no receipt row.
 */
export async function recordReceiptMetadata(
  tx: QueryExecutor,
  commandId: string,
  metadata: ReceiptMetadata,
): Promise<void> {
  await tx`
    update workflow_command_receipt
    set actor_app_user_id = ${metadata.actorAppUserId ?? null},
        command_type = ${metadata.commandType ?? null},
        correlation_id = ${metadata.correlationId ?? null},
        causation_id = ${metadata.causationId ?? null}
    where command_id = ${commandId}
  `
}
