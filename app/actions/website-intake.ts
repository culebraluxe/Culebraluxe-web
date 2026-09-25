'use server'

import {
  parseWebsiteIntakeFormData,
} from '@/lib/website-intake'
import {
  rustApiSubmitWebsiteIntake,
} from '@/lib/rust-api/client'
import type { WebsiteIntakeResult } from '@/lib/website-intake-types'

const unavailable: WebsiteIntakeResult = {
  accepted: false,
  status: 'unavailable',
}

export async function submitWebsiteIntake(
  formData: FormData,
): Promise<WebsiteIntakeResult> {
  let parsed
  try {
    parsed = parseWebsiteIntakeFormData(formData)
  } catch {
    return { accepted: false, status: 'invalid' }
  }

  // Preserve the non-enumerating honeypot behavior: accepted, no persistence.
  if (parsed.honeypot) return { accepted: true, status: 'accepted' }

  const { payload } = parsed
  try {
    return await rustApiSubmitWebsiteIntake({
      submissionId: payload.submissionId,
      requestType: payload.requestType,
      propertyId: payload.propertyId ?? null,
      displayName: payload.displayName,
      email: payload.email,
      message: payload.message ?? null,
      service: payload.service ?? null,
    })
  } catch (error) {
    console.error('Website intake could not be completed.', {
      error: error instanceof Error ? error.message : 'Unknown error',
      submissionId: payload.submissionId,
    })
    return unavailable
  }
}

/**
 * @deprecated Renamed to `submitWebsiteIntake` — this action handles the
 * property-less general enquiry path too. Kept as a non-breaking alias.
 */
export async function submitWebsitePropertyIntake(
  formData: FormData,
): Promise<WebsiteIntakeResult> {
  return submitWebsiteIntake(formData)
}
