import { NextResponse } from 'next/server'

import { sql } from '@/db/client'
import { neonTx } from '@/db/tx'

export const dynamic = 'force-dynamic'

const TOKEN = 'forms-cleanup-0830-keep-newest'

async function snapshot() {
  const rows = await sql`
    select
      f.id,
      f.template_id,
      f.template_version,
      f.status,
      f.created_at,
      f.updated_at,
      (select count(*)::int from document_form_participant p where p.form_instance_id = f.id) as participant_count,
      (select count(*)::int from transaction_document td where td.form_instance_id = f.id) as issued_document_count
    from document_form_instance f
    order by f.created_at desc, f.id desc
  `
  return rows
}

async function dependencyGraph() {
  return sql`
    select
      tc.table_schema,
      tc.table_name,
      kcu.column_name,
      ccu.table_schema as referenced_table_schema,
      ccu.table_name as referenced_table_name,
      ccu.column_name as referenced_column_name,
      rc.delete_rule
    from information_schema.table_constraints tc
    join information_schema.key_column_usage kcu
      on tc.constraint_name = kcu.constraint_name
     and tc.constraint_schema = kcu.constraint_schema
    join information_schema.constraint_column_usage ccu
      on ccu.constraint_name = tc.constraint_name
     and ccu.constraint_schema = tc.constraint_schema
    join information_schema.referential_constraints rc
      on rc.constraint_name = tc.constraint_name
     and rc.constraint_schema = tc.constraint_schema
    where tc.constraint_type = 'FOREIGN KEY'
      and ccu.table_name in ('document_form_instance', 'transaction_document', 'media')
    order by ccu.table_name, tc.table_name, kcu.column_name
  `
}

export async function GET(request: Request) {
  const url = new URL(request.url)
  if (url.searchParams.get('token') !== TOKEN) return new NextResponse('Not found', { status: 404 })
  const [forms, dependencies] = await Promise.all([snapshot(), dependencyGraph()])
  return NextResponse.json({ ok: true, forms, dependencies })
}

export async function POST(request: Request) {
  const url = new URL(request.url)
  if (url.searchParams.get('token') !== TOKEN) return new NextResponse('Not found', { status: 404 })

  const result = await neonTx(async (tx) => {
    const newestRows = await tx`
      select id
      from document_form_instance
      order by created_at desc, id desc
      limit 1
      for update
    `
    const keepId = newestRows[0]?.id ? String(newestRows[0].id) : null
    if (!keepId) return { keepId: null, deletedForms: 0, deletedParticipants: 0 }

    const candidates = await tx`
      select f.id
      from document_form_instance f
      where f.id <> ${keepId}
        and not exists (
          select 1 from transaction_document td where td.form_instance_id = f.id
        )
      for update
    `
    const ids = candidates.map((row) => String(row.id))
    if (ids.length === 0) return { keepId, deletedForms: 0, deletedParticipants: 0 }

    const participantDelete = await tx`
      delete from document_form_participant
      where form_instance_id = any(${ids}::uuid[])
      returning id
    `
    const formDelete = await tx`
      delete from document_form_instance
      where id = any(${ids}::uuid[])
      returning id
    `

    return {
      keepId,
      deletedForms: formDelete.length,
      deletedParticipants: participantDelete.length,
      deletedFormIds: formDelete.map((row) => String(row.id)),
    }
  })

  const forms = await snapshot()
  return NextResponse.json({ ok: true, result, forms })
}
