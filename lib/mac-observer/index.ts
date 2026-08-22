// ---------------------------------------------------------------------------
// CRM-23 — MacIntegrationObserver: the Mac integration edge.
//
//   MacIntegrationObserver
//     -> source observers/adapters (ContactsObserver, CalendarObserver,
//        MailObserver, MessagesObserver, WhatsAppObserver, future observers)
//     -> ExternalActivityEvent            (this module lowers raw -> neutral)
//     -> durable Integration Inbox        (lib/integration-inbox/processor.ts)
//     -> identity/contact resolver -> existing CRM intake stubs
//     -> canonical Business Command layer -> canonical CRM truth
//     -> (future) transactional outbox    (CRM-14J, downstream alerting)
//
// OBSERVER RESPONSIBILITY — ACQUISITION ONLY. This orchestrator acquires
// facts from its source observers, lowers raw observations into the neutral
// ExternalActivityEvent (preserving source identity/provenance), and hands
// them to the durable inbox. It NEVER decides that an email creates a task, a
// WhatsApp advances a deal, or a calendar change triggers a workflow. The
// inbox/mapper/domain layers own those decisions.
//
// Honest capability filtering: only observers with capability.status ===
// 'available' contribute observations. 'unproven' and 'unsupported' sources
// (mail until access is proven; messages/whatsapp — no public API) are
// represented honestly and contribute NOTHING; the processor additionally
// refuses non-'available' sources (defense in depth).
// ---------------------------------------------------------------------------

import type {
  ExternalActivityEvent,
  ExternalActivitySource,
  MacSourceObserver,
  RawObservation,
} from './contracts'
import { lowerContactsObservation } from './adapters/contacts-adapter'
import { lowerCalendarObservation } from './adapters/calendar-adapter'
import { lowerMailObservation } from './adapters/mail-adapter'
import { lowerMessagesObservation } from './adapters/messages-adapter'
import { lowerWhatsAppObservation } from './adapters/whatsapp-adapter'
import type { RawObservationLowerer } from './contracts'

/** The adapter lowerer registry — one neutral lowerer per source. */
export const RAW_OBSERVATION_LOWERERS: Record<
  ExternalActivitySource,
  RawObservationLowerer
> = {
  contacts: lowerContactsObservation,
  calendar: lowerCalendarObservation,
  mail: lowerMailObservation,
  messages: lowerMessagesObservation,
  whatsapp: lowerWhatsAppObservation,
}

/** Lower one raw observation into the neutral ExternalActivityEvent. */
export function lowerRawObservation(raw: RawObservation): ExternalActivityEvent {
  const lowerer = RAW_OBSERVATION_LOWERERS[raw.source]
  if (!lowerer) {
    throw new Error(`No adapter for source '${raw.source}'.`)
  }
  return lowerer(raw)
}

/**
 * The Mac integration observer: owns source observers, acquires facts, and
 * lowers them into neutral ExternalActivityEvents. Acquisition only — no
 * business decisions here.
 */
export class MacIntegrationObserver {
  constructor(private readonly observers: MacSourceObserver[]) {}

  /** The observers this edge watches (for introspection/registry). */
  get sourceObservers(): readonly MacSourceObserver[] {
    return this.observers
  }

  /**
   * Acquire + lower observations from every observer whose capability is
   * 'available'. Non-'available' observers contribute nothing — their facts
   * are never fabricated (criterion 8).
   */
  async acquire(): Promise<ExternalActivityEvent[]> {
    const events: ExternalActivityEvent[] = []
    for (const observer of this.observers) {
      if (observer.capability.status !== 'available') continue
      // eslint-disable-next-line no-await-in-loop
      const raw = await observer.observe()
      for (const observation of raw) {
        events.push(lowerRawObservation(observation))
      }
    }
    return events
  }
}

export type { ExternalActivityEvent, MacSourceObserver, RawObservation } from './contracts'
export type { SourceCapability, SourceCapabilityStatus } from './contracts'
export { contactsCapability, createContactsObserver } from './adapters/contacts-adapter'
export { calendarCapability, createCalendarObserver } from './adapters/calendar-adapter'
export { mailCapability, createMailObserver } from './adapters/mail-adapter'
export { messagesCapability, createMessagesObserver } from './adapters/messages-adapter'
export { whatsappCapability, createWhatsAppObserver } from './adapters/whatsapp-adapter'
export { createFakeMacObserver, contactsFixture, calendarFixture, mailFixture } from './fake-observer'
