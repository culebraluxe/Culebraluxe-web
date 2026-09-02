'use server'

import { revalidatePath } from 'next/cache'
import { redirect } from 'next/navigation'

import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'
import {
  confirmPlacement,
  requestPublishMany,
  withdrawPlacement,
} from '@/db/syndication'
import { isSyndicationChannel } from '@/lib/syndication/channels'

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
    .filter(isSyndicationChannel)

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
  if (!placementId) return { ok: false, error: 'Missing placement.' }

  const result = await confirmPlacement({ placementId, externalUrl })
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
