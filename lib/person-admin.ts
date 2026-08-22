// ---------------------------------------------------------------------------
// OPS-02 — Client Administration: pure write contract for the client
// (person) CRUD seam. No database imports; any layer can import and test it.
//
// One place for the closed role/status vocabularies, the editable profile
// shape, and input normalization/validation shared by the server actions and
// db/person-admin.ts (defense in depth: the action layer and the db seam both
// normalize through here, so a bypassed action can never write a malformed
// person row). Validation failures throw PortalWriteError('validation') so
// the portal write action maps them to a friendly result without classifying
// by message text.
// ---------------------------------------------------------------------------

import { PortalWriteError } from './portal-write-error'
import {
  normalizeDisplayName,
  normalizeEmail,
  normalizePhone,
} from './crm-intake-normalization'

export const CLIENT_ROLES = ['buyer', 'seller', 'both'] as const
export type ClientRole = (typeof CLIENT_ROLES)[number]

export const CLIENT_STATUSES = ['new', 'warm', 'active', 'referral'] as const
export type ClientStatus = (typeof CLIENT_STATUSES)[number]

export type ClientIdentityKind = 'email' | 'phone'

export const CLIENT_ROLE_SET: ReadonlySet<string> = new Set(CLIENT_ROLES)
export const CLIENT_STATUS_SET: ReadonlySet<string> = new Set(CLIENT_STATUSES)

export function isClientRole(value: string): value is ClientRole {
  return CLIENT_ROLE_SET.has(value)
}

export function isClientStatus(value: string): value is ClientStatus {
  return CLIENT_STATUS_SET.has(value)
}

/**
 * Editable person profile fields (everything except email/phone, which live
 * on person_identity and are managed by setClientIdentity).
 */
export type ClientProfileFields = {
  displayName?: string
  role?: ClientRole
  status?: ClientStatus
  location?: string | null
  budgetMin?: number | null
  budgetMax?: number | null
  preferredAreas?: string[] | null
  propertyTypes?: string[] | null
  priorities?: string[] | null
  timeline?: string | null
  notes?: string | null
  assignedUserId?: string | null
}

export type ClientCreateInput = ClientProfileFields & {
  displayName: string
  role: ClientRole
  email?: string | null
  phone?: string | null
}

export type NormalizedClientCreate = {
  displayName: string
  role: ClientRole
  status: ClientStatus
  location: string | null
  budgetMin: number | null
  budgetMax: number | null
  preferredAreas: string[] | null
  propertyTypes: string[] | null
  priorities: string[] | null
  timeline: string | null
  notes: string | null
  assignedUserId: string | null
  email: string | null
  phone: string | null
}

/** Normalized partial profile update; undefined = leave unchanged, null = clear. */
export type NormalizedClientProfileUpdate = {
  displayName?: string
  role?: ClientRole
  status?: ClientStatus
  location?: string | null
  budgetMin?: number | null
  budgetMax?: number | null
  preferredAreas?: string[] | null
  propertyTypes?: string[] | null
  priorities?: string[] | null
  timeline?: string | null
  notes?: string | null
  assignedUserId?: string | null
}

function cleanText(value: string | null | undefined) {
  if (value == null) return null
  const trimmed = value.trim()
  return trimmed.length > 0 ? trimmed : null
}

function cleanArray(values: string[] | null | undefined) {
  if (!Array.isArray(values)) return null
  const cleaned = values
    .map((value) => value.trim())
    .filter((value) => value.length > 0)
  return cleaned.length > 0 ? cleaned : null
}

function budgetNumber(value: number | null | undefined) {
  if (value == null) return null
  if (!Number.isFinite(value) || value < 0) {
    throw new PortalWriteError('validation', 'Budget must be a positive number.')
  }
  return value
}

function assertBudgetOrder(min: number | null, max: number | null) {
  if (min != null && max != null && min > max) {
    throw new PortalWriteError(
      'validation',
      'Minimum budget cannot exceed maximum budget.',
    )
  }
}

