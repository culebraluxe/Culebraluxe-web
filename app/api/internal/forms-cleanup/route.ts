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

export async function GET(request: Request) {
  const url = new URL(request.url)
  if (url.searchParams.get('token') !== TOKEN) return new NextResponse('Not found', { status: 404 })
  return NextResponse.json({ ok: true, forms: await snapshot() })
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
    if (!keepId) return { keepId: null, deletedForms: 0 }

    const deleted = await tx`
      delete from document_form_instance
      where id <> ${keepId}
      returning id
    `

    return {
      keepId,
      deletedForms: deleted.length,
      deletedFormIds: deleted.map((row) => String(row.id)),
    }
  })

  return NextResponse.json({ ok: true, result, forms: await snapshot() })
}
