// ---------------------------------------------------------------------------
// OPS-05 — Deal / Participant Administration: pure write contract for the deal
// maintenance seam. No database imports; any layer can import and test it.
//
// One place for the deal-creation normalization/validation and the structural
// participant vocabulary shared by the server actions and the db seam
// (defense in depth: the action layer and the db seam both normalize through
// here, so a bypassed action can never write a malformed deal or participant
// row). Validation failures throw PortalWriteError('validation') so the portal
// write action maps them to a friendly result without classifying by message
// text.
//
// Participant-model decision (migration 034): deal_participant is THE
// canonical participant model with at most ONE active structural participant
// (client/owner/seller) per role per deal. This contract fixes the subject
// KIND per structural role so the per-deal legacy FK mirrors can always be
// kept consistent by the write seam:
//
//     client → person (mirror: deal.client_person_id)
//     owner  → app_user (mirror: deal.owner_user_id)
//     seller → person (no per-deal legacy FK; property.seller_person_id is a
//                      property-domain fact and is not maintained here)
//
// The seller leg deliberately has no mirror update: property.seller_person_id
// is property-scoped and shared across deals, so deal-participant maintenance
// never rewrites it (a future listing-facts story owns that surface).
// ---------------------------------------------------------------------------

import { PortalWriteError } from './portal-write-error'

export const STRUCTURAL_PARTICIPANT_ROLES = [
  'client',
  'owner',
  'seller',
] as const
export type StructuralParticipantRole =
  (typeof STRUCTURAL_PARTICIPANT_ROLES)[number]

export const STRUCTURAL_PARTICIPANT_ROLE_SET: ReadonlySet<string> = new Set(
  STRUCTURAL_PARTICIPANT_ROLES,
)

export function isStructuralParticipantRole(
  value: string,
): value is StructuralParticipantRole {
  return STRUCTURAL_PARTICIPANT_ROLE_SET.has(value)
}

// Subject kind is fixed per structural role (see header). Kept as a plain
// record so the seam can switch on it without re-deriving role semantics.
export const STRUCTURAL_ROLE_SUBJECT_KIND: Record<
  StructuralParticipantRole,
  'person' | 'user'
> = {
  client: 'person',
  owner: 'user',
  seller: 'person',
}

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

export function isUuidLike(value: string): boolean {
  return UUID_PATTERN.test(value)
}

// ---------------------------------------------------------------------------
// Deal creation
// ---------------------------------------------------------------------------

/**
 * Minimal create surface for deal administration. A deal always belongs to an
 * existing active property and always has a client person (deal.client_person_id
 * is NOT NULL); the owner is an optional app user (the internal agent who owns
 * the deal). Stage/date/financing remain workflow-owned (CRM-14 command seams)
 * and are intentionally NOT part of this maintenance surface.
 */
export type DealCreateInput = {
  propertyId: string
  clientPersonId: string
  ownerUserId?: string | null
  notes?: string | null
}

export type NormalizedDealCreate = {
  propertyId: string
  clientPersonId: string
  ownerUserId: string | null
  notes: string | null
}

export function normalizeDealCreateInput(
  input: DealCreateInput,
): NormalizedDealCreate {
  if (!isUuidLike(input.propertyId)) {
    throw new PortalWriteError('validation', 'Invalid property identifier.')
  }
  if (!isUuidLike(input.clientPersonId)) {
    throw new PortalWriteError('validation', 'Invalid client person identifier.')
  }
  if (input.ownerUserId != null && input.ownerUserId !== '' && !isUuidLike(input.ownerUserId)) {
    throw new PortalWriteError('validation', 'Invalid owner user identifier.')
  }
  return {
    propertyId: input.propertyId,
    clientPersonId: input.clientPersonId,
    ownerUserId: input.ownerUserId ? input.ownerUserId : null,
    notes:
      typeof input.notes === 'string' && input.notes.trim() !== ''
        ? input.notes.trim()
        : null,
  }
}

// ---------------------------------------------------------------------------
// Structural participant maintenance
// ---------------------------------------------------------------------------

export type StructuralParticipantInput = {
  dealId: string
  role: StructuralParticipantRole
  personId?: string | null
  userId?: string | null
}

export type NormalizedStructuralParticipant = {
  dealId: string
  role: StructuralParticipantRole
  kind: 'person' | 'user'
  personId: string | null
  userId: string | null
}

export function normalizeStructuralParticipantInput(
  input: StructuralParticipantInput,
): NormalizedStructuralParticipant {
  if (!isUuidLike(input.dealId)) {
    throw new PortalWriteError('validation', 'Invalid deal identifier.')
  }
  if (!isStructuralParticipantRole(input.role)) {
    throw new PortalWriteError(
      'validation',
      'Participant role must be client, owner or seller.',
    )
  }

  const kind = STRUCTURAL_ROLE_SUBJECT_KIND[input.role]
  const personId = input.personId ? input.personId : null
  const userId = input.userId ? input.userId : null

  if (personId && userId) {
    throw new PortalWriteError(
      'validation',
      'Exactly one of person or user is required.',
    )
  }

  if (kind === 'person') {
    if (!personId) {
      throw new PortalWriteError(
        'validation',
        `A person is required for the ${input.role} role.`,
      )
    }
    if (!isUuidLike(personId)) {
      throw new PortalWriteError(
        'validation',
        `Invalid person identifier for the ${input.role} role.`,
      )
    }
    return { dealId: input.dealId, role: input.role, kind, personId, userId: null }
  }

  // kind === 'user' (owner)
  if (!userId) {
    throw new PortalWriteError(
      'validation',
      'An app user is required for the owner role.',
    )
  }
  if (!isUuidLike(userId)) {
    throw new PortalWriteError(
      'validation',
      'Invalid user identifier for the owner role.',
    )
  }
  return { dealId: input.dealId, role: input.role, kind, personId: null, userId }
}
