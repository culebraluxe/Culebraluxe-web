import { sql } from './client'

// Read-only Deal Workspace projection (CRM-12B + CRM-13). Composes canonical
// deal, property, person, app_user, task, and interaction data. Participants
// (client / owner / seller) are derived only from existing canonical FKs —
// there is no normalized participants table.

export type DealParticipant = {
  role: string
  kind: 'person' | 'user'
  name: string
  detail: string | null
}

export type DealWorkspaceTask = {
  id: string
  title: string
  detail: string | null
  dueAtLabel: string | null
  isOverdue: boolean
}

export type DealWorkspaceActivity = {
  id: string
  personId: string | null
  channel: string
  direction: string | null
  occurredAtLabel: string
  title: string | null
  summary: string | null
  personName: string | null
}

export type DealWorkspace = {
  deal: {
    id: string
    stage: string
    listPrice: number | null
    offerPrice: number | null
    closingDateLabel: string | null
    closedAtLabel: string | null
    notes: string | null
    createdAtLabel: string
    updatedAtLabel: string
  } | null
  property: {
    name: string
    location: string | null
    propertyType: string | null
    bedrooms: number | null
    bathrooms: number | null
    squareFeet: number | null
  } | null
  client: {
    id: string
    displayName: string
    role: string
    status: string
    email: string | null
    phone: string | null
  } | null
  participants: DealParticipant[]
  openTasks: DealWorkspaceTask[]
  activity: DealWorkspaceActivity[]
}

type DealRow = {
  id: string
  stage: string
  list_price: string | null
  offer_price: string | null
  notes: string | null
  closing_date_label: string | null
  closed_at_label: string | null
  created_at_label: string
  updated_at_label: string
  property_name: string
  property_location: string | null
  property_type: string | null
  bedrooms: string | null
  bathrooms: string | null
  square_feet: number | null
  client_id: string
  client_name: string
  client_role: string
  client_status: string
  client_email: string | null
  client_phone: string | null
  owner_name: string | null
  seller_id: string | null
  seller_name: string | null
}

type TaskRow = {
  id: string
  title: string
  detail: string | null
  due_at: string | null
  due_at_label: string | null
}

type ActivityRow = {
  id: string
  person_id: string | null
  channel: string
  direction: string | null
  occurred_at_label: string
  title: string | null
  summary: string | null
  person_name: string | null
}

function toNumber(value: string | null) {
  return value === null ? null : Number(value)
}

