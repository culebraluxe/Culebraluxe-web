// ---------------------------------------------------------------------------
// CRM-14J — Canonical CommandDispatcher.
//
// Dispatcher algorithm/invariant (architect brief):
//   1. Resolve handler by commandType.
//   2. Start application transaction (Postgres V1 transport — SKIP LOCKED /
//      UNIQUE command_id receipts are already proven in this system).
//   3. Read receipt by commandId inside the transaction boundary.
//   4. If a terminal receipt already exists, return the normalized replay
//      result WITHOUT re-running the domain mutation.
//   5. Execute handler / canonical domain service.
//   6. Persist canonical business mutation(s) — inside the domain service.
//   7. Persist CommandReceipt — inside the domain service (claim-first
//      finalizeReceipt), within this same transaction.
//   8. Persist any collected outbox events in the SAME transaction when the
//      event outbox seam is enabled.
//   9. Commit once (the surrounding TxRunner commits).
//  10. Return the normalized result.
//
// Error semantics: a deterministic business rejection is a returned
// CommandResult (stable, non-retryable for validation/not_found; conflict is
// retryable). Infrastructure failure THROWS — the transaction rolls back and
// leaves no mutation and no success receipt; retry is by re-submitting the
// same commandId. Replay success is never reported when the original
// transaction did not commit (only committed receipts are replayed; a missing
// or 'pending' receipt is an explicit retryable conflict).
//
// Receipt persistence note: the phase-1 handlers wrap canonical services that
// already own claim-first receipt persistence (db/workflow-command-receipt.ts).
// The dispatcher therefore does NOT double-write receipts — it reads them for
// the replay fast-path (step 3-4) and lets the service's finalizeReceipt write
// inside the same transaction (step 7). New-style handlers use ctx.receipts
// (claim/save) for the same effect.
//
// No business side effect may exist between steps 5-8 that cannot participate
// in this durability model; external side effects belong to outbox subscribers
// AFTER commit.
// ---------------------------------------------------------------------------

import type { TxRunner } from '../../db/tx'
import { replayOutcome } from '../../db/workflow-command-receipt'
import type {
  CommandEnvelope,
  CommandDispatcher,
  CommandExecutionContext,
  CommandReceipt,
  CommandRegistry,
  CommandReceiptRepository,
  CommandResult,
  CommandFailureCategory,
  DomainEvent,
  TypedCommandEnvelope,
  TypedCommandResult,
} from './contracts'
import { InMemoryDomainEventCollector } from './domain-events'
import type { OutboxEventRepository } from '../events/outbox-contracts'

export type CommandDispatcherOptions = {
  registry: CommandRegistry
  receipts: CommandReceiptRepository
  /** Postgres V1 transport — the transaction runner. */
  run: TxRunner
  /** Clock injection for deterministic tests. */
  now?: () => Date
  /**
   * Durable outbox sink. Null/absent until a real cross-cutting consumer
   * exists (alerting, integrations, cross-workflow triggers, automation) —
   * CRM-14I defer decision preserved; do not build the outbox loop/table
   * without a consumer.
   */
  eventSink?: OutboxEventRepository | null
}

function unknownCommandResult(envelope: CommandEnvelope): CommandResult {
  return {
    commandId: envelope.commandId,
    commandType: envelope.commandType,
    outcome: 'not_found',
    emittedEvents: [],
    aggregateId: null,
    message: `Unknown command type: ${envelope.commandType}`,
    replayed: false,
    error: {
      code: 'unknown_command_type',
      message: `Unknown command type: ${envelope.commandType}`,
      retryable: false,
      category: 'validation',
    },
  }
}

/** Merge handler-returned and collector events, deduped by eventId. */
function mergeEvents(a: DomainEvent[], b: DomainEvent[]): DomainEvent[] {
  const seen = new Set<string>()
  const out: DomainEvent[] = []
  for (const e of [...a, ...b]) {
    if (seen.has(e.eventId)) continue
    seen.add(e.eventId)
    out.push(e)
  }
  return out
}

function failureCategory(
  outcome: CommandResult['outcome'],
  replayed: boolean,
): CommandFailureCategory {
  if (replayed) return 'replay'
  switch (outcome) {
    case 'validation_failure':
    case 'not_found':
    case 'unauthorized':
    case 'precondition_failure':
    case 'conflict':
      return 'validation'
    default:
      return 'unknown'
  }
}

