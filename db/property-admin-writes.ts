import { randomUUID } from 'node:crypto'

import { PortalWriteError } from '../lib/portal-write-error'
import {
  normalizePropertyCreateInput,
  type PropertyCreateInput,
} from '../lib/property-admin'
import { neonTx, type TxRunner } from './tx'
import type { QueryExecutor } from './query-executor'

// ---------------------------------------------------------------------------
// OPS-03 — Property Administration: create / archive / restore seam.
//
// Completes the property CRUD surface: the read projection (db/property-admin
// .ts) and the listing-facts/visibility/media writers (db/portal-property.ts)
// already exist; this module owns the remaining lifecycle writes. All writes
// are soft: archive sets property.archived_at and every existing read
// projection (buyers inventory, detail, similar, public slugs, portal intro)
// already filters archived_at is null, so an archived property drops off the
// public site without any hard delete and is restorable.
//
// Boundaries (reuse-first):
//   - Create validation is the shared pure contract in lib/property-admin.ts
//     (status vocabulary, slug pattern, non-negative numerics); the action
//     layer and this seam both run it, so a bypassed action can never write a
//     malformed property row.
//   - city / state_or_province fall back to the schema defaults ('Culebra' /
//     'PR') when the operator leaves them blank, so a new property matches the
//     domain default the migration declares.
//   - The DB slug unique index remains the final backstop; a 23505 race maps
//     to a clear conflict.
//   - Each write runs inside ONE injected transaction (neonTx in production, a
//     fake in tests) so a failed insert can never leave partial state.
//   - No audit rows here: durable actor/action records for property
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

async function propertyExists(tx: QueryExecutor, propertyId: string) {
  const rows = await tx`
    select id from property where id = ${propertyId} limit 1
  `
  return rows.length > 0
}

// ---------------------------------------------------------------------------
// Create
// ---------------------------------------------------------------------------

export async function createProperty(
  input: PropertyCreateInput,
  run: TxRunner = neonTx,
): Promise<{ id: string; slug: string | null }> {
  const property = normalizePropertyCreateInput(input)
  const id = randomUUID()

  await run(async (tx) => {
    if (property.slug) {
      const clash = await tx`
        select id from property
        where slug = ${property.slug}
        limit 1
      `
      if (clash.length > 0) {
        throw new PortalWriteError(
          'conflict',
          'Another property already uses this slug.',
        )
      }
    }

    try {
      await tx`
        insert into property (
          id, name, slug, status, featured, property_type, list_price,
          location, city, state_or_province, neighborhood,
          bedrooms, bathrooms, square_feet
        ) values (
          ${id}, ${property.name}, ${property.slug}, ${property.status},
          ${property.featured}, ${property.propertyType}, ${property.listPrice},
          ${property.location},
          coalesce(${property.city}, 'Culebra'),
          coalesce(${property.stateOrProvince}, 'PR'),
          ${property.neighborhood},
          ${property.bedrooms}, ${property.bathrooms}, ${property.squareFeet}
        )
      `
    } catch (error) {
      if (isUniqueViolation(error)) {
        throw new PortalWriteError(
          'conflict',
          'Another property already uses this slug.',
        )
      }
      throw error
    }
  })

  return { id, slug: property.slug }
}

// ---------------------------------------------------------------------------
// Archive (soft delete)
// ---------------------------------------------------------------------------

export async function archiveProperty(
  propertyId: string,
  run: TxRunner = neonTx,
): Promise<{ id: string }> {
  if (!isUuid(propertyId)) {
    throw new PortalWriteError('validation', 'Invalid property identifier.')
  }

  await run(async (tx) => {
    const rows = await tx`
      update property
      set archived_at = now(), updated_at = now()
      where id = ${propertyId} and archived_at is null
      returning id
    `
    if (rows.length === 0) {
      if (!(await propertyExists(tx, propertyId))) {
        throw new PortalWriteError('not-found', 'Property not found.')
      }
      throw new PortalWriteError('conflict', 'Property is already archived.')
    }
  })

  return { id: propertyId }
}

// ---------------------------------------------------------------------------
// Restore (clear the soft-delete flag)
// ---------------------------------------------------------------------------

export async function restoreProperty(
  propertyId: string,
  run: TxRunner = neonTx,
): Promise<{ id: string }> {
  if (!isUuid(propertyId)) {
    throw new PortalWriteError('validation', 'Invalid property identifier.')
  }

  await run(async (tx) => {
    const rows = await tx`
      update property
      set archived_at = null, updated_at = now()
      where id = ${propertyId} and archived_at is not null
      returning id
    `
    if (rows.length === 0) {
      if (!(await propertyExists(tx, propertyId))) {
        throw new PortalWriteError('not-found', 'Property not found.')
      }
      throw new PortalWriteError(
        'conflict',
        'Property is not archived.',
      )
    }
  })

  return { id: propertyId }
}
