import { sql } from '../../db/client'
import { createIntegrationInboxDurability } from '../../db/integration-inbox'
import {
  findDealById,
  findPropertyById,
  findPropertyBySlug,
} from '../../db/intake-context'
import {
  getInteractionBySourceIdentity,
} from '../../db/interactions'
import {
  createPersonWithIdentities,
  findIdentityMatch,
  findIdentityOwnership,
  personExists,
} from '../../db/person-identities'
import { commandDispatcher } from '../commands'
import type { SourceCapability } from '../mac-observer/contracts'
import {
  DEFAULT_INTEGRATION_INBOX_CONFIGURATION,
  processExternalActivityEvent,
  type MacChannelConfigurations,
  type MacIntakeRepositories,
} from '../integration-inbox/processor'
import type { IntegrationInboxProcessingOutcome } from '../integration-inbox/contracts'
import { createCommandSeamInteractionPersistence } from '../integration-inbox/wiring'

import type { MetaWhatsAppConfiguration, MetaWhatsAppWebhookPayload } from './types'
import { parseMetaWhatsAppWebhook } from './parse'

const META_WHATSAPP_CAPABILITY: SourceCapability = {
  status: 'available',
  reason:
    'Authenticated Meta WhatsApp Cloud API webhook with stable provider message ids.',
  requiredAccess: ['Meta WhatsApp Cloud API webhook'],
  supportedAppleFrameworks: [],
}

const repositories: MacIntakeRepositories = {
  findInteractionBySourceIdentity: getInteractionBySourceIdentity,
  personExists,
  findIdentityMatch,
  findIdentityOwnership,
  createPersonWithIdentities,
  findPropertyById,
  findPropertyBySlug,
  findDealById,
}

const persistInteraction = createCommandSeamInteractionPersistence(
  commandDispatcher,
)
const durability = createIntegrationInboxDurability(sql, persistInteraction)

function channels(config: MetaWhatsAppConfiguration): MacChannelConfigurations {
  return {
    calendar: { ownedCalendarEmails: [] },
    mail: { internalMailboxes: [] },
    messages: { ownedLines: [] },
    whatsapp: {
      provider: 'meta',
      ownedLines: [{ phone: config.ownedPhoneE164 }],
    },
  }
}

export type ProcessMetaWhatsAppWebhookResult = {
  eventCount: number
  outcomes: IntegrationInboxProcessingOutcome[]
  retryableFailure: boolean
}

/**
 * Provider edge -> canonical realtime envelope -> durable integration inbox ->
 * existing WhatsApp intake -> canonical interaction.record command.
 */
export async function processMetaWhatsAppWebhook(input: {
  payload: MetaWhatsAppWebhookPayload
  config: MetaWhatsAppConfiguration
  observedAt?: string
}): Promise<ProcessMetaWhatsAppWebhookResult> {
  const events = parseMetaWhatsAppWebhook({
    payload: input.payload,
    phoneNumberId: input.config.phoneNumberId,
    ownedPhoneE164: input.config.ownedPhoneE164,
    observedAt: input.observedAt,
  })
  const outcomes: IntegrationInboxProcessingOutcome[] = []

  for (const event of events) {
    // Sequential processing preserves webhook order. Replay is safe at both
    // the inbox source key and canonical interaction source key.
    outcomes.push(await processExternalActivityEvent({
      event,
      configuration: {
        ...DEFAULT_INTEGRATION_INBOX_CONFIGURATION,
        capabilities: { whatsapp: META_WHATSAPP_CAPABILITY },
      },
      repositories,
      durability,
      channels: channels(input.config),
    }))
  }

  return {
    eventCount: events.length,
    outcomes,
    retryableFailure: outcomes.some(
      (outcome) => outcome.outcome === 'failed_retryable',
    ),
  }
}
