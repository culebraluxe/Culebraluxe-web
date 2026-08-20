// Framework integration seam between the generic workflow engine and the
// CulebraLuxe application contract surface.
//
// This file contains NO brokerage policy and NO runtime domain imports — only
// type mappings and pure transform functions. The engine never imports this
// file; a future CulebraLuxe adapter constructs an `ApplicationPort` from these
// mappings plus the application facade in lib/workflow/adapter.ts.

import type {
  ApplicationPort,
  ApplicationCommandRequest,
  ApplicationCommandResult,
  ApplicationFacts,
} from '../workflow_engine/lib/workflow/types';
import type {
  CommandEnvelope,
  CommandResult,
  CommandOutcome,
} from '../lib/workflow/contracts';

/** Engine command outcome ⇄ application command outcome (already aligned). */
export function toCommandOutcome(
  outcome: ApplicationCommandResult['outcome'],
): CommandOutcome {
  switch (outcome) {
    case 'success':
      return 'success';
    case 'validation_failure':
      return 'validation_failure';
    case 'not_found':
      return 'not_found';
    case 'conflict':
      return 'conflict';
    case 'unauthorized':
      return 'unauthorized';
    case 'precondition_failure':
      return 'precondition_failure';
  }
}

/** Map an engine command request onto the application command envelope. */
export function toCommandEnvelope(
  req: ApplicationCommandRequest,
  actorAppUserId: string | null,
): CommandEnvelope {
  return {
    commandId: req.commandId,
    commandType: req.commandType,
    actorAppUserId,
    aggregateType: (req.subjectType as CommandEnvelope['aggregateType']) ?? 'deal',
    aggregateId: req.subjectId,
    correlationId: req.correlationId,
    causationId: req.causationId,
    requestedAt: new Date().toISOString(),
    input: req.input,
  };
}

/** Map an application command result back onto the engine command result. */
export function toApplicationCommandResult(
  result: CommandResult,
  commandId: string,
): ApplicationCommandResult {
  return {
    commandId,
    outcome: result.outcome,
    message: result.message,
    emittedEvents: result.emittedEvents.map((e) => ({
      eventId: e.eventId,
      eventType: e.eventType,
      aggregateType: e.aggregateType,
      aggregateId: e.aggregateId,
    })),
  };
}

/**
 * Contract every CulebraLuxe `ApplicationPort` implementation must satisfy:
 * execute the application command (authority + domain validation stay inside
 * the application) and read compact, deterministic subject facts.
 */
export type CulebraLuxeApplicationPort = ApplicationPort & {
  readFacts(subject: {
    subjectType: string;
    subjectId: string;
  }): Promise<ApplicationFacts>;
};