function normalizeProfile(input: ClientProfileFields): NormalizedClientProfileUpdate {
  const normalized: NormalizedClientProfileUpdate = {}

  if (input.displayName !== undefined) {
    try {
      normalized.displayName = normalizeDisplayName(input.displayName)
    } catch {
      throw new PortalWriteError('validation', 'Display name is required.')
    }
  }

  if (input.role !== undefined) {
    if (!isClientRole(input.role)) {
      throw new PortalWriteError('validation', 'Client role is invalid.')
    }
    normalized.role = input.role
  }

  if (input.status !== undefined) {
    if (!isClientStatus(input.status)) {
      throw new PortalWriteError('validation', 'Client status is invalid.')
    }
    normalized.status = input.status
  }

  if (input.location !== undefined) {
    normalized.location = cleanText(input.location)
  }

  if (input.budgetMin !== undefined) {
    normalized.budgetMin = budgetNumber(input.budgetMin)
  }
  if (input.budgetMax !== undefined) {
    normalized.budgetMax = budgetNumber(input.budgetMax)
  }
  if (
    normalized.budgetMin !== undefined ||
    normalized.budgetMax !== undefined
  ) {
    assertBudgetOrder(
      normalized.budgetMin ?? null,
      normalized.budgetMax ?? null,
    )
  }

  if (input.preferredAreas !== undefined) {
    normalized.preferredAreas = cleanArray(input.preferredAreas)
  }
  if (input.propertyTypes !== undefined) {
    normalized.propertyTypes = cleanArray(input.propertyTypes)
  }
  if (input.priorities !== undefined) {
    normalized.priorities = cleanArray(input.priorities)
  }
  if (input.timeline !== undefined) {
    normalized.timeline = cleanText(input.timeline)
  }
  if (input.notes !== undefined) {
    normalized.notes = cleanText(input.notes)
  }
  if (input.assignedUserId !== undefined) {
    normalized.assignedUserId = cleanText(input.assignedUserId)
  }

  return normalized
}

/**
 * Normalize + validate a create-client input. Throws PortalWriteError on any
 * validation failure; returns the fully typed row shape the db seam inserts.
 */
export function normalizeClientCreateInput(
  input: ClientCreateInput,
): NormalizedClientCreate {
  if (!input || typeof input !== 'object') {
    throw new PortalWriteError('validation', 'Client input is invalid.')
  }
  if (!input.displayName || !input.displayName.trim()) {
    throw new PortalWriteError('validation', 'Display name is required.')
  }
  if (!isClientRole(input.role)) {
    throw new PortalWriteError('validation', 'Client role is invalid.')
  }

  const profile = normalizeProfile(input)
  const status = input.status ?? 'new'
  if (!isClientStatus(status)) {
    throw new PortalWriteError('validation', 'Client status is invalid.')
  }

  return {
    displayName: profile.displayName ?? normalizeDisplayName(input.displayName),
    role: input.role,
    status,
    location: profile.location ?? null,
    budgetMin: profile.budgetMin ?? null,
    budgetMax: profile.budgetMax ?? null,
    preferredAreas: profile.preferredAreas ?? null,
    propertyTypes: profile.propertyTypes ?? null,
    priorities: profile.priorities ?? null,
    timeline: profile.timeline ?? null,
    notes: profile.notes ?? null,
    assignedUserId: profile.assignedUserId ?? null,
    email: normalizeClientContact('email', input.email ?? null),
    phone: normalizeClientContact('phone', input.phone ?? null),
  }
}

/**
 * Normalize + validate a partial profile update (undefined = unchanged).
 * Returns undefined when the input is empty; the db seam treats undefined
 * keys as "do not touch".
 */
export function normalizeClientProfileUpdate(
  input: ClientProfileFields,
): NormalizedClientProfileUpdate {
  if (!input || typeof input !== 'object') {
    throw new PortalWriteError('validation', 'Client input is invalid.')
  }
  return normalizeProfile(input)
}

/**
 * Normalize a contact identity value for a client. null/empty clears the
 * identity; a value must pass the canonical CRM normalization (canonical
 * email or strict E.164 phone).
 */
export function normalizeClientContact(
  kind: ClientIdentityKind,
  value: string | null | undefined,
): string | null {
  if (value == null || value.trim() === '') return null
  try {
    if (kind === 'email') return normalizeEmail(value)
    return normalizePhone(value)
  } catch (error) {
    throw new PortalWriteError(
      'validation',
      error instanceof Error
        ? error.message
        : `${kind} identity is invalid.`,
    )
  }
}
