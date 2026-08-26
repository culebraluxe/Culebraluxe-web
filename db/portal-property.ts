import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import { PortalWriteError } from '@/lib/portal-write-error'
import { getPropertyOpenTasks } from './tasks'
import type { PropertyOpenTask } from './tasks'

// Bounded Listing / Seller Operations V1 (LOPS-01..09). This module owns the
// Property Administration write surface (server-action boundary calls here)
// and a single composed read projection for the per-property workspace page.
//
// Writes touch ONLY existing canonical columns:
//   - property facts/visibility        (property table)
//   - media editorial metadata         (media.alt_text / media.caption)
//   - property media order/hero/unlink (property_media table)
//
// Deliberately out of scope: schema changes, provider mutations, status values
// owned by CRM-14 transaction semantics, and any seller/person assignment.

export const EDITABLE_PROPERTY_STATUSES = [
  'prospect',
  'coming_soon',
  'active',
  'off_market',
] as const
export type EditablePropertyStatus = (typeof EDITABLE_PROPERTY_STATUSES)[number]

// Statuses that semantically represent transaction orchestration (accepted
// offer, closing, sold) and are therefore CRM-14-owned. They are surfaced
// read-only in the admin UI and never accepted by the visibility writer.
export const CRM14_PROPERTY_STATUSES = ['under_contract', 'sold'] as const

// ---------------------------------------------------------------------------
// Read projection
// ---------------------------------------------------------------------------

export type PropertyFactsView = {
  id: string
  name: string
  slug: string | null
  status: string
  featured: boolean
  propertyType: string | null
  listPrice: number | null
  location: string | null
  city: string | null
  stateOrProvince: string | null
  neighborhood: string | null
  latitude: number | null
  longitude: number | null
  bedrooms: number | null
  bathrooms: number | null
  squareFeet: number | null
  lotSize: number | null
  lotSizeUnits: string | null
  yearBuilt: number | null
  stories: number | null
  parkingSpaces: number | null
  shortDescription: string | null
  editorialDescription: string | null
  listingAgentName: string | null
  listingAgentEmail: string | null
  listingAgentPhone: string | null
  listingOffice: string | null
  archived: boolean
  createdAtLabel: string | null
  updatedAtLabel: string | null
}

export type WorkspaceMedia = {
  id: string
  mediaType: string
  role: string
  sortOrder: number
  altText: string | null
  caption: string | null
  filename: string | null
  mimeType: string | null
  muxPlaybackId: string | null
}

export type WorkspaceSeller = {
  id: string
  name: string
  role: string
  status: string
  location: string | null
  email: string | null
  phone: string | null
  assignedAgent: string | null
}

export type PropertyActivityEntry = {
  id: string
  kind: 'interaction' | 'showing' | 'intake' | 'task' | 'deal'
  at: string
  atLabel: string
  title: string
  summary: string | null
  personName: string | null
  personId: string | null
  dealId: string | null
}

export type WorkspaceInterest = {
  id: string
  personId: string
  personName: string
  status: string
  ranking: number | null
  createdAtLabel: string
}

export type WorkspaceEnquiry = {
  id: string
  requestType: string
  displayName: string
  email: string
  message: string | null
  status: string
  receivedAtLabel: string
}

export type WorkspaceShowing = {
  id: string
  personId: string
  personName: string
  dealId: string | null
  status: string
  requestedAtLabel: string
  scheduledAtLabel: string | null
  completedAtLabel: string | null
  cancelledAtLabel: string | null
  feedback: string | null
}

export type WorkspaceDeal = {
  id: string
  stage: string
  clientPersonId: string
  clientName: string
  listPrice: number | null
  offerPrice: number | null
  updatedAtLabel: string
  showingCount: number
  offerCount: number
}

export type PropertyWorkspace = {
  property: PropertyFactsView | null
  media: WorkspaceMedia[]
  seller: WorkspaceSeller | null
  openTasks: PropertyOpenTask[]
  activity: PropertyActivityEntry[]
  interests: WorkspaceInterest[]
  enquiries: WorkspaceEnquiry[]
  showings: WorkspaceShowing[]
  deals: WorkspaceDeal[]
}

function toNumber(value: string | null) {
  return value === null ? null : Number(value)
}

