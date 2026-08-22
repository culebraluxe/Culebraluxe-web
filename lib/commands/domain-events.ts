// ---------------------------------------------------------------------------
// CRM-14J — Domain event collector + factory.
//
// EVENT = FACT. Domain events represent committed facts ("X happened"). A
// command handler adds events to the execution context collector; the
// dispatcher drains them after the domain mutation succeeds and (when the
// outbox seam is enabled) appends them in the SAME transaction as the
// mutation + receipt (atomic commit; no event without its business truth).
//
// The correlation/causation chain (integration contract):
//   workflow instance id -> CommandEnvelope.correlationId
//   CommandEnvelope.commandId -> DomainEvent.causationId
//   DomainEvent.eventId -> future DomainEvent.causationId
// ---------------------------------------------------------------------------

import { randomUUID } from 'node:crypto'
import type {
  CommandEnvelope,
  DomainEvent,
  DomainEventCollector,
} from './contracts'

export class InMemoryDomainEventCollector implements DomainEventCollector {
  private events: DomainEvent[] = []

  add(event: DomainEvent): void {
    this.events.push(event)
  }

  drain(): DomainEvent[] {
    const drained = this.events
    this.events = []
    return drained
  }
}

export type CreateDomainEventInput = {
  eventType: DomainEvent['eventType']
  payload: Record<string, unknown>
  /** Explicit eventId (tests / deterministic producers); default random UUID. */
  eventId?: string
  /** Override for aggregate identity when the envelope cannot supply it. */
  aggregateType?: DomainEvent['aggregateType']
  aggregateId?: string
  /** Explicit causation (default: the envelope's commandId). */
  causationId?: string | null
}

/**
 * Build a DomainEvent from the command that produced it, wiring
 * correlationId (from the envelope) and causationId (defaults to the
 * envelope's commandId — the command caused the fact).
 */
export function createDomainEventFromCommand(
  envelope: CommandEnvelope,
  input: CreateDomainEventInput,
): DomainEvent {
  return {
    eventId: input.eventId ?? randomUUID(),
    eventType: input.eventType,
    occurredAt: new Date().toISOString(),
    actorAppUserId: envelope.actorAppUserId,
    aggregateType: input.aggregateType ?? envelope.aggregateType,
    aggregateId: input.aggregateId ?? envelope.aggregateId ?? '',
    correlationId: envelope.correlationId,
    causationId: input.causationId !== undefined
      ? input.causationId
      : envelope.commandId,
    payload: input.payload,
  }
}
