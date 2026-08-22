// ---------------------------------------------------------------------------
// INTAKE-01 — Batch lane adapter.
//
// The batch acquisition lane (CSV/JSON/API-dump imports) lowers each import
// row into the SAME canonical intake message as the realtime lane. Batch and
// realtime differ only at the edge: this module is the batch edge. The
// durable inbox, identity resolution and Business Command layer downstream are
// identical for both lanes (see lib/intake/inbox.ts — one projection).
//
// The manifest owns the batch/import identity and the adapter provenance;
// each item owns its stable item id and the neutral facts. The item's
// sourcePayload is bounded (≤ 32 KB, validated by the contract) and opaque —
// downstream never parses it.
// ---------------------------------------------------------------------------

import type { JsonObject } from '../crm-types'
import type {
  CanonicalIntakeMessage,
  IntakeAttachment,
  IntakeContent,
  IntakeContext,
  IntakeIdentity,
  IntakeParticipant,
  IntakeThread,
} from './contracts'

/**
 * One batch import: stable import id (replay-safe), the canonical source
 * system the rows belong to, and the adapter provenance that owns raw-source
 * translation.
 */
export type IntakeBatchManifest = {
  /** Stable batch/import id. Re-running an import with the SAME id (or any id
   *  — batch id is provenance, never identity) replays instead of duplicating. */
  importId: string
  /** Canonical source system, e.g. 'import:csv:contacts-v1'. */
  sourceSystem: string
  /** The lowering adapter that owns raw-source translation. */
  adapter: string
  adapterVersion: string
  /** When the import was executed (ISO UTC). */
  importedAt: string
}

/** One normalized batch row, ready to lower into the canonical envelope. */
export type IntakeBatchItemInput = {
  /** Stable item id within the import's source system (dedupe axis 2). */
  itemId: string
  /** Source-typed event kind, e.g. 'contact.imported', 'interaction.imported'. */
  eventType: string
  /** When the fact happened at the source (ISO UTC). */
  occurredAt: string
  direction?: 'inbound' | 'outbound'
  participants?: IntakeParticipant[]
  contactCandidates?: IntakeIdentity[]
  thread?: IntakeThread
  content?: IntakeContent
  attachments?: IntakeAttachment[]
  context?: IntakeContext
  correlationId?: string
  /** Causation metadata (the prior fact/command this import row answers). */
  causationId?: string
  /** Reference to the raw import artifact (row/file), never the payload. */
  rawReference?: string
  /** Bounded, opaque import-row extras (validated ≤ 32 KB). */
  sourcePayload?: JsonObject
}

/**
 * Lower one batch import row into the canonical intake message. Batch items
 * of one import share the manifest's importId as their correlation id when
 * the row carries none, so one import's durable inbox rows are queryable
 * together — the batch id is correlation/provenance, never identity.
 */
export function lowerBatchItemToIntakeMessage(
  manifest: IntakeBatchManifest,
  item: IntakeBatchItemInput,
): CanonicalIntakeMessage {
  return {
    schemaVersion: 1,
    acquisitionLane: 'batch',
    source: {
      system: manifest.sourceSystem,
      itemId: item.itemId,
      batchId: manifest.importId,
    },
    eventType: item.eventType,
    occurredAt: item.occurredAt,
    observedAt: manifest.importedAt,
    direction: item.direction,
    participants: item.participants ?? [],
    contactCandidates: item.contactCandidates,
    thread: item.thread,
    content: item.content,
    attachments: item.attachments,
    context: item.context,
    correlationId: item.correlationId ?? manifest.importId,
    causationId: item.causationId,
    provenance: {
      adapter: manifest.adapter,
      adapterVersion: manifest.adapterVersion,
      rawReference: item.rawReference,
    },
    sourcePayload: item.sourcePayload,
  }
}