export async function getPropertyWorkspace(
  propertyId: string,
): Promise<PropertyWorkspace> {
  const [propertyRows, mediaRows, sellerRows, identityRows, openTasks, activityRows, interestRows, enquiryRows, showingRows, dealRows] =
    await Promise.all([
      sql`
        select
          p.id,
          p.name,
          p.slug,
          p.status,
          p.featured,
          p.property_type,
          p.list_price::text,
          p.location,
          p.city,
          p.state_or_province,
          p.neighborhood,
          p.latitude::text,
          p.longitude::text,
          p.bedrooms::text,
          p.bathrooms::text,
          p.square_feet,
          p.lot_size::text,
          p.lot_size_units,
          p.year_built,
          p.stories::text,
          p.parking_spaces,
          p.short_description,
          p.editorial_description,
          p.listing_agent_name,
          p.listing_agent_email,
          p.listing_agent_phone,
          p.listing_office,
          p.archived_at,
          to_char(
            p.created_at at time zone 'America/Puerto_Rico',
            'Mon FMDD, YYYY'
          ) as created_at_label,
          to_char(
            p.updated_at at time zone 'America/Puerto_Rico',
            'Mon FMDD, YYYY HH12:MI AM'
          ) as updated_at_label
        from property p
        where p.id = ${propertyId}
        limit 1
      `,
      sql`
        select
          m.id as media_id,
          m.media_type,
          pm.role,
          pm.sort_order,
          m.alt_text,
          m.caption,
          m.filename,
          m.mime_type,
          m.mux_playback_id
        from property_media pm
        join media m
          on m.id = pm.media_id
        where pm.property_id = ${propertyId}
        order by
          case pm.role
            when 'hero' then 0
            when 'gallery' then 1
            when 'video' then 2
            when 'short' then 3
            when 'document' then 4
            else 5
          end,
          pm.sort_order asc,
          pm.created_at asc
      `,
      sql`
        select
          p.id,
          p.display_name,
          p.role,
          p.status,
          p.location,
          u.display_name as assigned_user_name
        from property prop
        join person p
          on p.id = prop.seller_person_id
        left join app_user u
          on u.id = p.assigned_user_id
        where prop.id = ${propertyId}
        limit 1
      `,
      sql`
        select
          pi.person_id,
          pi.identity_type,
          pi.identity_value,
          pi.is_primary
        from property prop
        join person_identity pi
          on pi.person_id = prop.seller_person_id
        where prop.id = ${propertyId}
        order by pi.is_primary desc, pi.identity_type asc
      `,
      getPropertyOpenTasks(propertyId),
      (async () => {
        const [interactionRows, showingRows, intakeRows, taskRows, dealRows] =
          await Promise.all([
            sql`
              select
                i.id,
                i.occurred_at::text as at,
                to_char(
                  i.occurred_at at time zone 'America/Puerto_Rico',
                  'Mon FMDD, YYYY HH12:MI AM'
                ) as at_label,
                coalesce(i.title, 'Interaction') as title,
                i.summary,
                person.display_name as person_name,
                person.id as person_id,
                i.deal_id
              from interaction i
              join person
                on person.id = i.person_id
              where i.property_id = ${propertyId}
                 or i.deal_id in (
                   select d.id from deal d where d.property_id = ${propertyId}
                 )
            `,
            sql`
              select
                s.id,
                s.requested_at::text as at,
                to_char(
                  s.requested_at at time zone 'America/Puerto_Rico',
                  'Mon FMDD, YYYY HH12:MI AM'
                ) as at_label,
                'Showing ' || s.status as title,
                s.feedback as summary,
                person.display_name as person_name,
                person.id as person_id,
                s.deal_id
              from showing s
              join person
                on person.id = s.person_id
              where s.property_id = ${propertyId}
                 or s.deal_id in (
                   select d.id from deal d where d.property_id = ${propertyId}
                 )
            `,
            sql`
              select
                w.id,
                w.created_at::text as at,
                to_char(
                  w.created_at at time zone 'America/Puerto_Rico',
                  'Mon FMDD, YYYY HH12:MI AM'
                ) as at_label,
                case
                  when w.request_type = 'private_viewing'
                  then 'Private viewing enquiry'
                  else 'Property information enquiry'
                end as title,
                w.message as summary,
                w.display_name as person_name,
                null::uuid as person_id,
                null::uuid as deal_id
              from website_intake_submission w
              where w.property_id = ${propertyId}
            `,
            sql`
              select
                t.id,
                t.created_at::text as at,
                to_char(
                  t.created_at at time zone 'America/Puerto_Rico',
                  'Mon FMDD, YYYY HH12:MI AM'
                ) as at_label,
                'Task: ' || t.title as title,
                t.detail as summary,
                person.display_name as person_name,
                person.id as person_id,
                t.deal_id
              from task t
              left join person
                on person.id = t.person_id
              where t.property_id = ${propertyId}
                 or t.deal_id in (
                   select d.id from deal d where d.property_id = ${propertyId}
                 )
            `,
            sql`
              select
                d.id,
                d.created_at::text as at,
                to_char(
                  d.created_at at time zone 'America/Puerto_Rico',
                  'Mon FMDD, YYYY HH12:MI AM'
                ) as at_label,
                'Deal: ' || client.display_name as title,
                d.notes as summary,
                client.display_name as person_name,
                d.client_person_id as person_id,
                d.id as deal_id
              from deal d
              join person client
                on client.id = d.client_person_id
              where d.property_id = ${propertyId}
            `,
          ])

        type ActivityRaw = {
          id: string
          at: string
          at_label: string
          title: string
          summary: string | null
          person_name: string | null
          person_id: string | null
          deal_id: string | null
        }

        const entries: PropertyActivityEntry[] = []

        for (const row of interactionRows as ActivityRaw[]) {
          entries.push({
            id: row.id,
            kind: 'interaction',
            at: row.at,
            atLabel: row.at_label,
            title: row.title,
            summary: row.summary ?? null,
            personName: row.person_name ?? null,
            personId: row.person_id ?? null,
            dealId: row.deal_id ?? null,
          })
        }
        for (const row of showingRows as ActivityRaw[]) {
          entries.push({
            id: row.id,
            kind: 'showing',
            at: row.at,
            atLabel: row.at_label,
            title: row.title,
            summary: row.summary ?? null,
            personName: row.person_name ?? null,
            personId: row.person_id ?? null,
            dealId: row.deal_id ?? null,
          })
        }
        for (const row of intakeRows as ActivityRaw[]) {
          entries.push({
            id: row.id,
            kind: 'intake',
            at: row.at,
            atLabel: row.at_label,
            title: row.title,
            summary: row.summary ?? null,
            personName: row.person_name ?? null,
            personId: row.person_id ?? null,
            dealId: row.deal_id ?? null,
          })
        }
        for (const row of taskRows as ActivityRaw[]) {
          entries.push({
            id: row.id,
            kind: 'task',
            at: row.at,
            atLabel: row.at_label,
            title: row.title,
            summary: row.summary ?? null,
            personName: row.person_name ?? null,
            personId: row.person_id ?? null,
            dealId: row.deal_id ?? null,
          })
        }
        for (const row of dealRows as ActivityRaw[]) {
          entries.push({
            id: row.id,
            kind: 'deal',
            at: row.at,
            atLabel: row.at_label,
            title: row.title,
            summary: row.summary ?? null,
            personName: row.person_name ?? null,
            personId: row.person_id ?? null,
            dealId: row.deal_id ?? null,
          })
        }

        return entries.sort((a, b) => (a.at < b.at ? 1 : a.at > b.at ? -1 : 0)).slice(0, 50)
      })(),
      sql`
        select
          pi.id,
          pi.person_id,
          pi.status,
          pi.ranking,
          person.display_name as person_name,
          to_char(
            pi.created_at at time zone 'America/Puerto_Rico',
            'Mon FMDD, YYYY'
          ) as created_at_label
        from property_interest pi
        join person
          on person.id = pi.person_id
        where pi.property_id = ${propertyId}
        order by pi.created_at desc
      `,
      sql`
        select
          w.id,
          w.request_type,
          w.display_name,
          w.email,
          w.message,
          w.status,
          to_char(
            w.created_at at time zone 'America/Puerto_Rico',
            'Mon FMDD, YYYY HH12:MI AM'
          ) as received_at_label
        from website_intake_submission w
        where w.property_id = ${propertyId}
        order by w.created_at desc
      `,
      sql`
        select
          s.id,
          s.person_id,
          s.deal_id,
          s.status,
          s.feedback,
          person.display_name as person_name,
          to_char(
            s.requested_at at time zone 'America/Puerto_Rico',
            'Mon FMDD, YYYY HH12:MI AM'
          ) as requested_at_label,
          to_char(
            s.scheduled_at at time zone 'America/Puerto_Rico',
            'Mon FMDD, YYYY HH12:MI AM'
          ) as scheduled_at_label,
          to_char(
            s.completed_at at time zone 'America/Puerto_Rico',
            'Mon FMDD, YYYY HH12:MI AM'
          ) as completed_at_label,
          to_char(
            s.cancelled_at at time zone 'America/Puerto_Rico',
            'Mon FMDD, YYYY HH12:MI AM'
          ) as cancelled_at_label
        from showing s
        join person
          on person.id = s.person_id
        where s.property_id = ${propertyId}
           or s.deal_id in (
             select d.id from deal d where d.property_id = ${propertyId}
           )
        order by s.requested_at desc
      `,
      sql`
        select
          d.id,
          d.stage,
          d.client_person_id,
          d.list_price::text,
          d.offer_price::text,
          client.display_name as client_name,
          to_char(
            d.updated_at at time zone 'America/Puerto_Rico',
            'Mon FMDD, YYYY'
          ) as updated_at_label,
          (select count(*) from showing s where s.deal_id = d.id)::int as showing_count,
          (select count(*) from offer o where o.deal_id = d.id)::int as offer_count
        from deal d
        join person client
          on client.id = d.client_person_id
        where d.property_id = ${propertyId}
        order by d.updated_at desc
      `,
    ])

  const propertyRow = propertyRows[0] as {
    id: string
    name: string
    slug: string | null
    status: string
    featured: boolean
    property_type: string | null
    list_price: string | null
    location: string | null
    city: string | null
    state_or_province: string | null
    neighborhood: string | null
    latitude: string | null
    longitude: string | null
    bedrooms: string | null
    bathrooms: string | null
    square_feet: number | null
    lot_size: string | null
    lot_size_units: string | null
    year_built: number | null
    stories: string | null
    parking_spaces: number | null
    short_description: string | null
    editorial_description: string | null
    listing_agent_name: string | null
    listing_agent_email: string | null
    listing_agent_phone: string | null
    listing_office: string | null
    archived_at: string | null
    created_at_label: string | null
    updated_at_label: string | null
  } | undefined

  const sellerRow = sellerRows[0] as {
    id: string
    display_name: string
    role: string
    status: string
    location: string | null
    assigned_user_name: string | null
  } | undefined

  const identityRowsTyped = identityRows as Array<{
    person_id: string
    identity_type: string
    identity_value: string
    is_primary: boolean
  }>

  const email = identityRowsTyped.find((r) => r.identity_type === 'email')?.identity_value ?? null
  const phone = identityRowsTyped.find((r) => r.identity_type === 'phone')?.identity_value ?? null

  const seller: WorkspaceSeller | null = sellerRow
    ? {
        id: sellerRow.id,
        name: sellerRow.display_name,
        role: sellerRow.role,
        status: sellerRow.status,
        location: sellerRow.location ?? null,
        email: email ?? null,
        phone: phone ?? null,
        assignedAgent: sellerRow.assigned_user_name ?? null,
      }
    : null

  const media = (mediaRows as Array<{
    media_id: string
    media_type: string
    role: string
    sort_order: number
    alt_text: string | null
    caption: string | null
    filename: string | null
    mime_type: string | null
    mux_playback_id: string | null
  }>).map((row) => ({
    id: row.media_id,
    mediaType: row.media_type,
    role: row.role,
    sortOrder: row.sort_order,
    altText: row.alt_text ?? null,
    caption: row.caption ?? null,
    filename: row.filename ?? null,
    mimeType: row.mime_type ?? null,
    muxPlaybackId: row.mux_playback_id ?? null,
  }))

  const interests = (interestRows as Array<{
    id: string
    person_id: string
    status: string
    ranking: number | null
    person_name: string
    created_at_label: string
  }>).map((row) => ({
    id: row.id,
    personId: row.person_id,
    personName: row.person_name,
    status: row.status,
    ranking: row.ranking ?? null,
    createdAtLabel: row.created_at_label,
  }))

  const enquiries = (enquiryRows as Array<{
    id: string
    request_type: string
    display_name: string
    email: string
    message: string | null
    status: string
    received_at_label: string
  }>).map((row) => ({
    id: row.id,
    requestType: row.request_type,
    displayName: row.display_name,
    email: row.email,
    message: row.message ?? null,
    status: row.status,
    receivedAtLabel: row.received_at_label,
  }))

  const showings = (showingRows as Array<{
    id: string
    person_id: string
    deal_id: string | null
    status: string
    feedback: string | null
    person_name: string
    requested_at_label: string
    scheduled_at_label: string | null
    completed_at_label: string | null
    cancelled_at_label: string | null
  }>).map((row) => ({
    id: row.id,
    personId: row.person_id,
    personName: row.person_name,
    dealId: row.deal_id ?? null,
    status: row.status,
    requestedAtLabel: row.requested_at_label,
    scheduledAtLabel: row.scheduled_at_label ?? null,
    completedAtLabel: row.completed_at_label ?? null,
    cancelledAtLabel: row.cancelled_at_label ?? null,
    feedback: row.feedback ?? null,
  }))

  const deals = (dealRows as Array<{
    id: string
    stage: string
    client_person_id: string
    list_price: string | null
    offer_price: string | null
    client_name: string
    updated_at_label: string
    showing_count: number
    offer_count: number
  }>).map((row) => ({
    id: row.id,
    stage: row.stage,
    clientPersonId: row.client_person_id,
    clientName: row.client_name,
    listPrice: toNumber(row.list_price),
    offerPrice: toNumber(row.offer_price),
    updatedAtLabel: row.updated_at_label,
    showingCount: row.showing_count,
    offerCount: row.offer_count,
  }))

  return {
    property: propertyRow
      ? {
          id: propertyRow.id,
          name: propertyRow.name,
          slug: propertyRow.slug ?? null,
          status: propertyRow.status,
          featured: propertyRow.featured,
          propertyType: propertyRow.property_type ?? null,
          listPrice: toNumber(propertyRow.list_price),
          location: propertyRow.location ?? null,
          city: propertyRow.city ?? null,
          stateOrProvince: propertyRow.state_or_province ?? null,
          neighborhood: propertyRow.neighborhood ?? null,
          latitude: toNumber(propertyRow.latitude),
          longitude: toNumber(propertyRow.longitude),
          bedrooms: toNumber(propertyRow.bedrooms),
          bathrooms: toNumber(propertyRow.bathrooms),
          squareFeet: propertyRow.square_feet,
          lotSize: toNumber(propertyRow.lot_size),
          lotSizeUnits: propertyRow.lot_size_units ?? null,
          yearBuilt: propertyRow.year_built,
          stories: toNumber(propertyRow.stories),
          parkingSpaces: propertyRow.parking_spaces,
          shortDescription: propertyRow.short_description ?? null,
          editorialDescription: propertyRow.editorial_description ?? null,
          listingAgentName: propertyRow.listing_agent_name ?? null,
          listingAgentEmail: propertyRow.listing_agent_email ?? null,
          listingAgentPhone: propertyRow.listing_agent_phone ?? null,
          listingOffice: propertyRow.listing_office ?? null,
          archived: propertyRow.archived_at !== null,
          createdAtLabel: propertyRow.created_at_label ?? null,
          updatedAtLabel: propertyRow.updated_at_label ?? null,
        }
      : null,
    media,
    seller,
    openTasks,
    activity: (activityRows as PropertyActivityEntry[]),
    interests,
    enquiries,
    showings,
    deals,
  }
}

