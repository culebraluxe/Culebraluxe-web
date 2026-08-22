'use server'

import { randomUUID } from 'node:crypto'

import { findDealById, findPropertyById, findPropertyBySlug } from '@/db/intake-context'
import { getInteractionBySourceIdentity } from '@/db/interactions'
import {
  createPersonWithIdentities,
  findIdentityMatch,
  findIdentityOwnership,
  personExists,
} from '@/db/person-identities'
import {
  claimWebsiteIntakeReceipt,
  insertOrReadWebsiteIntakeReceipt,
  persistCanonicalWebsiteIntake,
  transitionWebsiteIntakeReceipt,
} from '@/db/website-intake'
import {
  parseWebsiteIntakeFormData,
  processWebsiteIntake,
} from '@/lib/website-intake'
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
  // A filled honeypot gets the same non-enumerating success response and no writes.
  if (parsed.honeypot) return { accepted: true, status: 'accepted' }
  const { payload } = parsed

  try {
    return await processWebsiteIntake(payload, {
      createId: randomUUID,
      repositories: {
        findActiveProperty: findPropertyById,
        insertOrReadReceipt: insertOrReadWebsiteIntakeReceipt,
        claimReceipt: claimWebsiteIntakeReceipt,
        transitionReceipt: transitionWebsiteIntakeReceipt,
        persistCanonical: persistCanonicalWebsiteIntake,
        crm: {
          findInteractionBySourceIdentity: getInteractionBySourceIdentity,
          personExists,
          findIdentityMatch,
          findIdentityOwnership,
          createPersonWithIdentities,
          findPropertyById,
          findPropertyBySlug,
          findDealById,
        },
      },
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
 * property-less general enquiry path too. Kept as a non-breaking alias so
 * existing callers keep working.
 */
export async function submitWebsitePropertyIntake(
  formData: FormData,
): Promise<WebsiteIntakeResult> {
  return submitWebsiteIntake(formData)
}
