import { sql } from './client'

// Read-only Deal Workspace projection (CRM-12B + CRM-13). Composes canonical
// deal, property, person, app_user, task, interaction, deal_participant, and
// offer data. Participants are read from the canonical deal_participant table
// (active rows), resolving person vs app_user deterministically; the legacy
// deal/client/owner/seller FKs remain untouched for existing read paths.

export type DealParticipant = {
  id: string
  roleCategory: 'client' | 'owner' | 'seller' | 'other'
  roleLabel: string | null
  kind: 'person' | 'user'
  personId: string | null
  userId: string | null
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

export type DealWorkspaceOffer = {
  id: string
  personId: string
  personName: string | null
  parentOfferId: string | null
  amount: number
  status: string
  submittedAtLabel: string
  respondedAtLabel: string | null
  note: string | null
  isCounter: boolean
}

export type DealWorkspaceShowing = {
  id: string
  personId: string
  personName: string
  status: string
  requestedAtLabel: string
  scheduledAtLabel: string | null
  completedAtLabel: string | null
  cancelledAtLabel: string | null
  feedback: string | null
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
  offers: DealWorkspaceOffer[]
  showings: DealWorkspaceShowing[]
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

type ShowingRow = {
  id: string
  person_id: string
  person_name: string
  status: string
  requested_at_label: string
  scheduled_at_label: string | null
  completed_at_label: string | null
  cancelled_at_label: string | null
  feedback: string | null
}

type ParticipantRow = {
  id: string
  role_category: string
  role_label: string | null
  person_id: string | null
  user_id: string | null
  person_name: string | null
  user_name: string | null
  person_email: string | null
  person_phone: string | null
}

type OfferRow = {
  id: string
  person_id: string
  person_name: string | null
  parent_offer_id: string | null
  amount: string
  status: string
  submitted_at_label: string
  responded_at_label: string | null
  note: string | null
}

function toNumber(value: string | null) {
  return value === null ? null : Number(value)
}

export async function getDealWorkspace(
  dealId: string,
): Promise<DealWorkspace> {
  const [
    dealRows,
    taskRows,
    activityRows,
    participantRows,
    offerRows,
    showingRows,
  ] = await Promise.all([
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
    sql`
      select
        dp.id,
        dp.role as role_category,
        dp.role_label,
        dp.person_id,
        dp.user_id,
        person.display_name as person_name,
        app_user.display_name as user_name,
        person_email.identity_value as person_email,
        person_phone.identity_value as person_phone
      from deal_participant dp
      left join person
        on person.id = dp.person_id
      left join app_user
        on app_user.id = dp.user_id
      left join lateral (
        select pi.identity_value
        from person_identity pi
        where pi.person_id = dp.person_id
          and pi.identity_type = 'email'
        order by pi.is_primary desc, pi.created_at asc
        limit 1
      ) person_email on true
      left join lateral (
        select pi.identity_value
        from person_identity pi
        where pi.person_id = dp.person_id
          and pi.identity_type = 'phone'
        order by pi.is_primary desc, pi.created_at asc
        limit 1
      ) person_phone on true
      where dp.deal_id = ${dealId}
        and dp.active = true
      order by
        case dp.role
          when 'client' then 0
          when 'owner' then 1
          when 'seller' then 2
          else 3
        end,
        dp.created_at asc
    `,
    sql`
      select
        o.id,
        o.person_id,
        person.display_name as person_name,
        o.parent_offer_id,
        o.amount,
        o.status,
        to_char(
          o.submitted_at at time zone 'America/Puerto_Rico',
          'Mon FMDD, YYYY HH12:MI AM'
        ) as submitted_at_label,
        to_char(
          o.responded_at at time zone 'America/Puerto_Rico',
          'Mon FMDD, YYYY HH12:MI AM'
        ) as responded_at_label,
        o.note
      from offer o
      left join person
        on person.id = o.person_id
      where o.deal_id = ${dealId}
      order by o.submitted_at asc
    `,
    sql`
      select
        s.id,
        s.person_id,
        person.display_name as person_name,
        s.status,
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
        ) as cancelled_at_label,
        s.feedback
      from showing s
      join person
        on person.id = s.person_id
      where s.deal_id = ${dealId}
      order by
        case s.status
          when 'requested' then 0
          when 'scheduled' then 1
          when 'completed' then 2
          when 'cancelled' then 3
          else 4
        end,
        s.requested_at desc
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
      offers: [],
      showings: [],
    }
  }

  const participants: DealParticipant[] = (
    participantRows as ParticipantRow[]
  ).map((row) => ({
    id: row.id,
    roleCategory: row.role_category as DealParticipant['roleCategory'],
    roleLabel: row.role_label ?? null,
    kind: row.person_id ? 'person' : 'user',
    personId: row.person_id ?? null,
    userId: row.user_id ?? null,
    name: row.person_name ?? row.user_name ?? 'Unknown',
    detail: row.person_id
      ? [row.person_email, row.person_phone].filter(Boolean).join(' · ') || null
      : null,
  }))

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
    offers: (offerRows as OfferRow[]).map((row) => ({
      id: row.id,
      personId: row.person_id,
      personName: row.person_name ?? null,
      parentOfferId: row.parent_offer_id ?? null,
      amount: Number(row.amount),
      status: row.status,
      submittedAtLabel: row.submitted_at_label,
      respondedAtLabel: row.responded_at_label ?? null,
      note: row.note ?? null,
      isCounter: Boolean(row.parent_offer_id),
    })),
    showings: (showingRows as ShowingRow[]).map((row) => ({
      id: row.id,
      personId: row.person_id,
      personName: row.person_name,
      status: row.status,
      requestedAtLabel: row.requested_at_label,
      scheduledAtLabel: row.scheduled_at_label ?? null,
      completedAtLabel: row.completed_at_label ?? null,
      cancelledAtLabel: row.cancelled_at_label ?? null,
      feedback: row.feedback ?? null,
    })),
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
