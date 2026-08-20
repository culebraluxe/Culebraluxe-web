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
// This protects execution independently of each domain command's incidental
// state checks; the same commandId executes its business effect at most once.
// ---------------------------------------------------------------------------

export type CommandReceipt = {
  commandId: string
  outcome: CommandOutcome
  aggregateId: string | null
  message: string | null
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

/** Read the winner's committed final outcome. */
export async function readFinalReceipt(
  tx: QueryExecutor,
  commandId: string,
): Promise<CommandReceipt | null> {
  const rows = await tx`
    select command_id, outcome, aggregate_id, message
    from workflow_command_receipt
    where command_id = ${commandId}
    limit 1
  `
  const r = rows[0]
  if (!r) return null
  return {
    commandId: r.command_id as string,
    outcome: r.outcome as CommandOutcome,
    aggregateId: (r.aggregate_id as string | null) ?? null,
    message: (r.message as string | null) ?? null,
  }
}
