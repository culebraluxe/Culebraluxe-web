import { randomUUID } from 'node:crypto'

import { PortalWriteError } from '../lib/portal-write-error'
import {
  normalizeClientContact,
  normalizeClientCreateInput,
  normalizeClientProfileUpdate,
  type ClientCreateInput,
  type ClientIdentityKind,
  type ClientProfileFields,
} from '../lib/person-admin'
import { findIdentityOwnership } from './person-identities'
import { sql } from './client'
import { neonTx, type TxRunner } from './tx'
import type { QueryExecutor } from './query-executor'

// ---------------------------------------------------------------------------
// OPS-02 — Client Administration: canonical person (client) write seam.
//
// Create / update / archive / contact-identity for the Client Administration
// surface. Reads stay in db/clients.ts + db/client-admin.ts (unchanged). All
// writes are soft: archive sets archived_at; every read projection already
// filters archived_at is null, so an archived client drops out of the manager,
// the admin table, the dossier, and person search without any hard delete.
//
// Boundaries (reuse-first):
//   - Identity ownership rules come from the canonical CRM seam
//     (findIdentityOwnership): a value already owned by ANOTHER person is
//     never silently reassigned — the operator must resolve the conflict.
//   - Input normalization/validation is the shared pure contract in
//     lib/person-admin.ts (role/status vocabularies, canonical email / E.164
//     phone). Both the action layer and this seam run it, so a bypassed
//     action can never write a malformed row.
//   - The DB role/status CHECK constraints and person_identity_unique remain
//     the final backstops (a 23505 race maps to a clear conflict).
//   - Multi-statement writes run inside ONE injected transaction (neonTx in
//     production, a fake in tests), so a failed identity insert can never
//     orphan a created person or leave half-updated state.
//   - No audit rows here: durable actor/action records for client
//     administration verbs are the AUTH-05 allow-list's responsibility and
//     deliberately NOT extended in this story.
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

type PersonRow = {
  id: string
  display_name: string
  role: string
  status: string
  location: string | null
  budget_min: string | null
  budget_max: string | null
  preferred_areas: string[] | null
  property_types: string[] | null
  priorities: string[] | null
  timeline: string | null
  notes: string | null
  assigned_user_id: string | null
  archived_at: string | null
}

async function personRow(
  tx: QueryExecutor,
  personId: string,
): Promise<PersonRow | null> {
  const rows = await tx`
    select id, display_name, role, status, location, budget_min, budget_max,
      preferred_areas, property_types, priorities, timeline, notes,
      assigned_user_id, archived_at
    from person
    where id = ${personId}
    limit 1
  `
  return (rows[0] as PersonRow | undefined) ?? null
}

async function personDisplayName(tx: QueryExecutor, personId: string) {
  const row = await personRow(tx, personId)
  return row?.display_name ?? 'another person'
}

async function requireActivePerson(tx: QueryExecutor, personId: string) {
  const row = await personRow(tx, personId)
  if (!row || row.archived_at !== null) {
    throw new PortalWriteError('not-found', 'Person not found.')
  }
  return row
}

async function assertAgentExists(tx: QueryExecutor, userId: string) {
  const rows = await tx`
    select id from app_user
    where id = ${userId} and active = true
    limit 1
  `
  if (rows.length === 0) {
    throw new PortalWriteError(
      'validation',
      'Assigned agent is not an active user.',
    )
  }
}