// ---------------------------------------------------------------------------
// Writes
// ---------------------------------------------------------------------------

export type PropertyFactsInput = {
  name: string
  slug: string | null
  propertyType: string | null
  listPrice: number | null
  location: string | null
  city: string | null
  stateOrProvince: string | null
  neighborhood: string | null
  latitude: number | null
  longitude: number | null
  bedrooms: number | null
  bathrooms: number | null
  squareFeet: number | null
  lotSize: number | null
  lotSizeUnits: string | null
  yearBuilt: number | null
  stories: number | null
  parkingSpaces: number | null
  shortDescription: string | null
  editorialDescription: string | null
}

const SLUG_PATTERN = /^[a-z0-9]+(?:-[a-z0-9]+)*$/

export async function updatePropertyFacts(
  propertyId: string,
  input: PropertyFactsInput,
): Promise<{ id: string; slug: string | null }> {
  const name = input.name?.trim()
  if (!name) {
    throw new PortalWriteError('validation', 'Property name is required.')
  }

  const slug = input.slug?.trim() || null
  if (slug && !SLUG_PATTERN.test(slug)) {
    throw new PortalWriteError(
      'validation',
      'Slug must be lowercase letters, numbers, and hyphens.',
    )
  }

  const nonNegative = (value: number | null, label: string) => {
    if (value !== null && (!Number.isFinite(value) || value < 0)) {
      throw new PortalWriteError('validation', `${label} must be zero or greater.`)
    }
  }

  nonNegative(input.listPrice, 'List price')
  nonNegative(input.bedrooms, 'Bedrooms')
  nonNegative(input.bathrooms, 'Bathrooms')
  nonNegative(input.squareFeet, 'Square footage')
  nonNegative(input.lotSize, 'Lot size')
  nonNegative(input.stories, 'Stories')
  nonNegative(input.parkingSpaces, 'Parking spaces')

  if (
    input.latitude !== null &&
    (!Number.isFinite(input.latitude) ||
      input.latitude < -90 ||
      input.latitude > 90)
  ) {
    throw new PortalWriteError('validation', 'Latitude must be between -90 and 90.')
  }
  if (
    input.longitude !== null &&
    (!Number.isFinite(input.longitude) ||
      input.longitude < -180 ||
      input.longitude > 180)
  ) {
    throw new PortalWriteError('validation', 'Longitude must be between -180 and 180.')
  }
  if (
    input.yearBuilt !== null &&
    (!Number.isInteger(input.yearBuilt) ||
      input.yearBuilt < 1000 ||
      input.yearBuilt > 2100)
  ) {
    throw new PortalWriteError('validation', 'Year built must be between 1000 and 2100.')
  }

  if (slug) {
    const clash = await sql`
      select id from property
      where slug = ${slug} and id <> ${propertyId}
      limit 1
    `
    if (clash.length > 0) {
      throw new PortalWriteError(
        'conflict',
        'Another property already uses this slug.',
      )
    }
  }

  const rows = await sql`
    update property
    set
      name = ${name},
      slug = ${slug},
      property_type = ${input.propertyType},
      list_price = ${input.listPrice},
      location = ${input.location},
      city = ${input.city},
      state_or_province = ${input.stateOrProvince},
      neighborhood = ${input.neighborhood},
      latitude = ${input.latitude},
      longitude = ${input.longitude},
      bedrooms = ${input.bedrooms},
      bathrooms = ${input.bathrooms},
      square_feet = ${input.squareFeet},
      lot_size = ${input.lotSize},
      lot_size_units = ${input.lotSizeUnits},
      year_built = ${input.yearBuilt},
      stories = ${input.stories},
      parking_spaces = ${input.parkingSpaces},
      short_description = ${input.shortDescription},
      editorial_description = ${input.editorialDescription},
      updated_at = now()
    where id = ${propertyId}
    returning id, slug
  `

  const row = rows[0] as { id: string; slug: string | null } | undefined
  if (!row) {
    throw new PortalWriteError('not-found', 'Property not found.')
  }
  return { id: row.id, slug: row.slug ?? null }
}

