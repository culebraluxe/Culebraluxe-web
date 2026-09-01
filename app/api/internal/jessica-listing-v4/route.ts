import { NextResponse } from 'next/server'

import { dbTargetInfo, sql } from '@/db/client'

const ONE_TIME_TOKEN = 'A-B9hqcUvvDF5cErRL_p5abMXx8J2VeJ'

const FIELD_VALUES = {
  sellerName: 'Jessica Iverson',
  sellerCivilStatus: 'Single',
  sellerResidenceAddress: '26 Calle Pedro Marquez, PO Box 786',
  brokerName: 'Lisa Penfield',
  property: 'Sea to Soul',
  propertyLocation: 'Playa Sardinas II',
  catastroNumber: '476-054-192-33-000',
  listPrice: '650000',
  commission: '4%',
  startDate: '2026-08-31',
  endDate: '2027-07-27',
  listingType: 'Exclusive Right to Sell',
} as const

export const dynamic = 'force-dynamic'

export async function GET(request: Request) {
  const url = new URL(request.url)
  if (url.searchParams.get('token') !== ONE_TIME_TOKEN) {
    return NextResponse.json({ ok: false, error: 'not-found' }, { status: 404 })
  }

  const target = dbTargetInfo()
  if (target.target !== 'prod' || process.env.VERCEL_ENV !== 'production') {
    return NextResponse.json(
      { ok: false, error: 'production-only', databaseTarget: target },
      { status: 409 },
    )
  }

  try {
    const people = await sql`
      select id, display_name
      from person
      where lower(trim(display_name)) = lower('Jessica Iverson')
      order by id
    `
    const properties = await sql`
      select id, name, location
      from property
      where lower(trim(name)) = lower('Sea to Soul')
      order by id
    `
    const brokers = await sql`
      select id, person_id, display_name
      from app_user
      where active = true
        and lower(trim(display_name)) = lower('Lisa Penfield')
      order by id
    `

    if (people.length !== 1 || properties.length !== 1 || brokers.length !== 1) {
      return NextResponse.json(
        {
          ok: false,
          error: 'resolution-mismatch',
          counts: {
            jessicaPeople: people.length,
            seaToSoulProperties: properties.length,
            activeLisaUsers: brokers.length,
          },
          databaseTarget: target,
        },
        { status: 409 },
      )
    }

    const personId = String(people[0].id)
    const propertyId = String(properties[0].id)
    const brokerUserId = String(brokers[0].id)

    const existing = await sql`
      select id, template_id, template_version, status, person_id, property_id,
        field_values, created_at, updated_at
      from document_form_instance
      where template_id = 'LISTING-01'
        and template_version = 4
        and person_id = ${personId}
        and property_id = ${propertyId}
        and field_values ->> 'sellerName' = 'Jessica Iverson'
        and field_values ->> 'catastroNumber' = '476-054-192-33-000'
      order by created_at desc
      limit 2
    `

    if (existing.length > 1) {
      return NextResponse.json(
        { ok: false, error: 'duplicate-v4', count: existing.length, databaseTarget: target },
        { status: 409 },
      )
    }

    if (existing.length === 1) {
      const formId = String(existing[0].id)
      const participants = await sql`
        select id, role, person_id, display_name, sort_order
        from document_form_participant
        where form_instance_id = ${formId}
        order by sort_order asc, id
      `
      return NextResponse.json({
        ok: true,
        created: false,
        databaseTarget: target,
        form: existing[0],
        participants,
        resolved: {
          personId,
          propertyId,
          brokerUserId,
          propertyLocation: properties[0].location ?? null,
        },
      })
    }

    const rows = await sql`
      with created_form as (
        insert into document_form_instance (
          template_id,
          template_version,
          deal_id,
          person_id,
          property_id,
          status,
          field_values,
          sections,
          created_by_user_id
        ) values (
          'LISTING-01',
          4,
          null,
          ${personId},
          ${propertyId},
          'draft',
          ${JSON.stringify(FIELD_VALUES)}::jsonb,
          '{}'::jsonb,
          ${brokerUserId}
        )
        returning id, template_id, template_version, status, deal_id, person_id, property_id,
          field_values, sections, created_by_user_id, created_at, updated_at
      ), created_participant as (
        insert into document_form_participant (
          form_instance_id, role, person_id, display_name, sort_order
        )
        select id, 'SELLER', ${personId}, 'Jessica Iverson', 0
        from created_form
        returning id, form_instance_id, role, person_id, display_name, sort_order
      )
      select
        f.id,
        f.template_id,
        f.template_version,
        f.status,
        f.deal_id,
        f.person_id,
        f.property_id,
        f.field_values,
        f.sections,
        f.created_by_user_id,
        f.created_at,
        f.updated_at,
        p.id as participant_id,
        p.role as participant_role,
        p.person_id as participant_person_id,
        p.display_name as participant_display_name,
        p.sort_order as participant_sort_order
      from created_form f
      join created_participant p on p.form_instance_id = f.id
    `

    if (rows.length !== 1) {
      return NextResponse.json(
        { ok: false, error: 'unexpected-insert-result', count: rows.length, databaseTarget: target },
        { status: 500 },
      )
    }

    const row = rows[0]
    return NextResponse.json({
      ok: true,
      created: true,
      databaseTarget: target,
      form: {
        id: row.id,
        template_id: row.template_id,
        template_version: row.template_version,
        status: row.status,
        deal_id: row.deal_id,
        person_id: row.person_id,
        property_id: row.property_id,
        field_values: row.field_values,
        sections: row.sections,
        created_by_user_id: row.created_by_user_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
      },
      participants: [
        {
          id: row.participant_id,
          role: row.participant_role,
          person_id: row.participant_person_id,
          display_name: row.participant_display_name,
          sort_order: row.participant_sort_order,
        },
      ],
      resolved: {
        personId,
        propertyId,
        brokerUserId,
        propertyLocation: properties[0].location ?? null,
      },
    })
  } catch (error) {
    return NextResponse.json(
      {
        ok: false,
        error: error instanceof Error ? error.message : 'unknown',
        databaseTarget: target,
      },
      { status: 500 },
    )
  }
}