export async function getDealWorkspace(
  dealId: string,
): Promise<DealWorkspace> {
  const [dealRows, taskRows, activityRows] = await Promise.all([
    sql`
      select
        d.id,
        d.stage,
        d.list_price,
        d.offer_price,
        d.notes,
        to_char(d.closing_date, 'Mon FMDD, YYYY') as closing_date_label,
        to_char(
          d.closed_at at time zone 'America/Puerto_Rico',
          'Mon FMDD, YYYY'
        ) as closed_at_label,
        to_char(
          d.created_at at time zone 'America/Puerto_Rico',
          'Mon FMDD, YYYY'
        ) as created_at_label,
        to_char(
          d.updated_at at time zone 'America/Puerto_Rico',
          'Mon FMDD, YYYY'
        ) as updated_at_label,
        property.name as property_name,
        property.location as property_location,
        property.property_type,
        property.bedrooms,
        property.bathrooms,
        property.square_feet,
        client.id as client_id,
        client.display_name as client_name,
        client.role as client_role,
        client.status as client_status,
        client_email.identity_value as client_email,
        client_phone.identity_value as client_phone,
        owner.display_name as owner_name,
        seller.id as seller_id,
        seller.display_name as seller_name
      from deal d
      join property
        on property.id = d.property_id
      join person client
        on client.id = d.client_person_id
      left join app_user owner
        on owner.id = d.owner_user_id
      left join person seller
        on seller.id = property.seller_person_id
      left join lateral (
        select pi.identity_value
        from person_identity pi
        where pi.person_id = client.id
          and pi.identity_type = 'email'
        order by pi.is_primary desc, pi.created_at asc
        limit 1
      ) client_email on true
      left join lateral (
        select pi.identity_value
        from person_identity pi
        where pi.person_id = client.id
          and pi.identity_type = 'phone'
        order by pi.is_primary desc, pi.created_at asc
        limit 1
      ) client_phone on true
      where d.id = ${dealId}
      limit 1
    `,
    sql`
      select
        t.id,
        t.title,
        t.detail,
        t.due_at,
        to_char(
          t.due_at at time zone 'America/Puerto_Rico',
          'Mon FMDD, YYYY'
        ) as due_at_label
      from task t
      where t.deal_id = ${dealId}
        and t.status = 'open'
      order by t.due_at asc nulls last, t.created_at asc
    `,
    sql`
      select
        i.id,
        person.id as person_id,
        i.channel,
        i.direction,
        to_char(
          i.occurred_at at time zone 'America/Puerto_Rico',
          'Mon FMDD, YYYY HH12:MI AM'
        ) as occurred_at_label,
        i.title,
        i.summary,
        person.display_name as person_name
      from interaction i
      left join person
        on person.id = i.person_id
      where i.deal_id = ${dealId}
      order by i.occurred_at desc
      limit 20
    `,
  ])

  const dealRow = (dealRows as DealRow[])[0]
  const now = Date.now()

  if (!dealRow) {
    return {
      deal: null,
      property: null,
      client: null,
      participants: [],
      openTasks: [],
      activity: [],
    }
  }

  const participants: DealParticipant[] = []
  participants.push({
    role: 'Client',
    kind: 'person',
    name: dealRow.client_name,
    detail: [dealRow.client_email, dealRow.client_phone]
      .filter(Boolean)
      .join(' · ') || null,
  })
  if (dealRow.owner_name) {
    participants.push({ role: 'Owner', kind: 'user', name: dealRow.owner_name, detail: null })
  }
  if (dealRow.seller_id && dealRow.seller_name) {
    participants.push({ role: 'Seller', kind: 'person', name: dealRow.seller_name, detail: null })
  }

  return {
    deal: {
      id: dealRow.id,
      stage: dealRow.stage,
      listPrice: toNumber(dealRow.list_price),
      offerPrice: toNumber(dealRow.offer_price),
      closingDateLabel: dealRow.closing_date_label ?? null,
      closedAtLabel: dealRow.closed_at_label ?? null,
      notes: dealRow.notes ?? null,
      createdAtLabel: dealRow.created_at_label,
      updatedAtLabel: dealRow.updated_at_label,
    },
    property: {
      name: dealRow.property_name,
      location: dealRow.property_location ?? null,
      propertyType: dealRow.property_type ?? null,
      bedrooms: toNumber(dealRow.bedrooms),
      bathrooms: toNumber(dealRow.bathrooms),
      squareFeet: dealRow.square_feet,
    },
    client: {
      id: dealRow.client_id,
      displayName: dealRow.client_name,
      role: dealRow.client_role,
      status: dealRow.client_status,
      email: dealRow.client_email ?? null,
      phone: dealRow.client_phone ?? null,
    },
    participants,
    openTasks: (taskRows as TaskRow[]).map((row) => ({
      id: row.id,
      title: row.title,
      detail: row.detail ?? null,
      dueAtLabel: row.due_at_label ?? null,
      isOverdue:
        row.due_at !== null && new Date(row.due_at).getTime() < now,
    })),
    activity: (activityRows as ActivityRow[]).map((row) => ({
      id: row.id,
      personId: row.person_id ?? null,
      channel: row.channel,
      direction: row.direction ?? null,
      occurredAtLabel: row.occurred_at_label,
      title: row.title ?? null,
      summary: row.summary ?? null,
      personName: row.person_name ?? null,
    })),
  }
}