export async function updatePropertyVisibility(
  propertyId: string,
  input: { featured: boolean; status: string },
): Promise<{ id: string; slug: string | null }> {
  if (!EDITABLE_PROPERTY_STATUSES.includes(input.status as EditablePropertyStatus)) {
    throw new PortalWriteError(
      'validation',
      'This status cannot be changed from listing administration (transaction statuses are managed in CRM-14).',
    )
  }

  const current = await sql`
    select status from property where id = ${propertyId} limit 1
  `
  const currentRow = current[0] as { status: string } | undefined
  if (!currentRow) {
    throw new PortalWriteError('not-found', 'Property not found.')
  }
  if ((CRM14_PROPERTY_STATUSES as readonly string[]).includes(currentRow.status)) {
    throw new PortalWriteError(
      'conflict',
      'Property is under contract or sold; its status is owned by the transaction workflow (CRM-14).',
    )
  }

  const rows = await sql`
    update property
    set
      featured = ${input.featured},
      status = ${input.status},
      updated_at = now()
    where id = ${propertyId}
    returning id, slug
  `

  const row = rows[0] as { id: string; slug: string | null } | undefined
  if (!row) {
    throw new PortalWriteError('not-found', 'Property not found.')
  }
  return { id: row.id, slug: row.slug ?? null }
}

