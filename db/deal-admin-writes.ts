import { randomUUID } from 'node:crypto'

import { PortalWriteError } from '../lib/portal-write-error'
import {
  normalizeDealCreateInput,
  normalizeStructuralParticipantInput,
  type DealCreateInput,
  type StructuralParticipantInput,
} from '../lib/deal-admin'
import { neonTx, type TxRunner } from './tx'
import type { QueryExecutor, QueryRow } from './query-executor'

// ---------------------------------------------------------------------------
// OPS-05 — Deal / Participant Administration write seam.
//
// Completes the deal write surface: the read projections (db/deals.ts
// portfolio, db/deal-workspace.ts workspace) and the long-tail participant
// writes (db/deal-participants.ts addOtherParticipant / endParticipant /
// updateParticipantRoleLabel) already exist; this module owns deal creation
// and the STRUCTURAL participant lifecycle (client / owner / seller).
//
// Participant-model decision (migration 034): deal_participant is THE
// canonical participant model with at most ONE active structural participant
// per role per deal (partial unique index uq_deal_participant_active_
// structural_role). The legacy per-deal FKs (deal.client_person_id,
// deal.owner_user_id) remain as mirrors that read projections no longer use —
// but because deal.client_person_id is NOT NULL and both live on the deal
// row, this seam keeps them consistent with the canonical participants:
//
//   - createDeal inserts the deal AND its client/owner participant rows in
//     ONE transaction, so the canonical model is correct from birth.
//   - setStructuralParticipant ends any active same-role row, inserts the new
//     active row, and syncs the per-deal mirror (client → deal.client_person_id,
//     owner → deal.owner_user_id). The DB unique index remains the final
//     backstop; a 23505 race maps to a clean conflict.
//   - endStructuralParticipant ends an owner/seller row and clears the
//     per-deal owner mirror (guarded to the ended subject). A CLIENT can never
//     be ended — deal.client_person_id is NOT NULL, so the client is replaced,
//     never removed.
//   - property.seller_person_id is NOT synced: it is a property-domain fact
//     (property-scoped, shared across deals), outside deal-participant
//     maintenance.
//
// Each write runs inside ONE injected transaction (neonTx in production, a
// fake in tests) so a failed write can never leave partial state. No audit
// rows here: durable actor/action records for deal administration verbs are
// the AUTH-05 allow-list's responsibility and deliberately not extended in
// this story.
// ---------------------------------------------------------------------------

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

function isUuid(value: string): boolean {
  return UUID_PATTERN.test(value)
}

function isUniqueViolation(error: unknown) {
  return (
    typeof error === 'object' &&
    error !== null &&
    'code' in error &&
    error.code === '23505'
  )
}

async function dealExists(tx: QueryExecutor, dealId: string): Promise<boolean> {
  const rows = await tx`
    select id from deal where id = ${dealId} limit 1
  `
  return rows.length > 0
}

// ---------------------------------------------------------------------------
// Create
// ---------------------------------------------------------------------------

export async function createDeal(
  input: DealCreateInput,
  run: TxRunner = neonTx,
): Promise<{ id: string }> {
  const deal = normalizeDealCreateInput(input)
  const id = randomUUID()

  await run(async (tx) => {
    const propertyRows = (await tx`
      select id, archived_at from property
      where id = ${deal.propertyId}
      limit 1
    `) as QueryRow[]
    if (propertyRows.length === 0) {
      throw new PortalWriteError('not-found', 'Property not found.')
    }
    if (propertyRows[0].archived_at !== null) {
      throw new PortalWriteError(
        'conflict',
        'Archived properties cannot start a deal.',
      )
    }

    const personRows = (await tx`
      select id, archived_at from person
      where id = ${deal.clientPersonId}
      limit 1
    `) as QueryRow[]
    if (personRows.length === 0) {
      throw new PortalWriteError('not-found', 'Client person not found.')
    }
    if (personRows[0].archived_at !== null) {
      throw new PortalWriteError(
        'conflict',
        'Archived clients cannot start a deal.',
      )
    }

    if (deal.ownerUserId) {
      const userRows = (await tx`
        select id, active from app_user
        where id = ${deal.ownerUserId}
        limit 1
      `) as QueryRow[]
      if (userRows.length === 0) {
        throw new PortalWriteError('not-found', 'Owner user not found.')
      }
      if (userRows[0].active !== true) {
        throw new PortalWriteError(
          'conflict',
          'Inactive users cannot own a deal.',
        )
      }
    }

    await tx`
      insert into deal (id, property_id, client_person_id, owner_user_id, notes)
      values (
        ${id}, ${deal.propertyId}, ${deal.clientPersonId},
        ${deal.ownerUserId}, ${deal.notes}
      )
    `

    // Participant-model decision: the canonical participants exist from birth,
    // in the same transaction as the deal row.
    await tx`
      insert into deal_participant (deal_id, person_id, role, active)
      values (${id}, ${deal.clientPersonId}, 'client', true)
    `
    if (deal.ownerUserId) {
      await tx`
        insert into deal_participant (deal_id, user_id, role, active)
        values (${id}, ${deal.ownerUserId}, 'owner', true)
      `
    }
  })

  return { id }
}