// A canonical identity value may be owned by at most one person. When
// ownerPersonId is null (create) ANY owner blocks; otherwise only a
// different owner blocks — never silently reassign.
async function assertIdentityNotOwnedElsewhere(
  tx: QueryExecutor,
  kind: ClientIdentityKind,
  value: string,
  ownerPersonId: string | null,
) {
  const ownership = await findIdentityOwnership(
    { kind, value, evidence: 'user_supplied', normalizedValue: value },
    tx,
  )
  if (!ownership) return
  if (ownerPersonId !== null && ownership.personId === ownerPersonId) return
  const ownerLabel = await personDisplayName(tx, ownership.personId)
  throw new PortalWriteError(
    'conflict',
    `This ${kind} already belongs to ${ownerLabel}.`,
  )
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

export type AssignableAgent = {
  id: string
  displayName: string
}

/** Active app users an operator may assign as a client's agent. */
export async function listAssignableAgents(
  execute: QueryExecutor = sql,
): Promise<AssignableAgent[]> {
  const rows = await execute`
    select id, display_name
    from app_user
    where active = true
    order by display_name asc
  `
  return (rows as Array<{ id: string; display_name: string }>).map((row) => ({
    id: row.id,
    displayName: row.display_name,
  }))
}

// ---------------------------------------------------------------------------
// Create
// ---------------------------------------------------------------------------

export async function createClient(
  input: ClientCreateInput,
  run: TxRunner = neonTx,
): Promise<{ personId: string }> {
  const profile = normalizeClientCreateInput(input)
  const personId = randomUUID()

  await run(async (tx) => {
    if (profile.email) {
      await assertIdentityNotOwnedElsewhere(tx, 'email', profile.email, null)
    }
    if (profile.phone) {
      await assertIdentityNotOwnedElsewhere(tx, 'phone', profile.phone, null)
    }
    if (profile.assignedUserId) {
      await assertAgentExists(tx, profile.assignedUserId)
    }

    await tx`
      insert into person (
        id, display_name, role, status, location, budget_min, budget_max,
        preferred_areas, property_types, priorities, timeline, notes,
        assigned_user_id
      ) values (
        ${personId}, ${profile.displayName}, ${profile.role}, ${profile.status},
        ${profile.location}, ${profile.budgetMin}, ${profile.budgetMax},
        ${profile.preferredAreas}, ${profile.propertyTypes},
        ${profile.priorities}, ${profile.timeline}, ${profile.notes},
        ${profile.assignedUserId}
      )
    `

    if (profile.email) {
      await tx`
        insert into person_identity (
          person_id, identity_type, identity_value, is_primary
        ) values (
          ${personId}, 'email', ${profile.email}, true
        )
      `
    }
    if (profile.phone) {
      await tx`
        insert into person_identity (
          person_id, identity_type, identity_value, is_primary
        ) values (
          ${personId}, 'phone', ${profile.phone}, true
        )
      `
    }
  })

  return { personId }
}

// ---------------------------------------------------------------------------
// Update
// ---------------------------------------------------------------------------

export async function updateClientProfile(
  personId: string,
  input: ClientProfileFields,
  run: TxRunner = neonTx,
): Promise<{ personId: string }> {
  if (!isUuid(personId)) {
    throw new PortalWriteError('validation', 'Invalid person identifier.')
  }
  const update = normalizeClientProfileUpdate(input)

  await run(async (tx) => {
    const current = await requireActivePerson(tx, personId)

    if (update.assignedUserId !== undefined && update.assignedUserId !== null) {
      await assertAgentExists(tx, update.assignedUserId)
    }

    // Merge: undefined = keep the current value, null = clear.
    const next = {
      display_name:
        update.displayName !== undefined
          ? update.displayName
          : current.display_name,
      role: update.role !== undefined ? update.role : current.role,
      status: update.status !== undefined ? update.status : current.status,
      location:
        update.location !== undefined ? update.location : current.location,
      budget_min:
        update.budgetMin !== undefined
          ? update.budgetMin
          : current.budget_min,
      budget_max:
        update.budgetMax !== undefined
          ? update.budgetMax
          : current.budget_max,
      preferred_areas:
        update.preferredAreas !== undefined
          ? update.preferredAreas
          : current.preferred_areas,
      property_types:
        update.propertyTypes !== undefined
          ? update.propertyTypes
          : current.property_types,
      priorities:
        update.priorities !== undefined
          ? update.priorities
          : current.priorities,
      timeline:
        update.timeline !== undefined ? update.timeline : current.timeline,
      notes: update.notes !== undefined ? update.notes : current.notes,
      assigned_user_id:
        update.assignedUserId !== undefined
          ? update.assignedUserId
          : current.assigned_user_id,
    }

    await tx`
      update person
      set display_name = ${next.display_name},
          role = ${next.role},
          status = ${next.status},
          location = ${next.location},
          budget_min = ${next.budget_min},
          budget_max = ${next.budget_max},
          preferred_areas = ${next.preferred_areas},
          property_types = ${next.property_types},
          priorities = ${next.priorities},
          timeline = ${next.timeline},
          notes = ${next.notes},
          assigned_user_id = ${next.assigned_user_id},
          updated_at = now()
      where id = ${personId} and archived_at is null
      returning id
    `
  })

  return { personId }
}

// ---------------------------------------------------------------------------
// Archive (soft delete)
// ---------------------------------------------------------------------------

export async function archiveClient(
  personId: string,
  run: TxRunner = neonTx,
): Promise<{ personId: string }> {
  if (!isUuid(personId)) {
    throw new PortalWriteError('validation', 'Invalid person identifier.')
  }

  await run(async (tx) => {
    const rows = await tx`
      update person
      set archived_at = now(), updated_at = now()
      where id = ${personId} and archived_at is null
      returning id
    `
    if (rows.length === 0) {
      const exists = await personRow(tx, personId)
      if (!exists) {
        throw new PortalWriteError('not-found', 'Person not found.')
      }
      throw new PortalWriteError('conflict', 'Person is already archived.')
    }
  })

  return { personId }
}

// ---------------------------------------------------------------------------
// Contact identity (email / phone)
// ---------------------------------------------------------------------------

export async function setClientIdentity(
  personId: string,
  kind: ClientIdentityKind,
  value: string | null,
  run: TxRunner = neonTx,
): Promise<{ personId: string; kind: ClientIdentityKind }> {
  if (!isUuid(personId)) {
    throw new PortalWriteError('validation', 'Invalid person identifier.')
  }
  const normalized = normalizeClientContact(kind, value)

  await run(async (tx) => {
    await requireActivePerson(tx, personId)

    // null clears every identity of this kind (the UI exposes the primary
    // email/phone; clearing is the operator's explicit "remove contact").
    if (normalized === null) {
      await tx`
        delete from person_identity
        where person_id = ${personId} and identity_type = ${kind}
      `
      return
    }

    const ownership = await findIdentityOwnership(
      { kind, value: normalized, evidence: 'user_supplied', normalizedValue: normalized },
      tx,
    )
    if (ownership && ownership.personId !== personId) {
      const ownerLabel = await personDisplayName(tx, ownership.personId)
      throw new PortalWriteError(
        'conflict',
        `This ${kind} already belongs to ${ownerLabel}.`,
      )
    }

    // Exactly one primary per kind on this person.
    await tx`
      update person_identity
      set is_primary = false
      where person_id = ${personId} and identity_type = ${kind}
    `

    if (ownership) {
      await tx`
        update person_identity
        set is_primary = true
        where id = ${ownership.identityId}
      `
    } else {
      try {
        await tx`
          insert into person_identity (
            person_id, identity_type, identity_value, is_primary
          ) values (
            ${personId}, ${kind}, ${normalized}, true
          )
        `
      } catch (error) {
        if (isUniqueViolation(error)) {
          throw new PortalWriteError(
            'conflict',
            `This ${kind} already belongs to another person.`,
          )
        }
        throw error
      }
    }
  })

  return { personId, kind }
}
