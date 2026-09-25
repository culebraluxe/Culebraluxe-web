'use server'

import { normalizeLeadInput } from '@/lib/catchup/lead-intake'
import { rustApiCreateCatchupLead } from '@/lib/rust-api/client'

type CatchupLeadResult = {
  status: 'created' | 'resolved' | 'resolution_required'
  personId: string | null
  interactionId: string | null
}

// CATCH-UP — validation remains a pure UI-edge concern. Canonical identity
// resolution, Person creation and Interaction persistence are Rust-owned.
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
    const result = await rustApiCreateCatchupLead<CatchupLeadResult>(parsed.value)
    return { ok: true, data: result.value }
  } catch (error) {
    return {
      ok: false,
      message: error instanceof Error ? error.message : 'Could not create lead.',
    }
  }
}
