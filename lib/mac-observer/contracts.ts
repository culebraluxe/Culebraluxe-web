// ---------------------------------------------------------------------------
// CRM-23 — Mac Integration Observer: source-neutral contracts.
//
// THE MAC IS AN INTEGRATION EDGE, NOT THE CRM.
//
//   MacIntegrationObserver
//     -> source observers/adapters (ContactsObserver, CalendarObserver,
//        MailObserver, MessagesObserver, WhatsAppObserver, future observers)
//     -> ExternalActivityEvent   (source-neutral fact, this module)
//     -> durable Integration Inbox (lib/integration-inbox)
//     -> identity/contact resolver -> existing CRM intake stubs
//     -> canonical Business Command layer -> canonical CRM truth
//     -> (future) transactional outbox for downstream alerting (CRM-14J)
//
// OBSERVER RESPONSIBILITY (this layer): detect/acquire facts from the source,
// preserve source event identity/provenance, normalize transport metadata,
// and hand the neutral fact to the durable Integration Inbox. An observer
// NEVER decides that an email creates a task, a WhatsApp advances a deal, or
// a calendar change triggers a workflow — those decisions live in the
// mapper/domain layers (and, for workflow transitions, downstream
// subscribers).
//
// SECURITY/PRIVACY: least privilege per source; do not scrape or persist
// entire app databases merely because the Mac can see them; store only what
// CRM requires; content bodies are referenced (contentReference), never
// duplicated into canonical tables; raw/provenance references point at
// bounded, revocable artifacts; source access mechanisms are declared
// explicitly via SourceCapability and documented in docs/agent.
//
// ADAPTER BOUNDARY: supported Apple frameworks (Contacts, EventKit, Mail /
// Message) are preferred where available; sources with weaker or non-public
// APIs (iMessage content, WhatsApp) are declared UNSUPPORTED honestly — the
// complexity of proving such access stays BELOW the observer contract and is
// never faked above it.
// ---------------------------------------------------------------------------

import type { JsonObject } from '../crm-types'

// ---------------------------------------------------------------------------
// Source identity
// ---------------------------------------------------------------------------

/**
 * Canonical Mac integration sources. `(string & {})` keeps the union open so
 * future sources (Slack, LinkedIn, Apple Notes…) extend it without a contract
 * break; adapters still implement the same MacSourceObserver surface.
 */
export type ExternalActivitySource =
  | 'contacts'
  | 'calendar'
  | 'mail'
  | 'messages'
  | 'whatsapp'
  | (string & {})

// ---------------------------------------------------------------------------
// Honest source capability (criterion 8: unsupported / inaccessible source
// APIs are represented honestly; no fake semantics above the adapter).
// ---------------------------------------------------------------------------

export type SourceCapabilityStatus = 'available' | 'unproven' | 'unsupported'

export type SourceCapability = {
  status: SourceCapabilityStatus
  /**
   * Human-readable explanation of WHY the source is at this capability level
   * (e.g. "no public macOS API for iMessage message content without private
   * frameworks"). Never a fabricated promise of access.
   */
  reason: string
  /**
   * The concrete access mechanisms the source requires (TCC consent, EventKit,
   * Full Disk Access, provider API, …). Documented + revocable: the observer
   * never persists more than the CRM requires even when access exists.
   */
  requiredAccess: string[]
  /** Apple frameworks preferred where available (adapter boundary). */
  supportedAppleFrameworks: string[]
}

// ---------------------------------------------------------------------------
// External participant / identity (contact is the spine)
// ---------------------------------------------------------------------------

export type ExternalIdentityKind =
  | 'email'
  | 'phone'
  | 'contact'
  | 'apple_account'
  | 'whatsapp'
  | 'external'

/** One neutral participant/identity on the observed fact. */
export type ExternalIdentity = {
  kind: ExternalIdentityKind
  value: string
  displayName?: string
}

export type ExternalParticipantRole =
  | 'sender'
  | 'recipient'
  | 'organizer'
  | 'attendee'
  | 'subject'
  | 'contact'

/** A participant with its transport role on the observed fact. */
export type ExternalParticipant = ExternalIdentity & {
  role: ExternalParticipantRole
}

// ---------------------------------------------------------------------------
// Content / attachment / provenance (privacy-preserving references)
// ---------------------------------------------------------------------------

export type ExternalAttachment = {
  /** Stable provider-side reference to the attachment artifact. */
  referenceId: string
  filename?: string
  mimeType?: string
  sizeBytes?: number
}

/**
 * Content/body REFERENCE rather than uncontrolled duplication. The raw body
 * never crosses into canonical CRM tables; canonical CRM stores normalized
 * business data. A reference may point at a bounded, revocable artifact
 * (provider content id, local file reference) — retention is policy-driven.
 */
