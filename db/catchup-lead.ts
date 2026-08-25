import { randomUUID } from 'node:crypto'

import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import { createInteraction } from './interactions'
import type { NormalizedLead } from '@/lib/catchup/lead-intake'

// ---------------------------------------------------------------------------
// CATCH-UP — canonical website lead write.
//
// No separate lead table/system: canonical Person remains the person. A valid
// inquiry resolves an existing identity (single owner) or creates a canonical
// Person (role buyer, status new) with the identity, then writes ONE canonical
// interaction with website-inquiry provenance. Ambiguous identity (multiple
// owners, or email/phone pointing at different people) never silently merges.
// ---------------------------------------------------------------------------

export type WebsiteLeadWriteResult = {
  status: 'created' | 'resolved' | 'resolution_required'
  personId: string | null
  interactionId: string | null
}

const AMBIGUOUS = Symbol('ambiguous')

type IdentityRow = { person_id: string }

async function ownerOfIdentity(
  execute: QueryExecutor,
  kind: 'email' | 'phone',
  value: string,
): Promise<string | typeof AMBIGUOUS | null> {
  const rows = (await execute`
    select person_id from person_identity
    where identity_type = ${kind} and identity_value = ${value}
  `) as IdentityRow[]
  const owners = Array.from(new Set(rows.map((r) => r.person_id)))
  if (owners.length === 1) return owners[0]
  if (owners.length > 1) return AMBIGUOUS
  return null
}

export async function createWebsiteLead(
  input: NormalizedLead,
  execute: QueryExecutor = sql,
): Promise<WebsiteLeadWriteResult> {
  const submissionId = randomUUID()

  const emailOwner = input.email
    ? await ownerOfIdentity(execute, 'email', input.email)
    : null
  const phoneOwner = input.phone
    ? await ownerOfIdentity(execute, 'phone', input.phone)
    : null

  if (emailOwner === AMBIGUOUS || phoneOwner === AMBIGUOUS) {
    return { status: 'resolution_required', personId: null, interactionId: null }
  }
  if (
    emailOwner &&
    phoneOwner &&
    typeof emailOwner === 'string' &&
    typeof phoneOwner === 'string' &&
    emailOwner !== phoneOwner
  ) {
    // Email and phone each resolve to a different person — do not silently merge.
    return { status: 'resolution_required', personId: null, interactionId: null }
  }

  const existing =
    (typeof emailOwner === 'string' ? emailOwner : null) ??
    (typeof phoneOwner === 'string' ? phoneOwner : null)

  let personId = existing
  let created = false

  if (!personId) {
    personId = randomUUID()
    await execute`
      insert into person (id, display_name, role, status)
      values (${personId}, ${input.name}, 'buyer', 'new')
    `
    if (input.email) {
      await execute`
        insert into person_identity (person_id, identity_type, identity_value, is_primary)
        values (${personId}, 'email', ${input.email}, true)
      `
    }
    if (input.phone) {
      await execute`
        insert into person_identity (person_id, identity_type, identity_value, is_primary)
        values (${personId}, 'phone', ${input.phone}, true)
      `
    }
    created = true
  }

  const interactionId = randomUUID()
  const { interaction } = await createInteraction(
    {
      personId,
      channel: 'website',
      eventType: 'lead_inquiry',
      direction: 'inbound',
      occurredAt: new Date().toISOString(),
      title: `Website inquiry from ${input.name}`,
      summary: input.message,
      sourceSystem: 'website',
      sourceExternalId: submissionId,
      sourceMetadata: { source: 'website_inquiry', requestType: 'general_enquiry' },
    },
    execute,
  )

  return {
    status: created ? 'created' : 'resolved',
    personId,
    interactionId: interaction.id,
  }
}
