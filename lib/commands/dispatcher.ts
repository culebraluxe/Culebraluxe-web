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
  /**
   * OPTIONAL observer-only Flight Recorder. When set, the dispatcher records
   * COMMAND_RECEIVED / COMMAND_REPLAYED / COMMAND_COMPLETED / COMMAND_FAILED
   * and DOMAIN_EVENT_EMITTED trace evidence. Recorder calls are ALWAYS wrapped
   * in a contained try/catch — a recorder failure can never break command
   * execution or its transaction. Absent (default) = no recording (zero change
   * to existing callers/tests).
   */
  traceRecorder?: TraceRecorder | null
}

/** Structural, observer-only trace recorder signature (decoupled from the DB). */
export type TraceRecorder = (input: {
  eventType: string
  system: string
  occurredAt: string
  completedAt?: string | null
  durationMs?: number | null
  outcome?: string | null
  commandId?: string | null
  domainEventId?: string | null
  correlationId?: string | null
  causationId?: string | null
  dealId?: string | null
  workflowInstanceId?: string | null
  summary?: string | null
  metadata?: Record<string, unknown> | null
}) => Promise<void>

function unknownCommandResult(envelope: CommandEnvelope): CommandResult {
  return {
    commandId: envelope.commandId,
    outcome: 'not_found',
    emittedEvents: [],
    aggregateId: null,
    message: `Unknown command type: ${envelope.commandType}`,
    replayed: false,
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

function normalizeResult(
  envelope: CommandEnvelope,
  result: CommandResult,
  emitted: DomainEvent[],
  hasReceipt: boolean,
): CommandResult {
  const error =
    result.error ??
    (result.outcome === 'success'
      ? undefined
      : {
          code: result.outcome,
          message:
            result.message ??
            `Command '${envelope.commandType}' failed with outcome '${result.outcome}'.`,
          retryable: result.outcome === 'conflict',
        })
  return {
    commandId: envelope.commandId,
    outcome: result.outcome,
    emittedEvents: mergeEvents(result.emittedEvents, emitted),
    aggregateId: result.aggregateId ?? null,
    message: result.message ?? null,
    replayed: result.replayed ?? false,
    value: result.value,
    error,
    receiptId: result.receiptId ?? (hasReceipt ? envelope.commandId : undefined),
  }
}

export class CommandDispatcherImpl implements CommandDispatcher {
  constructor(private readonly options: CommandDispatcherOptions) {}

  /** The configured durable event sink (null/absent before a consumer exists). */
  get eventSink(): OutboxEventRepository | null | undefined {
    return this.options.eventSink
  }

  async execute<TPayload extends Record<string, unknown>, TResult = unknown>(
    command: TypedCommandEnvelope<TPayload>,
  ): Promise<TypedCommandResult<TResult>> {
    // OPTIONAL observer-only trace recorder. Never allowed to break the command.
    const record = async (
      input: Parameters<NonNullable<CommandDispatcherOptions['traceRecorder']>>[0],
    ) => {
      try {
        await this.options.traceRecorder?.(input)
      } catch {
        /* contained: tracing must never affect the business operation */
      }
    }
    const nowIso = () => (this.options.now?.() ?? new Date()).toISOString()
    const startIso = nowIso()
    const base = {
      system: 'command',
      occurredAt: startIso,
      commandId: command.commandId,
      correlationId: command.correlationId,
      causationId: command.causationId,
      dealId: command.aggregateType === 'deal' ? command.aggregateId : null,
      workflowInstanceId: command.correlationId ?? null,
    }
    await record({ eventType: 'COMMAND_RECEIVED', ...base, summary: `Command ${command.commandType} received` })

    // 1. Resolve handler by commandType.
    const handler = this.options.registry.resolve(command.commandType)
    if (!handler) {
      await record({
        eventType: 'COMMAND_COMPLETED', ...base, occurredAt: nowIso(), outcome: 'not_found',
        summary: `Unknown command ${command.commandType}`,
      })
      return unknownCommandResult(command) as TypedCommandResult<TResult>
    }

    try {
      // 2-10. The application transaction (commit once at the end).
      const result = await this.options.run(async (tx) => {
        // 3-4. Replay fast-path: a terminal receipt replays without re-running.
        const existing = await this.options.receipts.find(command.commandId, tx)
        if (existing) {
          return this.replayResult(command, existing) as TypedCommandResult<TResult>
        }

        // 5-7. Execute the handler / canonical domain service.
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

        // 8. Outbox rows in the SAME transaction when the seam is enabled.
        const emitted = collector.drain()
        if (this.options.eventSink && emitted.length > 0) {
          await this.options.eventSink.append(emitted, tx)
        }

        // 10. Normalize (receiptId is honest only when a receipt row exists).
        const persisted = await this.options.receipts.find(command.commandId, tx)
        return normalizeResult(command, result, emitted, persisted !== null) as TypedCommandResult<TResult>
      })

      const endIso = nowIso()
      const durationMs = Math.max(0, new Date(endIso).getTime() - new Date(startIso).getTime())
      await record({
        eventType: result.replayed ? 'COMMAND_REPLAYED' : 'COMMAND_COMPLETED',
        ...base, occurredAt: endIso, completedAt: endIso, durationMs,
        outcome: result.outcome, summary: `Command ${command.commandType} ${result.outcome}`,
      })
      for (const e of result.emittedEvents) {
        await record({
          eventType: 'DOMAIN_EVENT_EMITTED', system: 'domain', occurredAt: e.occurredAt,
          domainEventId: e.eventId, commandId: e.causationId ?? null,
          correlationId: e.correlationId, causationId: e.causationId,
          dealId: e.aggregateType === 'deal' ? e.aggregateId : null,
          workflowInstanceId: e.correlationId ?? null,
          summary: `Domain event ${e.eventType}`,
        })
      }
      return result
    } catch (err) {
      const endIso = nowIso()
      await record({
        eventType: 'COMMAND_FAILED', ...base, occurredAt: endIso, completedAt: endIso, outcome: 'FAILURE',
        summary: `Command ${command.commandType} failed`,
        metadata: { error: err instanceof Error ? err.message.slice(0, 200) : String(err) },
      })
      throw err
    }
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
    return {
      commandId: command.commandId,
      outcome: decision.outcome,
      emittedEvents: [],
      aggregateId: receipt.aggregateId ?? null,
      message: decision.message,
      replayed: true,
      receiptId: command.commandId,
    }
  }
}
