// ---------------------------------------------------------------------------
// CRM-14J — Canonical Business Command Layer contracts.
//
// COMMAND = INTENT ("please do X"); DOMAIN = TRUTH + RULES; RECEIPT = PROOF;
// EVENT = FACT; OUTBOX = DURABLE HANDOFF; SUBSCRIBER = REACTION.
//
// This module is the compile-ready contract surface of the canonical command
// seam. It REUSES the existing provider-neutral contracts
// (lib/workflow/contracts.ts — CommandEnvelope, CommandResult, DomainEvent,
// CommandOutcome) rather than duplicating them; the canonical layer adds the
// orchestration contracts (handler, registry, dispatcher, receipt repository,
// domain-event collector, execution context) on top.
//
// Vocabulary mapping to the architect-brief blueprint:
//   blueprint payload      -> envelope.input   (repo convention)
//   blueprint occurredAt   -> envelope.requestedAt
//   blueprint actor        -> envelope.actorAppUserId
//   blueprint status       -> CommandResult.outcome ('success' == Succeeded;
//                             every other terminal outcome == Failed)
//
// Naming/durability notes:
//   - The transaction handle IS the repo's interactive QueryExecutor; the
//     TxRunner that brackets dispatcher.execute() owns commit/rollback, which
//     is why ApplicationTransaction aliases QueryExecutor instead of exposing
//     commit()/rollback() methods.
//   - Commands NEVER host business rules. Legality, invariants and mutation
//     stay in the canonical domain services (db/*). Handlers are thin adapters
//     that translate the envelope into a canonical service call.
//   - The workflow engine never knows command internals; it only sends an
//     ApplicationCommandRequest and receives an outcome.
// ---------------------------------------------------------------------------

import type { QueryExecutor } from '../../db/query-executor'
import type { TxRunner } from '../../db/tx'
import type {
  CommandEnvelope,
  CommandResult,
  CommandOutcome,
  DomainEvent,
} from '../workflow/contracts'
import type {
  CommandEnvelope as WorkflowCommandEnvelope,
  CommandResult as WorkflowCommandResult,
} from '../workflow/contracts'

// Re-export the reused truth contracts so the canonical layer has one import
// surface (the types themselves are the existing ones, not copies):
//   CommandEnvelope — existing generic envelope (`input` is the blueprint's
//                     `payload`, `requestedAt` its `occurredAt`).
//   CommandResult   — existing normalized result, enriched additively with the
//                     blueprint's `value` / `error` / `receiptId`.
//   DomainEvent     — a committed fact ("X happened").
export type {
  CommandEnvelope,
  CommandResult,
  CommandOutcome,
  DomainEvent,
  AggregateType,
} from '../workflow/contracts'

/** Envelope typed over its payload (`input` narrowed). */
export type TypedCommandEnvelope<
  TPayload extends Record<string, unknown> = Record<string, unknown>,
> = WorkflowCommandEnvelope & { input: TPayload }

/** Result typed over its structured success payload. */
export type TypedCommandResult<
  TResult = unknown,
> = WorkflowCommandResult & { value?: TResult }

/** Actor / caller provenance for the envelope (blueprint `actor`). */
export type ActorContext = {
  /** Canonical application user id, when the actor is a logged-in user. */
  appUserId: string | null
  /** Free-form caller kind (api key, agent run, system, workflow, portal). */
  actorType?: string
  /** Free-form caller identity within actorType. */
  actorId?: string
}

/**
 * The transaction handle given to command handlers. In the Postgres V1
 * transport this is the interactive QueryExecutor (db/query-executor): the
 * tagged-template handle of the open transaction. Commit/rollback are owned by
 * the TxRunner that brackets dispatcher.execute() — the dispatcher commits
 * once after mutation + receipt + (future) outbox rows, and rolls back on any
 * thrown infrastructure failure. Do not commit/rollback from inside a handler.
 */
export type ApplicationTransaction = QueryExecutor

/** Blueprint status projection: Succeeded / Failed / Pending. */
export type CommandReceiptStatus = 'Succeeded' | 'Failed' | 'Pending'

/** Derive the blueprint status from the stored canonical outcome. */
export function commandReceiptStatus(
  outcome: CommandOutcome | 'pending',
): CommandReceiptStatus {
  if (outcome === 'pending') return 'Pending'
  return outcome === 'success' ? 'Succeeded' : 'Failed'
}

/**
 * Durable command receipt — proof, replay, idempotency. The current
 * `workflow_command_receipt` table stores command_id / outcome / aggregate_id /
 * message / created_at / actor_app_user_id (AUTH-05); the richer fields below
 * (commandType, correlationId, causationId, aggregateType, resultPayload,
 * errorCode, errorMessage) are compile-ready contract surface carried on the
 * canonical envelope today and persisted by the same receipt row when a
 * durable consumer needs them (additive migration — CRM-14I defer decision
 * preserved).
 */
