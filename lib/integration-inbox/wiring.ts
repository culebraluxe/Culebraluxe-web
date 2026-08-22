// ---------------------------------------------------------------------------
// CRM-23 — Integration Inbox production wiring.
//
// CRM-23 acceptance criterion 7: "CRM changes happen through the canonical
// Business Command layer." This module is the PRODUCTION wiring that makes
// that true for the integration inbox: interaction persistence goes through
// the canonical `interaction.record` command (lib/commands), never by writing
// the interaction table directly. The processor stays transport-agnostic —
// tests inject an in-memory persistence; production uses this seam.
//
// The commandId is the stable source identity (`integration-inbox:<source
// system>:<external id>`), so the claim-first command receipt doubles as an
// idempotency backstop for replayed dispatches.
// ---------------------------------------------------------------------------

import { INTERACTION_RECORD } from '../commands/command-types'
import type { CommandDispatcher } from '../commands/contracts'
import type { CreateInteractionInput } from '../crm-types'
import type { MacSourceObserver, SourceCapability } from '../mac-observer/contracts'

/**
 * Build the persistence hook that executes interaction.record through the
 * canonical command dispatcher. A replayed command (already-committed receipt)
 * resolves the interactionId from the receipt's aggregateId — the receipt
 * never fabricates a duplicate interaction.
 */
export function createCommandSeamInteractionPersistence(
  dispatcher: CommandDispatcher,
): (input: CreateInteractionInput) => Promise<{ interactionId: string; created: boolean }> {
  return async (input) => {
    const sourceSystem = input.sourceSystem?.trim()
    const sourceExternalId = input.sourceExternalId?.trim()
    const commandId = `integration-inbox:${sourceSystem}:${sourceExternalId}`

    const result = await dispatcher.execute({
      commandId,
      commandType: INTERACTION_RECORD,
      actorAppUserId: null,
      aggregateType: 'interaction',
      aggregateId: null,
      correlationId: null,
      causationId: null,
      requestedAt: new Date().toISOString(),
      input: input as unknown as Record<string, unknown>,
    })

    if (result.outcome !== 'success') {
      throw new Error(
        `interaction.record failed (${result.outcome}): ${result.message ?? 'unknown error'}`,
      )
    }

    const value = result.value as { interactionId?: string; created?: boolean } | undefined
    const interactionId = value?.interactionId ?? result.aggregateId
    if (!interactionId) {
      throw new Error('interaction.record succeeded without an interaction id.')
    }
    return { interactionId, created: value?.created ?? false }
  }
}

/** Capability registry from a set of source observers (the honest gate). */
export function createMacCapabilities(
  observers: readonly MacSourceObserver[],
): Record<string, SourceCapability> {
  const capabilities: Record<string, SourceCapability> = {}
  for (const observer of observers) {
    capabilities[observer.source] = observer.capability
  }
  return capabilities
}