export type ExternalContent = {
  subject?: string
  summary?: string
  /** Reference to the raw body artifact — never the body itself. */
  contentReference?: string
}

/**
 * Raw/provenance reference for the observed fact. The canonical CRM never
 * stores the raw payload; `rawReference` points at the bounded artifact the
 * adapter lowered from (a provider webhook id, a local observation file), so
 * the fact's provenance can be audited without persisting raw sensitive data.
 */
export type ExternalProvenance = {
  /** Reference to the raw observation artifact (never the payload itself). */
  rawReference?: string
  /** The adapter that lowered the raw observation (e.g. 'contacts.v1'). */
  adapter: string
  adapterVersion: string
}

// ---------------------------------------------------------------------------
// ExternalActivityEvent — the source-neutral observed fact
// ---------------------------------------------------------------------------

/**
 * A source-neutral fact observed on the Mac. Every event carries stable source
 * identity (source + sourceAccount + externalEventId), occurredAt/observedAt,
 * source type, participant identities, thread/conversation reference where
 * available, provenance/raw reference, and correlation metadata — acceptance
 * criterion 2. No source-specific field may leak into CRM domain services:
 * adapters lower raw observations into this neutral shape and nothing else
 * crosses the observer boundary.
 */
export interface ExternalActivityEvent {
  schemaVersion: 1
  /** Canonical source (contacts | calendar | mail | messages | whatsapp | …). */
  source: ExternalActivitySource
  /** The macOS account namespace the fact was observed on (e.g. 'iCloud:acct'). */
  sourceAccount: string
  /** Stable source event identity (the inbox dedupe key component). */
  externalEventId: string
  /** Source-typed event kind, e.g. 'contact.updated', 'calendar.event_created'. */
  eventType: string
  /** When the fact happened at the source. */
  occurredAt: string
  /** When the Mac observer observed/acquired the fact. */
  observedAt: string
  /** Transport direction when the source expresses one. */
  direction?: 'inbound' | 'outbound'
  /** Participant identities with transport roles (sender/recipient/…). */
  participants: ExternalParticipant[]
  /** Candidate canonical contact identities (email/phone/contact refs). */
  contactCandidates?: ExternalIdentity[]
  /** Thread / conversation reference where available. */
  thread?: {
    /** Provider thread/conversation id. */
    id?: string
    /** Alternate conversation identifier (e.g. Messages conversation id). */
    conversationId?: string
    /** For threaded replies: the message/event this replies to. */
    inReplyTo?: string
  }
  /** Subject/summary + body reference (never raw body duplication). */
  content?: ExternalContent
  attachments?: ExternalAttachment[]
  /** Correlation metadata (join this fact to a business flow if applicable). */
  correlationId?: string
  /** Trusted business context ONLY when the source asserts it (exact match). */
  context?: {
    propertyId?: string
    propertySlug?: string
    propertyUrl?: string
    dealId?: string
  }
  provenance: ExternalProvenance
}

// ---------------------------------------------------------------------------
// Raw observation — what a macOS observer process hands to the adapter
// ---------------------------------------------------------------------------

/**
 * The raw transport fact as observed (JSON from the macOS observer process /
 * provider API). Adapters LOWER raw observations into ExternalActivityEvent;
 * raw payloads never cross into CRM.
 */
export type RawObservation = {
  source: ExternalActivitySource
  sourceAccount: string
  /** Raw source event identity (lowered to externalEventId). */
  rawEventId: string
  observedAt: string
  /** Raw payload — kept out of canonical tables by contract. */
  payload: JsonObject
}

// ---------------------------------------------------------------------------
// MacSourceObserver — the acquisition-only observer contract
// ---------------------------------------------------------------------------

/**
 * One source observer. ACQUISITION ONLY: `observe()` returns raw observations
 * for lowering; it never maps to CRM intents, never creates tasks/deals, and
 * never triggers workflows. The adapter boundary keeps all source-specific
 * complexity below this contract.
 */
export interface MacSourceObserver {
  readonly source: ExternalActivitySource
  /** The macOS account namespace this observer watches. */
  readonly accountNamespace: string
  /** Honest capability declaration (criterion 8). */
  readonly capability: SourceCapability
  /**
   * Acquire raw observations since the last acquisition. Sources with
   * capability.status !== 'available' return [] — never fabricated facts.
   * Implementations that need a cursor own it below the contract.
   */
  observe(): Promise<RawObservation[]>
}

/** Lower one raw observation into the neutral ExternalActivityEvent. */
export type RawObservationLowerer = (
  raw: RawObservation,
) => ExternalActivityEvent