export type CommandReceipt = {
  commandId: string
  /** Stored canonical outcome ('pending' is the claim sentinel, never terminal). */
  outcome: CommandOutcome | 'pending'
  /** Blueprint status projection of `outcome`. */
  status: CommandReceiptStatus
  aggregateId: string | null
  message: string | null
  createdAt: string | null
  /**
   * AUTH-05: acting app_user recorded on the receipt for allow-listed
   * consequential commands (offer.accept, deal.set_stage_*,
   * deal.set_financing_type, deal.set_closing_date). Null when the caller
   * supplied no actor (e.g. engine-driven commands).
   */
  actorAppUserId?: string | null
  commandType?: string
  correlationId?: string | null
  causationId?: string | null
  aggregateType?: string | null
  resultPayload?: unknown
  errorCode?: string | null
  errorMessage?: string | null
}

/** Read/persist command receipts inside a transaction (idempotent replay). */
export interface CommandReceiptRepository {
  /**
   * Read a receipt inside the transaction boundary (or under equivalent
   * serialization semantics). Returns null when the commandId is unknown.
   */
  find(
    commandId: string,
    tx: ApplicationTransaction,
  ): Promise<CommandReceipt | null>
  /**
   * Persist/update a receipt inside the SAME transaction as the business
   * mutation it proves. Must be idempotent (the stored row is keyed by
   * commandId).
   */
  save(receipt: CommandReceipt, tx: ApplicationTransaction): Promise<void>
  /**
   * Claim-first serialization: true only for the single winner of a commandId
   * (UNIQUE(command_id) INSERT ... ON CONFLICT DO NOTHING). Canonical domain
   * services use this to serialize concurrent same-commandId executions;
   * new-style handlers may use it directly.
   */
  claim(commandId: string, tx: ApplicationTransaction): Promise<boolean>
}

/** Collects domain events produced by a command execution (in-memory). */
export interface DomainEventCollector {
  add(event: DomainEvent): void
  drain(): DomainEvent[]
}

/**
 * Execution context handed to a command handler. `tx` and `run` are bound to
 * the dispatcher's open transaction so the handler's canonical service call
 * and any receipt/outbox writes all commit atomically with the mutation.
 */
export interface CommandExecutionContext {
  /** The open transaction (Postgres V1 transport). */
  tx: ApplicationTransaction
  /** Receipt repository for claim/finalize/replay (idempotency). */
  receipts: CommandReceiptRepository
  /** The command registry (resolve sibling commands if needed). */
  registry: CommandRegistry
  /** Collector for domain events this command emits (committed facts). */
  events: DomainEventCollector
  /**
   * TxRunner bound to the dispatcher's transaction. Canonical domain services
   * accept a TxRunner; pass this so their body runs in the SAME transaction as
   * the receipt and any outbox rows (commit once, rollback as one unit).
   */
  run: TxRunner
  /** Application clock (injectable for deterministic tests). */
  now(): Date
}

/**
 * Command handler — the thin adapter that translates a canonical envelope into
 * a canonical domain service call. Handlers carry NO business rules: legality,
 * invariants and mutation live in the domain services they call.
 */
export interface CommandHandler<
  C extends CommandEnvelope = CommandEnvelope,
  R = CommandResult,
> {
  handle(command: C, ctx: CommandExecutionContext): Promise<R>
}

/**
 * Business command object flavor (blueprint). `type` is the stable command
 * identifier and `payload` the intent; `execute` runs against a context whose
 * transaction is owned by the dispatcher. Prefer CommandHandler for new
 * commands; this shape is provided for contract completeness and can be
 * adapted with `fromBusinessCommand`.
 */
export interface BusinessCommand<
  TPayload extends Record<string, unknown> = Record<string, unknown>,
  TResult = unknown,
> {
  readonly type: string
  readonly payload: TPayload
  execute(ctx: CommandExecutionContext): Promise<TypedCommandResult<TResult>>
}

/** Adapt a BusinessCommand object into a CommandHandler (envelope payload). */
export function fromBusinessCommand<
  TPayload extends Record<string, unknown>,
  TResult,
>(
  command: BusinessCommand<TPayload, TResult>,
): CommandHandler<TypedCommandEnvelope<TPayload>, TypedCommandResult<TResult>> {
  return {
    async handle(envelope, ctx) {
      if (envelope.commandType !== command.type) {
        throw new Error(
          `BusinessCommand '${command.type}' cannot handle envelope type '${envelope.commandType}'.`,
        )
      }
      return command.execute(ctx)
    },
  }
}

/** Resolve a command handler by stable command type. */
export interface CommandRegistry {
  register(commandType: string, handler: CommandHandler<any, any>): void
  resolve(commandType: string): CommandHandler<any, any> | undefined
}

/** The canonical command seam: every caller (UI/API/agent/workflow) executes here. */
export interface CommandDispatcher {
  execute<TPayload extends Record<string, unknown>, TResult = unknown>(
    command: TypedCommandEnvelope<TPayload>,
  ): Promise<TypedCommandResult<TResult>>
}