// ---------------------------------------------------------------------------
// Set / replace a structural participant (client | owner | seller)
// ---------------------------------------------------------------------------

export async function setStructuralParticipant(
  input: StructuralParticipantInput,
  run: TxRunner = neonTx,
): Promise<{ participantId: string }> {
  const participant = normalizeStructuralParticipantInput(input)
  let createdId: string | null = null

  await run(async (tx) => {
    if (!(await dealExists(tx, participant.dealId))) {
      throw new PortalWriteError('not-found', 'Deal not found.')
    }

    if (participant.kind === 'person') {
      const personRows = (await tx`
        select id, archived_at from person
        where id = ${participant.personId}
        limit 1
      `) as QueryRow[]
      if (personRows.length === 0) {
        throw new PortalWriteError('not-found', 'Person not found.')
      }
      if (personRows[0].archived_at !== null) {
        throw new PortalWriteError(
          'conflict',
          'Archived people cannot be deal participants.',
        )
      }
    } else {
      const userRows = (await tx`
        select id, active from app_user
        where id = ${participant.userId}
        limit 1
      `) as QueryRow[]
      if (userRows.length === 0) {
        throw new PortalWriteError('not-found', 'User not found.')
      }
      if (userRows[0].active !== true) {
        throw new PortalWriteError(
          'conflict',
          'Inactive users cannot be deal participants.',
        )
      }
    }

    // Invariant (migration 034): one active structural participant per role
    // per deal. Ending the current row first keeps the transition legal.
    await tx`
      update deal_participant
      set active = false, ended_at = now(), updated_at = now()
      where deal_id = ${participant.dealId}
        and role = ${participant.role}
        and active = true
    `

    let participantId: string | null = null
    try {
      const rows = await tx`
        insert into deal_participant (deal_id, person_id, user_id, role, active)
        values (
          ${participant.dealId}, ${participant.personId}, ${participant.userId},
          ${participant.role}, true
        )
        returning id
      `
      participantId = (rows[0] as { id?: string } | undefined)?.id ?? null
    } catch (error) {
      if (isUniqueViolation(error)) {
        throw new PortalWriteError(
          'conflict',
          'A concurrent update changed this participant; refresh and retry.',
        )
      }
      throw error
    }
    if (!participantId) {
      throw new Error('Participant could not be created.')
    }
    createdId = participantId

    // Per-deal legacy FK mirrors (deal-level facts, kept consistent with the
    // canonical participant). property.seller_person_id is never touched here.
    if (participant.role === 'client') {
      await tx`
        update deal
        set client_person_id = ${participant.personId}, updated_at = now()
        where id = ${participant.dealId}
      `
    } else if (participant.role === 'owner') {
      await tx`
        update deal
        set owner_user_id = ${participant.userId}, updated_at = now()
        where id = ${participant.dealId}
      `
    }
  })

  // createdId is always set when the transaction completes (the insert throws
  // on failure); keep the type honest for the return contract.
  if (!createdId) throw new Error('Participant could not be created.')
  return { participantId: createdId }
}

// ---------------------------------------------------------------------------
// End a structural participant (owner | seller). The client is never ended —
// deal.client_person_id is NOT NULL, so a client is replaced, not removed.
// ---------------------------------------------------------------------------

export async function endStructuralParticipant(
  participantId: string,
  run: TxRunner = neonTx,
): Promise<{ participantId: string }> {
  if (!isUuid(participantId)) {
    throw new PortalWriteError('validation', 'Invalid participant identifier.')
  }

  await run(async (tx) => {
    const rows = (await tx`
      select id, deal_id, role, person_id, user_id
      from deal_participant
      where id = ${participantId}
        and active = true
      limit 1
    `) as QueryRow[]
    const row = rows[0] as
      | {
          deal_id?: string
          role?: string
          person_id?: string | null
          user_id?: string | null
        }
      | undefined
    if (!row?.deal_id) {
      throw new PortalWriteError('not-found', 'Participant not found.')
    }
    if (row.role === 'client') {
      throw new PortalWriteError(
        'conflict',
        'A deal must keep a client — replace the client instead of ending it.',
      )
    }
    if (row.role === 'other') {
      throw new PortalWriteError(
        'conflict',
        'Long-tail participants are managed from the participant row actions.',
      )
    }

    await tx`
      update deal_participant
      set active = false, ended_at = now(), updated_at = now()
      where id = ${participantId}
        and active = true
    `

    if (row.role === 'owner') {
      // Guarded to the ended subject so a newer owner is never clobbered.
      await tx`
        update deal
        set owner_user_id = null, updated_at = now()
        where id = ${row.deal_id}
          and owner_user_id = ${row.user_id ?? null}
      `
    }
  })

  return { participantId }
}
