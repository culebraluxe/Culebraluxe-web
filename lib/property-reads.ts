import 'server-only'

import { randomUUID } from 'node:crypto'

import { coreServices } from '@/lib/service-runtime'
import { PROPERTY_OPERATIONS } from '@/services/property'
import type {
  PropertyFilterInput,
  PropertyIntro,
  PropertySummary,
} from '@/services/property'
import type { Result } from '@/db/client'
import type { PropertyDetailResult } from '@/lib/property-types'

/**
 * The public site's property reads — through the Property SERVICE, never the
 * database. This module exists so public pages keep their simple, failure-safe
 * call shape (`Result`) while the data actually comes from the service kernel.
 *
 * Every read answers one question: which properties are ACTIVE LISTINGS we are
 * marketing right now? A property that is only a known place (an Apple Contacts
 * address, or a row we stopped marketing) is not inventory and never appears.
 *
 * Hardening: these reads keep the DB-HARDEN-01C contract — a failure is a Result,
 * never a rejected page. The service kernel captures the underlying failure.
 */

export type {
  PropertyFilterInput,
  PropertyIntro,
  PropertySummary,
}

const service = coreServices.property

/** Public reads run as the unauthenticated public surface (GUEST may query). */
function publicContext() {
  return {
    actor: { id: null, kind: 'system' as const },
    correlationId: randomUUID(),
  }
}

async function read<T>(operation: string, payload: unknown): Promise<Result<T>> {
  const context = publicContext()
  const result = await service.execute({
    operation: operation as never,
    payload: payload as never,
    context,
  })

  if (!result.ok) {
    // A kernel/authorization failure (not a DB failure — those travel inside the
    // Result the repository returns). The kernel already captured it; mirror it as
    // a Result so a public page degrades instead of rejecting.
    return {
      ok: false,
      error: {
        kind: 'UNKNOWN',
        operation,
        incidentId: context.correlationId,
        code: result.error.code,
        detail: result.error.message,
      },
    }
  }

  return result.value as Result<T>
}

/* ============================================================
   INVENTORY
   ============================================================ */

/**
 * Public inventory.
 *
 * HARDEN-05 — `publicOnly: true` returns ACTIVE LISTINGS only (is_active_listing
 * + is_published, non-archived). The public surfaces MUST pass it. `false` (the
 * default) returns the working lifecycle set used by internal portal pickers.
 */
export async function getProperties(opts: { publicOnly?: boolean } = {}) {
  return read<PropertySummary[]>(PROPERTY_OPERATIONS.LIST, {
    publicOnly: opts.publicOnly === true,
  })
}

/** Structured buyers search over active-listing inventory (PX-24B). */
export async function getFilteredProperties(filters: PropertyFilterInput) {
  return read<{ properties: PropertySummary[]; viewOptions: string[] }>(
    PROPERTY_OPERATIONS.SEARCH,
    { filters },
  )
}

/** Deterministic similar listings for the current property. */
export async function getSimilarProperties(
  propertyId: string,
  current: {
    propertyType: string | null
    city: string | null
    neighborhood: string | null
    listPrice: number | null
  },
  limit = 3,
) {
  return read<PropertySummary[]>(PROPERTY_OPERATIONS.SIMILAR, {
    propertyId,
    current,
    limit,
  })
}

/** Minimal public Property reference (id, name, location). */
export async function getPropertyIntroById(propertyId: string) {
  return read<PropertyIntro | null>(PROPERTY_OPERATIONS.INTRO, { propertyId })
}

/* ============================================================
   LISTING DETAIL
   ============================================================ */

/** The set of currently live public listing slugs. */
export async function getPublicPropertySlugs() {
  return read<string[]>(PROPERTY_OPERATIONS.PUBLIC_SLUGS, {})
}

/** Full public listing payload for one slug. */
export async function getPropertyBySlug(slug: string) {
  return read<PropertyDetailResult | null>(PROPERTY_OPERATIONS.BY_SLUG, { slug })
}