export async function setPropertyPublished(
  propertyId: string,
  isPublished: boolean,
  execute: QueryExecutor = sql,
): Promise<{ id: string; slug: string | null }> {
  // HARDEN-05 — publication is the single Property-level market-visibility
  // authority. Idempotent: setting the same value again is a no-op update.
  if (typeof isPublished !== 'boolean') {
    throw new PortalWriteError(
      'validation',
      'isPublished must be a boolean.',
    )
  }

  const rows = await execute`
    update property
    set is_published = ${isPublished}, updated_at = now()
    where id = ${propertyId}
    returning id, slug
  `

  const row = rows[0] as { id: string; slug: string | null } | undefined
  if (!row) {
    throw new PortalWriteError('not-found', 'Property not found.')
  }
  return { id: row.id, slug: row.slug ?? null }
}

export async function setPropertyMediaOrder(
  propertyId: string,
  orderedMediaIds: string[],
): Promise<{ id: string }> {
  if (orderedMediaIds.length === 0) {
    throw new PortalWriteError('validation', 'Media order cannot be empty.')
  }
  const unique = new Set(orderedMediaIds)
  if (unique.size !== orderedMediaIds.length) {
    throw new PortalWriteError('validation', 'Media order contains duplicates.')
  }

  const owned = await sql`
    select media_id from property_media where property_id = ${propertyId}
  `
  const ownedSet = new Set(
    (owned as Array<{ media_id: string }>).map((row) => String(row.media_id)),
  )
  for (const mediaId of orderedMediaIds) {
    if (!ownedSet.has(mediaId)) {
      throw new PortalWriteError(
        'validation',
        'Media is not linked to this property.',
      )
    }
  }

  const positions = orderedMediaIds.map((_, index) => index)
  await sql`
    update property_media pm
    set sort_order = ord.position
    from unnest(${orderedMediaIds}::uuid[], ${positions}::int[]) as ord(media_id, position)
    where pm.property_id = ${propertyId}
      and pm.media_id = ord.media_id
  `

  return { id: propertyId }
}

