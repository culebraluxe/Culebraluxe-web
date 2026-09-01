import { NextResponse } from 'next/server'

import { db, dbTargetInfo } from '@/db/client'

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

  const result = await db.transaction('ops-jessica-listing-v4-rebuild', async (tx) => {
    const people = await tx`
      select id, display_name
      from person
      where lower(trim(display_name)) = lower('Jessica Iverson')
      order by id
    `
    if (people.length !== 1) {
      throw new Error(`Expected exactly one Jessica Iverson person; found ${people.length}.`)
    }

    const properties = await tx`
      select id, name, location
      from property
      where lower(trim(name)) = lower('Sea to Soul')
      order by id
    `
    if (properties.length !== 1) {
      throw new Error(`Expected exactly one Sea to Soul property; found ${properties.length}.`)
    }

    const brokers = await tx`
      select id, person_id, display_name
      from app_user
      where active = true
        and lower(trim(display_name)) = lower('Lisa Penfield')
      order by id
    `
    if (brokers.length !== 1) {
      throw new Error(`Expected exactly one active Lisa Penfield app user; found ${brokers.length}.`)
    }

    const personId = String(people[0].id)
    const propertyId = String(properties[0].id)
    const brokerUserId = String(brokers[0].id)

    const existing = await tx`
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
      throw new Error(`Refusing to proceed: found ${existing.length} matching LISTING-01 v4 forms.`)
    }

    if (existing.length === 1) {
      const formId = String(existing[0].id)
      const participants = await tx`
        select id, role, person_id, display_name, sort_order
        from document_form_participant
        where form_instance_id = ${formId}
        order by sort_order asc, id
      `
      return {
        created: false,
        form: existing[0],
        participants,
        resolved: {
          personId,
          propertyId,
          brokerUserId,
          propertyLocation: properties[0].location ?? null,
        },
      }
    }

    const forms = await tx`
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
    `

    if (forms.length !== 1) throw new Error('Form insert did not return exactly one row.')
    const formId = String(forms[0].id)

    const participants = await tx`
      insert into document_form_participant (
        form_instance_id, role, person_id, display_name, sort_order
      ) values (
        ${formId}, 'SELLER', ${personId}, 'Jessica Iverson', 0
      )
      returning id, form_instance_id, role, person_id, display_name, sort_order
    `

    return {
      created: true,
      form: forms[0],
      participants,
      resolved: {
        personId,
        propertyId,
        brokerUserId,
        propertyLocation: properties[0].location ?? null,
      },
    }
  })

  if (!result.ok) {
    return NextResponse.json(
      { ok: false, error: result.error, databaseTarget: target },
      { status: 500 },
    )
  }

  return NextResponse.json({ ok: true, databaseTarget: target, ...result.data })
}
