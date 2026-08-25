'use server'

import { normalizeLeadInput } from '@/lib/catchup/lead-intake'
import { createWebsiteLead } from '@/db/catchup-lead'

// CATCH-UP — website lead intake server action. Accepts Name + (Email OR Phone)
// and returns the canonical write result. Long questionnaires are never
// required. Ambiguous identity is never silently merged.
export async function createWebsiteLeadAction(input: {
  name?: unknown
  email?: unknown
  phone?: unknown
  message?: unknown
}) {
  const parsed = normalizeLeadInput(input)
  if (!parsed.ok) {
    return { ok: false, errors: parsed.errors }
  }
  try {
    const data = await createWebsiteLead(parsed.value)
    return { ok: true, data }
  } catch (error) {
    return {
      ok: false,
      message: error instanceof Error ? error.message : 'Could not create lead.',
    }
  }
}