export async function setPropertyHero(
  propertyId: string,
  mediaId: string,
): Promise<{ id: string; mediaId: string }> {
  const target = await sql`
    select pm.media_id, m.media_type
    from property_media pm
    join media m
      on m.id = pm.media_id
    where pm.property_id = ${propertyId}
      and pm.media_id = ${mediaId}
    limit 1
  `
  const targetRow = target[0] as { media_id: string; media_type: string } | undefined
  if (!targetRow) {
    throw new PortalWriteError('validation', 'Media is not linked to this property.')
  }
  if (targetRow.media_type !== 'image') {
    throw new PortalWriteError('validation', 'Only images can be the hero.')
  }

  // Atomic demote + promote in one statement so the one-hero invariant cannot
  // be left in a zero-hero (or multi-hero) state if the write fails mid-way.
  await sql`
    with demoted as (
      update property_media
      set role = 'gallery'
      where property_id = ${propertyId}
        and role = 'hero'
    )
    update property_media
    set role = 'hero'
    where property_id = ${propertyId}
      and media_id = ${mediaId}
  `

  return { id: propertyId, mediaId }
}

export async function unlinkPropertyMedia(
  propertyId: string,
  mediaId: string,
): Promise<{ id: string; mediaId: string }> {
  const rows = await sql`
    delete from property_media
    where property_id = ${propertyId}
      and media_id = ${mediaId}
    returning media_id
  `
  if (rows.length === 0) {
    throw new PortalWriteError(
      'not-found',
      'Media is not linked to this property.',
    )
  }
  return { id: propertyId, mediaId }
}

export async function updateMediaMetadata(
  mediaId: string,
  input: { altText: string | null; caption: string | null },
): Promise<{ id: string }> {
  const rows = await sql`
    update media
    set
      alt_text = ${input.altText},
      caption = ${input.caption},
      updated_at = now()
    where id = ${mediaId}
    returning id
  `
  const row = rows[0] as { id: string } | undefined
  if (!row) {
    throw new PortalWriteError('not-found', 'Media not found.')
  }
  return { id: row.id }
}
