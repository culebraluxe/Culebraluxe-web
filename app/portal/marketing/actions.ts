'use server'

import { revalidatePath } from 'next/cache'
import { redirect } from 'next/navigation'

import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'
import {
  addSighting,
  confirmPlacement,
  logListingInquiry,
  renewPlacement,
  requestPublishMany,
  withdrawPlacement,
} from '@/db/syndication'
import { searchPeople } from '@/db/people'
import type { SightingNetwork } from '@/lib/syndication/types'
import { isPrepareChannel } from '@/lib/syndication/channels'

export type MarketingWriteState = {
  ok: boolean
  error?: string
  message?: string
} | null

async function requireRead(): Promise<void> {
  const access = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    'portal.read',
  )
  if (!access.ok) redirect(access.redirectTo)
}

function revalidateMarketing() {
  revalidatePath('/portal/marketing')
  revalidatePath('/portal/marketing/syndication')
}

export async function publishListingsAction(
  _prev: MarketingWriteState,
  formData: FormData,
): Promise<MarketingWriteState> {
  await requireRead()
  const propertyId = String(formData.get('propertyId') ?? '').trim()
  if (!propertyId) return { ok: false, error: 'Select a source listing.' }

  const channels = formData
    .getAll('channel')
    .map((value) => String(value))
    .filter(isPrepareChannel)

  const result = await requestPublishMany({ propertyId, channels })
  revalidateMarketing()
  if (!result.ok) {
    return {
      ok: false,
      error: result.error ?? 'One or more adapters could not run.',
      message: result.results
        .map((row) => `${row.channel}: ${row.message ?? row.error ?? ''}`)
        .join(' · '),
    }
  }
  return {
    ok: true,
    message:
      result.results.length === 1
        ? result.results[0].message
        : `Prepared ${result.results.length} channel packs.`,
  }
}

export async function confirmPlacementAction(
  _prev: MarketingWriteState,
  formData: FormData,
): Promise<MarketingWriteState> {
  await requireRead()
  const placementId = String(formData.get('placementId') ?? '').trim()
  const externalUrl = String(formData.get('externalUrl') ?? '').trim() || null
  const externalId = String(formData.get('externalId') ?? '').trim() || null
  if (!placementId) return { ok: false, error: 'Missing placement.' }
  // MLS# is an identifier, never a URL.
  if (externalId && /(https?:|\/\/|zillow|realtor)/i.test(externalId)) {
    return {
      ok: false,
      error: 'MLS# cannot be a URL. Enter the MLS number (paste a Zillow URL as a sighting, or a portal URL in the Matrix URL field).',
    }
  }

  const result = await confirmPlacement({ placementId, externalUrl, externalId })
  revalidateMarketing()
  if (!result.ok) return { ok: false, error: result.error }
  return { ok: true, message: 'Round trip closed — placement is live.' }
}

export async function withdrawPlacementAction(
  _prev: MarketingWriteState,
  formData: FormData,
): Promise<MarketingWriteState> {
  await requireRead()
  const placementId = String(formData.get('placementId') ?? '').trim()
  if (!placementId) return { ok: false, error: 'Missing placement.' }
  const result = await withdrawPlacement(placementId)
  revalidateMarketing()
  if (!result.ok) return { ok: false, error: result.error }
  return { ok: true, message: 'Placement withdrawn.' }
}

export async function renewPlacementAction(
  _prev: MarketingWriteState,
  formData: FormData,
): Promise<MarketingWriteState> {
  await requireRead()
  const placementId = String(formData.get('placementId') ?? '').trim()
  if (!placementId) return { ok: false, error: 'Missing placement.' }
  const result = await renewPlacement(placementId)
  revalidateMarketing()
  if (!result.ok) return { ok: false, error: result.error }
  return { ok: true, message: 'Placement renewed — pack window reopened.' }
}

const SIGHTING_NETWORKS: readonly SightingNetwork[] = ['zillow', 'realtor_com', 'homes_com', 'other']

/** Record where a listing was observed (V3 §2.3). Never creates a placement. */
export async function addSightingAction(
  _prev: MarketingWriteState,
  formData: FormData,
): Promise<MarketingWriteState> {
  await requireRead()
  const propertyId = String(formData.get('propertyId') ?? '').trim()
  const network = String(formData.get('network') ?? '')
  const url = String(formData.get('url') ?? '').trim()
  const notes = String(formData.get('notes') ?? '').trim() || null
  if (!propertyId || !url) return { ok: false, error: 'Choose a listing and paste a URL.' }
  if (!(SIGHTING_NETWORKS as readonly string[]).includes(network)) {
    return { ok: false, error: 'Unknown network.' }
  }
  const result = await addSighting({ propertyId, network: network as SightingNetwork, url, notes })
  revalidateMarketing()
  if (!result.ok) return { ok: false, error: result.error }
  return { ok: true, message: result.message ?? 'Sighting noted — pinned to the constellation.' }
}

const INQUIRY_SOURCES = ['phone', 'whatsapp', 'email', 'walkin'] as const

/** Log a listing inquiry against an existing person (reuses property_interest). */
export async function logInquiryAction(
  _prev: MarketingWriteState,
  formData: FormData,
): Promise<MarketingWriteState> {
  await requireRead()
  const personId = String(formData.get('personId') ?? '').trim()
  const propertyId = String(formData.get('propertyId') ?? '').trim()
  const source = String(formData.get('source') ?? '')
  const notes = String(formData.get('notes') ?? '').trim() || null
  if (!personId || !propertyId) return { ok: false, error: 'Choose a person and a listing.' }
  if (!(INQUIRY_SOURCES as readonly string[]).includes(source)) return { ok: false, error: 'Unknown source.' }
  const result = await logListingInquiry({ personId, propertyId, source: source as (typeof INQUIRY_SOURCES)[number], notes })
  revalidateMarketing()
  if (!result.ok) return { ok: false, error: result.error }
  return { ok: true, message: 'Inquiry logged against the person.' }
}

/** Read-only person picker for the Launch panel (requires a query). */
export async function searchInquiryPeopleAction(query: string) {
  await requireRead()
  return searchPeople(query ?? '', 8)
}
