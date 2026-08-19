import { sql } from './client'

import type {
  WebsiteIntakeRequestType,
  WebsiteIntakeStatus,
} from '@/lib/website-intake-types'

// Read-only projection for the Portal "Needs Review" intake triage view.
// Surfaces website intake submissions that genuinely need human attention:
// received (arrived, not yet processed) or resolution_required (flagged for a
// human decision). In-flight 'processing' receipts are excluded because they
// are actively owned by the intake pipeline, not awaiting a person. Property
// context is joined via the existing canonical property relationship only; no
// identity inference is performed.

export type NeedsReviewItem = {
  id: string
  requestType: WebsiteIntakeRequestType
  displayName: string
  email: string
  message: string | null
  status: WebsiteIntakeStatus
  receivedAt: string
  receivedAtLabel: string
  propertyName: string | null
  propertyLocation: string | null
}

type NeedsReviewRow = {
  id: string
  request_type: WebsiteIntakeRequestType
  display_name: string
  email: string
  message: string | null
  status: WebsiteIntakeStatus
  created_at: string
  received_at_label: string
  property_name: string | null
  property_location: string | null
}

export async function getNeedsReviewItems(): Promise<NeedsReviewItem[]> {
  const rows = await sql`
    select
      w.id,
      w.request_type,
      w.display_name,
      w.email,
      w.message,
      w.status,
      w.created_at,
      to_char(
        w.created_at at time zone 'America/Puerto_Rico',
        'Mon FMDD, YYYY HH12:MI AM'
      ) as received_at_label,
      property.name as property_name,
      property.location as property_location
    from website_intake_submission w
    left join property
      on property.id = w.property_id
    where w.status in ('received', 'resolution_required')
    order by w.created_at desc
  `

  return (rows as NeedsReviewRow[]).map((row) => ({
    id: row.id,
    requestType: row.request_type,
    displayName: row.display_name,
    email: row.email,
    message: row.message ?? null,
    status: row.status,
    receivedAt: row.created_at,
    receivedAtLabel: row.received_at_label,
    propertyName: row.property_name ?? null,
    propertyLocation: row.property_location ?? null,
  }))
}