function normalizeResult(
  envelope: CommandEnvelope,
  result: CommandResult,
  emitted: DomainEvent[],
  hasReceipt: boolean,
): CommandResult {
  const replayed = result.replayed ?? false
  const outcome = result.outcome
  // CMD-01 — explicit failure taxonomy + guaranteed non-empty failure message.
  const error =
    result.error !== undefined || outcome !== 'success'
      ? {
          code: result.error?.code ?? outcome,
          message:
            result.error?.message?.trim() ||
            result.message?.trim() ||
            `Command '${envelope.commandType}' failed with outcome '${outcome}'.`,
          retryable: result.error?.retryable ?? outcome === 'conflict',
          category: result.error?.category ?? failureCategory(outcome, replayed),
        }
      : undefined
  return {
    commandId: envelope.commandId,
    // CMD-01 — the dispatcher stamps the envelope's stable command type on
    // every result (the receipt row carries it durably as well).
    commandType: envelope.commandType,
    outcome,
    emittedEvents: mergeEvents(result.emittedEvents, emitted),
    aggregateId: result.aggregateId ?? null,
    message: result.message ?? null,
    replayed,
    value: result.value,
    error,
    receiptId: result.receiptId ?? (hasReceipt ? envelope.commandId : undefined),
  }
}

export class CommandDispatcherImpl implements CommandDispatcher {
  constructor(private readonly options: CommandDispatcherOptions) {}

  async execute<TPayload extends Record<string, unknown>, TResult = unknown>(
    command: TypedCommandEnvelope<TPayload>,
  ): Promise<TypedCommandResult<TResult>> {
    // CMD-01 — the durable receipt is keyed on commandId; a caller-supplied
    // idempotency key MUST be the commandId (no second identity is ever
    // introduced). Deterministic contract rejection, not a technical failure.
    if (
      command.idempotencyKey !== undefined &&
      command.idempotencyKey !== command.commandId
    ) {
      const message =
        `idempotencyKey '${command.idempotencyKey}' must equal commandId ` +
        `'${command.commandId}'; the durable receipt is keyed on commandId.`
      return {
        commandId: command.commandId,
        commandType: command.commandType,
        outcome: 'validation_failure',
        emittedEvents: [],
        aggregateId: command.aggregateId ?? null,
        message,
        replayed: false,
        error: {
          code: 'invalid_idempotency_key',
          message,
          retryable: false,
          category: 'validation',
        },
      } as TypedCommandResult<TResult>
    }

    // 1. Resolve handler by commandType.
    const handler = this.options.registry.resolve(command.commandType)
    if (!handler) {
      return unknownCommandResult(command) as TypedCommandResult<TResult>
    }

    // 2. Start the application transaction (commit once at the end).
    return this.options.run(async (tx) => {
      // 3-4. Replay fast-path: a terminal receipt replays without re-running.
      const existing = await this.options.receipts.find(command.commandId, tx)
      if (existing) {
        return this.replayResult(
          command,
          existing,
        ) as TypedCommandResult<TResult>
      }

      // 5-7. Execute the handler / canonical domain service. The handler's
      // canonical service claims + finalizes the receipt inside THIS tx and
      // mutates canonical truth; external side effects never run here.
      const collector = new InMemoryDomainEventCollector()
      const ctx: CommandExecutionContext = {
        tx,
        receipts: this.options.receipts,
        registry: this.options.registry,
        events: collector,
        run: (cb) => cb(tx),
        now: this.options.now ?? (() => new Date()),
      }
      const result = await handler.handle(command, ctx)

      // CMD-01 — persist the envelope chain facts (actor, command type,
      // correlation/causation) on the receipt row in the SAME transaction as
      // the mutation. Outcome/aggregateId/message stay owned by the executing
      // service; this only enriches the durable proof (no-op when the handler
      // wrote no receipt row).
      await this.options.receipts.recordMetadata(
        command.commandId,
        {
          actorAppUserId: command.actorAppUserId,
          commandType: command.commandType,
          correlationId: command.correlationId,
          causationId: command.causationId,
        },
        tx,
      )

      // 8. Outbox rows in the SAME transaction when the seam is enabled.
      const emitted = collector.drain()
      if (this.options.eventSink && emitted.length > 0) {
        await this.options.eventSink.append(emitted, tx)
      }

      // 10. Normalize (receiptId is honest only when a receipt row exists).
      const persisted = await this.options.receipts.find(command.commandId, tx)
      return normalizeResult(
        command,
        result,
        emitted,
        persisted !== null,
      ) as TypedCommandResult<TResult>
    })
  }

  private replayResult(
    command: CommandEnvelope,
    receipt: CommandReceipt,
  ): CommandResult {
    const decision = replayOutcome({
      commandId: receipt.commandId,
      outcome: receipt.outcome,
      aggregateId: receipt.aggregateId,
      message: receipt.message,
    })
    const outcome = decision.outcome
    // CMD-01 — replay failures are explicit, categorized and never blank.
    const message =
      decision.message?.trim() ||
      `Command '${command.commandType}' was replayed from its committed receipt with outcome '${outcome}'.`
    return {
      commandId: command.commandId,
      commandType: command.commandType,
      outcome,
      emittedEvents: [],
      aggregateId: receipt.aggregateId ?? null,
      message,
      replayed: true,
      error:
        outcome === 'success'
          ? undefined
          : {
              code: outcome,
              message,
              retryable: outcome === 'conflict',
              category: 'replay',
            },
      receiptId: command.commandId,
    }
  }
}
